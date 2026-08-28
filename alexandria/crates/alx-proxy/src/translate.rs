//! Traducción Anthropic ↔ OpenAI.
//!
//! El proxy acepta ambos protocolos en el mismo puerto y habla el protocolo
//! que cada proveedor pida. Cuatro direcciones:
//! - req Anth→OAI / resp OAI→Anth  (cliente Anthropic, proveedor OpenAI)
//! - req OAI→Anth / resp Anth→OAI  (cliente OpenAI, proveedor Anthropic)
//!
//! (cliente y proveedor del mismo protocolo = passthrough sin traducir).

use serde_json::{json, Value};

/// Extrae el texto del usuario de un body (Anthropic u OpenAI) para
/// clasificar la tarea. Concatena el último mensaje de usuario, acotado.
pub fn extract_user_text(body: &Value, protocol: crate::Protocol) -> String {
    let msgs = match protocol {
        crate::Protocol::Anthropic => body.get("messages"),
        crate::Protocol::OpenAi => body.get("messages"),
    };
    let Some(msgs) = msgs.and_then(|m| m.as_array()) else {
        return String::new();
    };
    let mut text = String::new();
    for m in msgs.iter().rev() {
        if m.get("role").and_then(|r| r.as_str()) != Some("user") {
            continue;
        }
        match m.get("content") {
            Some(Value::String(s)) => {
                text.push_str(s);
                break;
            }
            Some(Value::Array(blocks)) => {
                for b in blocks {
                    if b.get("type").and_then(|t| t.as_str()) == Some("text") {
                        if let Some(t) = b.get("text").and_then(|t| t.as_str()) {
                            text.push_str(t);
                        }
                    }
                }
                break;
            }
            _ => {}
        }
    }
    text.chars().take(4000).collect()
}

fn content_to_oai(content: &Value) -> Value {
    match content {
        Value::String(s) => json!(s),
        Value::Array(blocks) => {
            let mut out = String::new();
            for b in blocks {
                if b.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(t) = b.get("text").and_then(|t| t.as_str()) {
                        out.push_str(t);
                    }
                }
            }
            json!(out)
        }
        other => other.clone(),
    }
}

// ─────────────────── request Anthropic → OpenAI ───────────────────

/// Convierte un request Anthropic Messages al formato chat/completions.
/// `system` (string o bloques) → mensaje system; user/assistant → roles.
pub fn anth_request_to_oai(anth: &Value, model: &str) -> Value {
    let mut messages: Vec<Value> = Vec::new();
    match anth.get("system") {
        Some(Value::String(s)) if !s.is_empty() => {
            messages.push(json!({"role": "system", "content": s}));
        }
        Some(Value::Array(blocks)) => {
            let sys = content_to_oai(&Value::Array(blocks.clone()));
            if sys.as_str().map(|s| !s.is_empty()).unwrap_or(false) {
                messages.push(json!({"role": "system", "content": sys}));
            }
        }
        _ => {}
    }
    if let Some(msgs) = anth.get("messages").and_then(|m| m.as_array()) {
        for m in msgs {
            let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            if role != "user" && role != "assistant" {
                continue;
            }
            messages.push(json!({
                "role": role,
                "content": content_to_oai(m.get("content").unwrap_or(&json!(""))),
            }));
        }
    }
    let mut out = json!({
        "model": model,
        "messages": messages,
    });
    if let Some(mt) = anth.get("max_tokens").and_then(|v| v.as_u64()) {
        out["max_tokens"] = json!(mt);
    }
    if let Some(t) = anth.get("temperature").and_then(|v| v.as_f64()) {
        out["temperature"] = json!(t);
    }
    if anth.get("stream").and_then(|v| v.as_bool()) == Some(true) {
        out["stream"] = json!(true);
    }
    out
}

// ─────────────────── request OpenAI → Anthropic ───────────────────

/// Convierte un request chat/completions al formato Messages. Los mensajes
/// system van al campo `system`; max_tokens default 1024 (Anthropic lo exige).
pub fn oai_request_to_anth(oai: &Value, model: &str) -> Value {
    let mut system = String::new();
    let mut messages: Vec<Value> = Vec::new();
    if let Some(msgs) = oai.get("messages").and_then(|m| m.as_array()) {
        for m in msgs {
            let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let content = content_to_oai(m.get("content").unwrap_or(&json!("")));
            let text = content.as_str().unwrap_or("").to_string();
            match role {
                "system" | "developer" if !text.is_empty() => {
                    if !system.is_empty() {
                        system.push('\n');
                    }
                    system.push_str(&text);
                }
                "user" | "assistant" => messages.push(json!({"role": role, "content": text})),
                _ => {}
            }
        }
    }
    let mut out = json!({
        "model": model,
        "max_tokens": oai.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(1024),
        "messages": messages,
    });
    if !system.is_empty() {
        out["system"] = json!(system);
    }
    if let Some(t) = oai.get("temperature").and_then(|v| v.as_f64()) {
        out["temperature"] = json!(t);
    }
    if oai.get("stream").and_then(|v| v.as_bool()) == Some(true) {
        out["stream"] = json!(true);
    }
    out
}

// ─────────────────────── respuestas (no-stream) ───────────────────────

/// Respuesta OpenAI → formato Anthropic Messages.
pub fn oai_response_to_anth(oai: &Value, model: &str) -> Value {
    let choice = oai
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .cloned()
        .unwrap_or(json!({}));
    let text = choice
        .get("message")
        .and_then(|m| m.get("content"))
        .map(content_to_oai)
        .and_then(|c| c.as_str().map(String::from))
        .unwrap_or_default();
    let finish = choice
        .get("finish_reason")
        .and_then(|f| f.as_str())
        .unwrap_or("stop");
    let stop_reason = match finish {
        "length" => "max_tokens",
        "tool_calls" => "tool_use",
        "content_filter" => "refusal",
        _ => "end_turn",
    };
    let usage = oai.get("usage").cloned().unwrap_or(json!({}));
    json!({
        "id": oai.get("id").cloned().unwrap_or(json!("msg-alx")),
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": [{"type": "text", "text": text}],
        "stop_reason": stop_reason,
        "usage": {
            "input_tokens": usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
            "output_tokens": usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        }
    })
}

/// Respuesta Anthropic Messages → formato OpenAI chat/completions.
pub fn anth_response_to_oai(anth: &Value, model: &str) -> Value {
    let text = anth
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
    let stop = anth
        .get("stop_reason")
        .and_then(|s| s.as_str())
        .unwrap_or("end_turn");
    let finish = match stop {
        "max_tokens" => "length",
        "tool_use" => "tool_calls",
        _ => "stop",
    };
    let usage = anth.get("usage").cloned().unwrap_or(json!({}));
    json!({
        "id": anth.get("id").cloned().unwrap_or(json!("chatcmpl-alx")),
        "object": "chat.completion",
        "model": model,
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": text},
            "finish_reason": finish,
        }],
        "usage": {
            "prompt_tokens": usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
            "completion_tokens": usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Protocol;

    #[test]
    fn extrae_texto_de_ultimo_user() {
        let anth = json!({"messages": [
            {"role": "user", "content": "hola"},
            {"role": "assistant", "content": "hey"},
            {"role": "user", "content": [{"type": "text", "text": "la pregunta final"}]},
        ]});
        assert_eq!(extract_user_text(&anth, Protocol::Anthropic), "la pregunta final");
        let oai = json!({"messages": [
            {"role": "system", "content": "sys"},
            {"role": "user", "content": "pregunta oai"},
        ]});
        assert_eq!(extract_user_text(&oai, Protocol::OpenAi), "pregunta oai");
    }

    #[test]
    fn anth_request_a_oai_preserva_system_y_roles() {
        let anth = json!({
            "system": "eres un linter",
            "max_tokens": 512,
            "temperature": 0.2,
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "hola"}]},
                {"role": "assistant", "content": "qué tal"},
            ]
        });
        let oai = anth_request_to_oai(&anth, "gpt-x");
        assert_eq!(oai["model"], "gpt-x");
        assert_eq!(oai["max_tokens"], 512);
        assert_eq!(oai["messages"][0]["role"], "system");
        assert_eq!(oai["messages"][0]["content"], "eres un linter");
        assert_eq!(oai["messages"][1]["content"], "hola"); // bloques → string
        assert_eq!(oai["messages"][2]["role"], "assistant");
    }

    #[test]
    fn oai_request_a_anth_fusiona_system_y_pone_max_tokens() {
        let oai = json!({
            "messages": [
                {"role": "system", "content": "s1"},
                {"role": "system", "content": "s2"},
                {"role": "user", "content": "pregunta"},
            ]
        });
        let anth = oai_request_to_anth(&oai, "claude-x");
        assert_eq!(anth["model"], "claude-x");
        assert_eq!(anth["system"], "s1\ns2");
        assert_eq!(anth["max_tokens"], 1024); // default obligatorio
        assert_eq!(anth["messages"][0]["role"], "user");
    }

    #[test]
    fn respuestas_giran_en_ambas_direcciones() {
        let oai = json!({
            "id": "c1", "model": "gpt-x",
            "choices": [{"message": {"role": "assistant", "content": "respuesta"},
                         "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}
        });
        let anth = oai_response_to_anth(&oai, "visible");
        assert_eq!(anth["type"], "message");
        assert_eq!(anth["content"][0]["text"], "respuesta");
        assert_eq!(anth["usage"]["input_tokens"], 10);
        assert_eq!(anth["usage"]["output_tokens"], 5);

        let anth2 = json!({
            "type": "message", "model": "claude-x",
            "content": [{"type": "text", "text": "hola de vuelta"}],
            "stop_reason": "max_tokens",
            "usage": {"input_tokens": 7, "output_tokens": 3}
        });
        let oai2 = anth_response_to_oai(&anth2, "visible");
        assert_eq!(oai2["choices"][0]["message"]["content"], "hola de vuelta");
        assert_eq!(oai2["choices"][0]["finish_reason"], "length");
        assert_eq!(oai2["usage"]["prompt_tokens"], 7);
    }
}
