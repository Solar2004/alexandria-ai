//! Servidor axum del proxy: un puerto, ambos protocolos, failover completo.
//!
//! Endpoints:
//! - `POST /v1/messages`              → cliente Anthropic (Claude Code, etc.)
//! - `POST /v1/messages/count_tokens` → estimación local (no gasta upstream)
//! - `POST /v1/chat/completions`      → cliente OpenAI (cualquier SDK)
//! - `GET  /proxy/status`             → proveedores, circuitos, ajustes
//! - `GET  /health`
//!
//! Flujo por request: clasifica la tarea (governor o `X-Alx-Tier`) → lista de
//! candidatos (router con rotación y breaker) → intenta en orden → máscara del
//! modelo → ledger. Streaming same-protocol = passthrough con máscara del
//! primer chunk; streaming cross-protocol = upstream no-stream + SSE sintético
//! (v1 honesto: el texto llega en un solo delta).

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::StreamExt;
use serde_json::{json, Value};

use alx_core::types::ModelTier;

use crate::config::ProxySettings;
use crate::mask;
use crate::route::RouteEngine;
use crate::translate;
use crate::{estimate_tokens, Protocol};

pub struct AppState {
    pub engine: RouteEngine,
    pub http: reqwest::Client,
    pub semaphore: Arc<tokio::sync::Semaphore>,
    pub settings: ProxySettings,
}

fn tier_u8(t: ModelTier) -> u8 {
    match t {
        ModelTier::T1Cheap => 1,
        ModelTier::T2Medium => 2,
        ModelTier::T3Premium => 3,
    }
}

/// Ledger: una línea JSONL por intento. Ruta: `$ALX_PROXY_LEDGER` →
/// `~/.local/state/alexandria/proxy-ledger.jsonl`. Falla silenciosa: el
/// ledger nunca rompe un request.
fn ledger(event: Value) {
    let path = std::env::var("ALX_PROXY_LEDGER").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{home}/.local/state/alexandria/proxy-ledger.jsonl")
    });
    if let Some(dir) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        use std::io::Write;
        let _ = writeln!(f, "{event}");
    }
}

fn upstream_url(protocol: &str, base: &str) -> String {
    let b = base.trim_end_matches('/');
    match protocol {
        "anthropic" => format!("{b}/v1/messages"),
        _ => {
            if b.ends_with("/v1") {
                format!("{b}/chat/completions")
            } else {
                format!("{b}/v1/chat/completions")
            }
        }
    }
}

fn auth_headers(protocol: &str, key: Option<&str>) -> Vec<(String, String)> {
    match (protocol, key) {
        ("anthropic", Some(k)) => vec![
            ("x-api-key".into(), k.to_string()),
            ("anthropic-version".into(), "2023-06-01".into()),
        ],
        ("anthropic", None) => vec![("anthropic-version".into(), "2023-06-01".into())],
        ("openai", Some(k)) => vec![("authorization".into(), format!("Bearer {k}"))],
        _ => vec![],
    }
}

/// Construye el body upstream: misma familia → passthrough con modelo real;
/// familia distinta → traducción. `stream` lo fuerza el llamador.
fn build_upstream_body(
    client_proto: Protocol,
    body: &Value,
    cand_model: &str,
    upstream_proto: Protocol,
) -> Value {
    match (client_proto, upstream_proto) {
        (Protocol::Anthropic, Protocol::OpenAi) => translate::anth_request_to_oai(body, cand_model),
        (Protocol::OpenAi, Protocol::Anthropic) => translate::oai_request_to_anth(body, cand_model),
        _ => {
            let mut b = body.clone();
            b["model"] = json!(cand_model);
            b
        }
    }
}

fn error_response(client_proto: Protocol, status: StatusCode, msg: &str) -> Response {
    let body = match client_proto {
        Protocol::Anthropic => json!({
            "type": "error",
            "error": {"type": "api_error", "message": msg},
        }),
        Protocol::OpenAi => json!({
            "error": {"message": msg, "type": "api_error", "code": status.as_u16()},
        }),
    };
    (status, Json(body)).into_response()
}

/// SSE sintético para cliente Anthropic cuando el upstream no-stream.
fn synthetic_anth_stream(full: &Value) -> Vec<u8> {
    let text: String = full
        .get("content")
        .and_then(|c| c.as_array())
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();
    let stop = full
        .get("stop_reason")
        .and_then(|s| s.as_str())
        .unwrap_or("end_turn");
    let out_toks = full
        .get("usage")
        .and_then(|u| u.get("output_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let mut out = String::new();
    for (ev, data) in [
        ("message_start", json!({"type":"message_start","message":{"role":"assistant","model":full["model"],"content":[],"usage":full.get("usage").cloned().unwrap_or(json!({}))}})),
        ("content_block_start", json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}})),
        ("content_block_delta", json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":text}})),
        ("content_block_stop", json!({"type":"content_block_stop","index":0})),
        ("message_delta", json!({"type":"message_delta","delta":{"stop_reason":stop},"usage":{"output_tokens":out_toks}})),
        ("message_stop", json!({"type":"message_stop"})),
    ] {
        out.push_str(&format!("event: {ev}\ndata: {data}\n\n"));
    }
    out.into_bytes()
}

/// SSE sintético para cliente OpenAI cuando el upstream no-stream.
fn synthetic_oai_stream(full: &Value) -> Vec<u8> {
    let text = full
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    let model = full.get("model").cloned().unwrap_or(json!("unknown"));
    let mut out = String::new();
    let chunk1 = json!({
        "id": full.get("id").cloned().unwrap_or(json!("chatcmpl-alx")),
        "object": "chat.completion.chunk",
        "model": model,
        "choices": [{"index": 0, "delta": {"role": "assistant", "content": text}, "finish_reason": null}]
    });
    let chunk2 = json!({
        "id": full.get("id").cloned().unwrap_or(json!("chatcmpl-alx")),
        "object": "chat.completion.chunk",
        "model": model,
        "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]
    });
    out.push_str(&format!("data: {chunk1}\n\n"));
    out.push_str(&format!("data: {chunk2}\n\n"));
    out.push_str("data: [DONE]\n\n");
    out.into_bytes()
}

/// Intenta la lista de candidatos en orden; devuelve la primera respuesta
/// upstream sana (sin traducir de vuelta). `None` = todos fallaron.
async fn attempt_upstream(
    state: &AppState,
    client_proto: Protocol,
    body: &Value,
    tier_header: Option<u8>,
) -> Result<(reqwest::Response, crate::route::Candidate), (StatusCode, String)> {
    let want_stream = body.get("stream").and_then(|v| v.as_bool()) == Some(true);
    let text = translate::extract_user_text(body, client_proto);
    let tier = tier_header
        .unwrap_or_else(|| tier_u8(alx_governor::classify_prompt_text(&text)));

    let candidates = state.engine.candidates(tier);
    if candidates.is_empty() {
        return Err((StatusCode::BAD_GATEWAY, "sin proveedores configurados".into()));
    }

    let mut last_err = String::new();
    for cand in candidates {
        let upstream_proto = match cand.protocol.as_str() {
            "anthropic" => Protocol::Anthropic,
            _ => Protocol::OpenAi,
        };
        // Cross-protocol stream → upstream no-stream (v1); luego SSE sintético.
        let upstream_stream = want_stream && client_proto == upstream_proto;
        let mut up_body = build_upstream_body(client_proto, body, &cand.model, upstream_proto);
        up_body["stream"] = json!(upstream_stream);

        let url = upstream_url(&cand.protocol, &cand.base_url);
        let mut req = state
            .http
            .post(&url)
            .timeout(Duration::from_secs(cand.timeout_s))
            .header("content-type", "application/json");
        for (k, v) in auth_headers(&cand.protocol, cand.api_key.as_deref()) {
            req = req.header(k, v);
        }
        let started = Instant::now();
        let resp = req.json(&up_body).send().await;
        match resp {
            Ok(r) if r.status().is_success() => {
                state.engine.record_success(&cand.provider, &cand.model);
                ledger(json!({
                    "ts": started.elapsed().as_millis() as u64,
                    "provider": cand.provider, "model": cand.model,
                    "tier": tier, "ok": true, "ms": started.elapsed().as_millis() as u64,
                }));
                return Ok((r, cand));
            }
            Ok(r) => {
                let code = r.status().as_u16();
                let snippet = r
                    .text()
                    .await
                    .unwrap_or_default()
                    .chars()
                    .take(200)
                    .collect::<String>();
                state.engine.record_failure(&cand.provider, &cand.model);
                last_err = format!("{}/{} → HTTP {code}: {snippet}", cand.provider, cand.model);
                ledger(json!({
                    "provider": cand.provider, "model": cand.model,
                    "tier": tier, "ok": false, "code": code, "err": snippet,
                }));
                if code != 429 && code < 500 {
                    // 4xx de cliente (auth/payload) no mejora con otro modelo
                    // del mismo pool SOLO si es 401/403/429; payload malo sí es
                    // nuestro problema → probamos siguiente igualmente.
                }
            }
            Err(e) => {
                state.engine.record_failure(&cand.provider, &cand.model);
                last_err = format!("{}/{} → {e}", cand.provider, cand.model);
                ledger(json!({"provider": cand.provider, "model": cand.model, "tier": tier, "ok": false, "err": e.to_string()}));
            }
        }
    }
    Err((StatusCode::BAD_GATEWAY, format!("todos los candidatos fallaron; último: {last_err}")))
}

async fn handle_chat(
    state: Arc<AppState>,
    headers: HeaderMap,
    client_proto: Protocol,
    body: Value,
) -> Response {
    // Semáforo global (entropía): limita concurrencia upstream.
    let _permit = match tokio::time::timeout(
        Duration::from_secs(state.settings.queue_timeout_s),
        state.semaphore.clone().acquire_owned(),
    )
    .await
    {
        Ok(Ok(p)) => p,
        _ => {
            return error_response(
                client_proto,
                StatusCode::TOO_MANY_REQUESTS,
                "cola del proxy llena; reintenta",
            )
        }
    };

    let tier_header =
        RouteEngine::tier_from_header(headers.get("x-alx-tier").and_then(|v| v.to_str().ok()));

    match attempt_upstream(&state, client_proto, &body, tier_header).await {
        Ok((up, cand)) => {
            let want_stream = body.get("stream").and_then(|v| v.as_bool()) == Some(true);
            let upstream_proto = match cand.protocol.as_str() {
                "anthropic" => Protocol::Anthropic,
                _ => Protocol::OpenAi,
            };
            let visible = state.settings.visible_model.clone();

            if want_stream && client_proto == upstream_proto {
                // Passthrough con máscara del modelo en el primer chunk.
                let um = cand.model.clone();
                let vm = visible.clone();
                let stream = up.bytes_stream().map(move |chunk| {
                    let um = um.clone();
                    let vm = vm.clone();
                    chunk.map(move |b| {
                        let (masked, _) = mask::mask_sse_chunk(&b, &um, &vm, false);
                        // nota: already_masked se reinicia por chunk; el
                        // upstream repite "model" solo en el primero, el
                        // reemplazo textual es idempotente si se repitiese.
                        masked.into_owned()
                    })
                });
                let mut resp = Response::new(Body::from_stream(stream));
                resp.headers_mut().insert(
                    "content-type",
                    "text/event-stream".parse().unwrap(),
                );
                resp.headers_mut().insert("cache-control", "no-cache".parse().unwrap());
                resp
            } else if want_stream {
                // Cross-protocol stream: upstream no-stream → SSE sintético.
                let full: Value = match up.json().await {
                    Ok(v) => v,
                    Err(e) => {
                        return error_response(
                            client_proto,
                            StatusCode::BAD_GATEWAY,
                            &format!("upstream sin body: {e}"),
                        )
                    }
                };
                let mut full = full;
                mask::mask_body(&mut full, &visible);
                let bytes = match client_proto {
                    Protocol::Anthropic => synthetic_anth_stream(&full),
                    Protocol::OpenAi => synthetic_oai_stream(&full),
                };
                let mut resp = Response::new(Body::from(bytes));
                resp.headers_mut()
                    .insert("content-type", "text/event-stream".parse().unwrap());
                resp
            } else {
                // No-stream: traducir de vuelta (o passthrough) + máscara.
                let raw: Value = match up.json().await {
                    Ok(v) => v,
                    Err(e) => {
                        return error_response(
                            client_proto,
                            StatusCode::BAD_GATEWAY,
                            &format!("upstream sin body: {e}"),
                        )
                    }
                };
                let mut out = match (client_proto, upstream_proto) {
                    (Protocol::Anthropic, Protocol::OpenAi) => {
                        translate::oai_response_to_anth(&raw, &visible)
                    }
                    (Protocol::OpenAi, Protocol::Anthropic) => {
                        translate::anth_response_to_oai(&raw, &visible)
                    }
                    _ => {
                        let mut r = raw;
                        mask::mask_body(&mut r, &visible);
                        r
                    }
                };
                if client_proto != upstream_proto {
                    mask::mask_body(&mut out, &visible);
                }
                Json(out).into_response()
            }
        }
        Err((code, msg)) => error_response(client_proto, code, &msg),
    }
}

async fn messages(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    handle_chat(state, headers, Protocol::Anthropic, body).await
}

async fn chat_completions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    handle_chat(state, headers, Protocol::OpenAi, body).await
}

/// count_tokens sin upstream: estimación 4 chars/token (lección routa #3).
async fn count_tokens(Json(body): Json<Value>) -> Response {
    let sys = body.get("system").map(|s| s.to_string()).unwrap_or_default();
    let mut total = sys.len() as u64;
    if let Some(msgs) = body.get("messages").and_then(|m| m.as_array()) {
        for m in msgs {
            total += m.to_string().len() as u64;
        }
    }
    Json(json!({"input_tokens": estimate_tokens(&format!("{sys}{total}"))})).into_response()
}

async fn health() -> Response {
    Json(json!({"ok": true, "service": "alx-proxy"})).into_response()
}

async fn proxy_status(State(state): State<Arc<AppState>>) -> Response {
    let cfg = state.engine.config();
    let providers: Vec<Value> = cfg
        .providers
        .iter()
        .map(|p| {
            json!({
                "name": p.name, "protocol": p.protocol, "base_url": p.base_url,
                "tier": p.tier, "weight": p.weight,
                "keys": p.api_keys.len(), "models": p.models,
            })
        })
        .collect();
    let breakers: Vec<Value> = state
        .engine
        .breaker_summary()
        .into_iter()
        .map(|(k, f, open)| json!({"circuit": k, "failures": f, "open": open}))
        .collect();
    Json(json!({
        "visible_model": state.settings.visible_model,
        "port": state.settings.port,
        "max_concurrency": state.settings.max_concurrency,
        "routing_dumb": cfg.routing.dumb,
        "providers": providers,
        "breakers": breakers,
    }))
    .into_response()
}

/// Arranca el servidor (127.0.0.1:port) y bloquea hasta Ctrl-C.
pub async fn serve(cfg: crate::config::ProxyConfig) {
    let settings = cfg.proxy.clone();
    let engine = RouteEngine::new(cfg);
    let conc = settings.max_concurrency;
    let port = settings.port;
    let state = Arc::new(AppState {
        engine,
        http: reqwest::Client::new(),
        semaphore: Arc::new(tokio::sync::Semaphore::new(conc)),
        settings,
    });
    let app = Router::new()
        .route("/v1/messages", post(messages))
        .route("/v1/messages/count_tokens", post(count_tokens))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/proxy/status", get(proxy_status))
        .route("/health", get(health))
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024 * 1024))
        .with_state(state);
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    eprintln!("alx-proxy: escuchando en http://{addr} (Anthropic + OpenAI)");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("serve");
}
