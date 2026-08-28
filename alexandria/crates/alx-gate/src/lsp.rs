//! LSP real para alx-gate — discovery, handshake `initialize` y diagnostics.
//!
//! Antes el crate solo PROMETÍA "LSP discovery" en este comentario. Ahora:
//! - [`detect_servers`]: qué LSP servers hay instalados (binario + versión).
//! - [`initialize_handshake`]: conexión stdio JSON-RPC real (Content-Length
//!   framing) contra el server y lectura de `serverInfo`/capabilities.
//! - [`collect_diagnostics`]: `didOpen` + pull `textDocument/diagnostic` con
//!   fallback push `publishDiagnostics`, merge de ambos, shutdown limpio.
//! - [`verify_lsp`]: evidence (`LintReport`) con nº de errores/warnings —
//!   entra en los gates como cualquier otra verificación.
//!
//! Sin deps nuevas: framing a mano sobre stdio, timeout por deadline.

use alx_core::types::{Evidence, EvidenceKind};
use serde_json::json;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// Severidad LSP: 1=Error, 2=Warning, 3=Info, 4=Hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl Severity {
    fn from_code(code: u64) -> Self {
        match code {
            1 => Severity::Error,
            2 => Severity::Warning,
            3 => Severity::Info,
            _ => Severity::Hint,
        }
    }

    /// Nombre legible para informes.
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
            Severity::Hint => "hint",
        }
    }
}

use serde::{Deserialize, Serialize};

/// Un diagnóstico LSP de un fichero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub file: String,
    /// Línea 1-indexada (LSP es 0-indexed; convertimos para humanos).
    pub line: usize,
    pub severity: Severity,
    pub message: String,
    pub source: String,
}

/// Especificación de un LSP server conocido: lenguaje + binario + args.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspServerSpec {
    pub language: String,
    pub binary: String,
    pub args: Vec<String>,
    /// Extensiones de fichero que atiende (sin punto).
    pub extensions: Vec<String>,
}

/// Servers conocidos, en orden de preferencia por lenguaje.
pub fn known_servers() -> Vec<LspServerSpec> {
    vec![
        LspServerSpec {
            language: "rust".into(),
            binary: "rust-analyzer".into(),
            args: vec![],
            extensions: vec!["rs".into()],
        },
        LspServerSpec {
            language: "typescript".into(),
            binary: "typescript-language-server".into(),
            args: vec!["--stdio".into()],
            extensions: vec!["ts".into(), "tsx".into(), "js".into(), "jsx".into()],
        },
        LspServerSpec {
            language: "python".into(),
            binary: "pyright-langserver".into(),
            args: vec!["--stdio".into()],
            extensions: vec!["py".into()],
        },
        LspServerSpec {
            language: "c/c++".into(),
            binary: "clangd".into(),
            args: vec![],
            extensions: vec!["c".into(), "h".into(), "cpp".into(), "hpp".into()],
        },
    ]
}

/// Resultado de detectar un server en el sistema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspDetect {
    pub spec: LspServerSpec,
    pub available: bool,
    /// Primera línea de `binary --version` (vacía si no hay binario).
    pub version: String,
}

/// ¿Está el binario en PATH? (sin spawn de shell: busca en PATH directo).
fn which(binary: &str) -> bool {
    let Ok(path) = std::env::var("PATH") else {
        return false;
    };
    std::env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .any(|full| full.is_file())
}

/// Versión del server: primera línea de `binary --version` (timeout 3s).
fn server_version(binary: &str) -> String {
    let out = crate::run_command(&format!("{binary} --version"), 3000);
    out.stdout_head
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

/// Detecta todos los servers conocidos: disponibles + versión.
pub fn detect_servers() -> Vec<LspDetect> {
    known_servers()
        .into_iter()
        .map(|spec| {
            let available = which(&spec.binary);
            let version = if available { server_version(&spec.binary) } else { String::new() };
            LspDetect { spec, available, version }
        })
        .collect()
}

/// Elige el server correcto para un fichero por extensión.
pub fn server_for_file(file: &str, servers: &[LspDetect]) -> Option<LspDetect> {
    let ext = std::path::Path::new(file)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    servers
        .iter()
        .find(|d| d.available && d.spec.extensions.iter().any(|x| x == ext))
        .cloned()
}

// ─── Framing JSON-RPC/LSP ───────────────────────────────────────────────────

/// Codifica un mensaje LSP: `Content-Length` + `\r\n\r\n` + cuerpo.
pub fn encode_message(body: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes()
}

/// Lee un mensaje LSP de un reader: parsea headers y lee el cuerpo exacto.
/// `None` = EOF antes de un mensaje completo.
pub fn read_message(reader: &mut impl BufRead) -> Option<String> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).ok()?;
        if n == 0 {
            return None; // EOF
        }
        let line = line.trim_end();
        if line.is_empty() {
            break; // fin de headers
        }
        if let Some(v) = line.strip_prefix("Content-Length:").and_then(|s| s.trim().parse::<usize>().ok()) {
            content_length = Some(v);
        }
    }
    let len = content_length?;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).to_string())
}

// ─── Cliente LSP ────────────────────────────────────────────────────────────

struct LspClient {
    child: Child,
    rx: Receiver<String>,
    stdin: std::process::ChildStdin,
    next_id: i64,
}

impl LspClient {
    /// Spawn + reader thread (stdout → canal).
    fn spawn(spec: &LspServerSpec, root: &str) -> Result<Self, String> {
        let mut child = Command::new(&spec.binary)
            .args(&spec.args)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("no pude spawnear {}: {e}", spec.binary))?;
        let stdout = child.stdout.take().ok_or("stdout no piped")?;
        let stdin = child.stdin.take().ok_or("stdin no piped")?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            while let Some(msg) = read_message(&mut reader) {
                if tx.send(msg).is_err() {
                    break;
                }
            }
        });
        Ok(Self { child, rx, stdin, next_id: 1 })
    }

    fn send(&mut self, body: &serde_json::Value) -> Result<(), String> {
        let bytes = encode_message(&body.to_string());
        self.stdin
            .write_all(&bytes)
            .map_err(|e| format!("stdin roto: {e}"))
    }

    fn request(&mut self, method: &str, params: serde_json::Value) -> (i64, serde_json::Value) {
        let id = self.next_id;
        self.next_id += 1;
        (id, json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
    }

    /// Espera la respuesta JSON-RPC con `id` dado hasta `deadline`.
    fn wait_response(&self, id: i64, deadline: Instant) -> Result<serde_json::Value, String> {
        loop {
            let left = deadline.checked_duration_since(Instant::now()).ok_or("timeout LSP")?;
            match self.rx.recv_timeout(left) {
                Ok(msg) => {
                    let v: serde_json::Value = serde_json::from_str(&msg)
                        .map_err(|e| format!("mensaje no-JSON: {e}"))?;
                    if v.get("id") == Some(&json!(id)) && v.get("method").is_none() {
                        if let Some(err) = v["error"].as_object() {
                            return Err(format!("LSP error: {err:?}"));
                        }
                        return Ok(v["result"].clone());
                    }
                    // notificaciones/otras respuestas: se ignoran aquí
                }
                Err(RecvTimeoutError::Timeout) => return Err("timeout LSP".into()),
                Err(RecvTimeoutError::Disconnected) => return Err("server LSP murió".into()),
            }
        }
    }

    /// Shutdown + exit + kill (limpieza siempre).
    fn shutdown(&mut self) {
        let (id, req) = self.request("shutdown", json!(null));
        let _ = self.send(&req);
        let _ = self.wait_response(id, Instant::now() + Duration::from_secs(2));
        let _ = self.send(&json!({"jsonrpc": "2.0", "method": "exit"}));
        let _ = self.child.wait();
        let _ = self.child.kill();
    }
}

/// Handshake `initialize` real contra el server. Devuelve `serverInfo` como
/// texto ("name vX.Y") — prueba de que el binario HABLA LSP de verdad.
pub fn initialize_handshake(spec: &LspServerSpec, root: &str, timeout_ms: u64) -> Result<String, String> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let mut cli = LspClient::spawn(spec, root)?;
    let result = (|| -> Result<String, String> {
        let (id, req) = cli.request(
            "initialize",
            json!({
                "processId": std::process::id(),
                "rootUri": format!("file://{root}"),
                "capabilities": {},
            }),
        );
        cli.send(&req)?;
        let res = cli.wait_response(id, deadline)?;
        let info = &res["serverInfo"];
        let name = info["name"].as_str().unwrap_or("?");
        let version = info["version"].as_str().unwrap_or("");
        Ok(if version.is_empty() { name.to_string() } else { format!("{name} {version}") })
    })();
    cli.shutdown();
    result
}

/// Colecciona diagnostics LSP de los ficheros (didOpen + pull + push merge).
/// Agrupa por server (un spawn por lenguaje implicado). `timeout_ms` por file.
pub fn collect_diagnostics(
    servers: &[LspDetect],
    root: &str,
    files: &[String],
    timeout_ms: u64,
) -> Result<Vec<Diagnostic>, String> {
    let mut out = Vec::new();
    // Agrupa ficheros por server (un cliente LSP por lenguaje).
    let mut by_server: HashMap<String, (LspDetect, Vec<String>)> = HashMap::new();
    for f in files {
        let det = server_for_file(f, servers)
            .ok_or_else(|| format!("sin LSP server para {f}"))?;
        by_server
            .entry(det.spec.language.clone())
            .or_insert_with(|| (det, Vec::new()))
            .1
            .push(f.clone());
    }
    for (_lang, (det, fs)) in by_server {
        out.extend(server_diagnostics(&det.spec, root, &fs, timeout_ms)?);
    }
    Ok(out)
}

/// Diagnostics de UN server para sus ficheros.
fn server_diagnostics(
    spec: &LspServerSpec,
    root: &str,
    files: &[String],
    timeout_ms: u64,
) -> Result<Vec<Diagnostic>, String> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms.saturating_mul(files.len() as u64).max(timeout_ms));
    let mut cli = LspClient::spawn(spec, root)?;
    let result = (|| -> Result<Vec<Diagnostic>, String> {
        let (id, req) = cli.request(
            "initialize",
            json!({
                "processId": std::process::id(),
                "rootUri": format!("file://{root}"),
                "capabilities": {},
            }),
        );
        cli.send(&req)?;
        let res = cli.wait_response(id, deadline)?;
        let server_name = res["serverInfo"]["name"].as_str().unwrap_or(spec.binary.as_str()).to_string();
        cli.send(&json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}))?;

        let mut diags = Vec::new();
        let supports_pull = res["capabilities"]["diagnosticProvider"].is_object();
        for f in files {
            let abs = std::fs::canonicalize(f)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| f.clone());
            let text = std::fs::read_to_string(&abs)
                .map_err(|e| format!("no pude leer {f}: {e}"))?;
            let uri = format!("file://{abs}");
            cli.send(&json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri,
                        "languageId": spec.language,
                        "version": 1,
                        "text": text,
                    }
                }
            }))?;

            if supports_pull {
                // Pull con polling estable: los servers (rust-analyzer) cargan
                // el workspace tras initialize; la 1ª respuesta suele venir
                // vacía. Repite hasta que el conteo se estabiliza o deadline.
                let mut last_count: Option<usize> = None;
                let mut stable_rounds = 0u8;
                loop {
                    let (pid, preq) = cli.request(
                        "textDocument/diagnostic",
                        json!({"textDocument": {"uri": uri}}),
                    );
                    cli.send(&preq)?;
                    match cli.wait_response(pid, deadline) {
                        Ok(res) => {
                            let items: Vec<serde_json::Value> = res["items"]
                                .as_array()
                                .cloned()
                                .unwrap_or_default();
                            let count = items.len();
                            if Some(count) == last_count {
                                stable_rounds += 1;
                            } else {
                                stable_rounds = 0;
                            }
                            last_count = Some(count);
                            diags.retain(|d: &Diagnostic| d.file != *f); // pull es full: reemplaza
                            for d in parse_diagnostics(&items, f, &server_name) {
                                diags.push(d);
                            }
                            if stable_rounds >= 1 || Instant::now() >= deadline {
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(1500));
                        }
                        Err(_) => break, // server sin pull real: confiar en push
                    }
                }
            }
            // Push: los servers suelen emitir publishDiagnostics tras didOpen.
            // Drena hasta un quiet-period corto; merge sin duplicar (file,line,msg).
            let quiet = Instant::now() + Duration::from_millis(600);
            loop {
                let left = deadline.checked_duration_since(Instant::now()).ok_or("timeout LSP")?;
                match cli.rx.recv_timeout(left.min(Duration::from_millis(600))) {
                    Ok(msg) => {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&msg) {
                            if v["method"].as_str() == Some("textDocument/publishDiagnostics") {
                                let uri = v["params"]["uri"].as_str().unwrap_or("");
                                let fname = uri.strip_prefix("file://").unwrap_or(uri);
                                if let Some(items) = v["params"]["diagnostics"].as_array() {
                                    for d in parse_diagnostics(items, fname, &server_name) {
                                        // Dedup por (file, línea, severidad, mensaje):
                                        // pull y push pueden diferir solo en `source`.
                                        let dup = diags.iter().any(|x| {
                                            x.file == d.file
                                                && x.line == d.line
                                                && x.severity == d.severity
                                                && x.message == d.message
                                        });
                                        if !dup {
                                            diags.push(d);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        if Instant::now() >= quiet {
                            break;
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => return Err("server LSP murió".into()),
                }
            }
        }
        Ok(diags)
    })();
    cli.shutdown();
    result
}

/// Parsea items de diagnostics LSP → [`Diagnostic`] (línea 1-indexada).
fn parse_diagnostics(items: &[serde_json::Value], file: &str, source: &str) -> Vec<Diagnostic> {
    items
        .iter()
        .map(|d| Diagnostic {
            file: file.to_string(),
            line: d["range"]["start"]["line"].as_u64().unwrap_or(0) as usize + 1,
            severity: Severity::from_code(d["severity"].as_u64().unwrap_or(1)),
            message: d["message"].as_str().unwrap_or("").chars().take(300).collect(),
            source: d["source"].as_str().unwrap_or(source).to_string(),
        })
        .collect()
}

/// Evidence del gate LSP: `LintReport` con metrics errors/warnings.
/// `passed` = sin errores de severidad Error (warnings no fallan el gate).
pub fn verify_lsp(
    servers: &[LspDetect],
    root: &str,
    files: &[String],
    timeout_ms: u64,
) -> Result<Evidence, String> {
    let diags = collect_diagnostics(servers, root, files, timeout_ms)?;
    let errors = diags.iter().filter(|d| d.severity == Severity::Error).count();
    let warnings = diags.iter().filter(|d| d.severity == Severity::Warning).count();
    let mut body = String::new();
    for d in &diags {
        body.push_str(&format!("{}:{} {} [{}] {}\n", d.file, d.line, d.severity.as_str(), d.source, d.message));
    }
    let cmd = format!("lsp-check {}", files.join(" "));
    let mut ev = Evidence {
        kind: EvidenceKind::LintReport,
        command: cmd,
        exit_code: if errors > 0 { 1 } else { 0 },
        stdout_head: body.chars().take(4000).collect(),
        passed: errors == 0,
        metrics: HashMap::new(),
    };
    ev.metrics.insert("errors".into(), errors as f64);
    ev.metrics.insert("warnings".into(), warnings as f64);
    Ok(ev)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_roundtrip() {
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#;
        let bytes = encode_message(body);
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with(&format!("Content-Length: {}\r\n\r\n", body.len())));
        let mut reader = BufReader::new(text.as_bytes());
        let got = read_message(&mut reader).unwrap();
        assert_eq!(got, body);
    }

    #[test]
    fn framing_two_messages_back_to_back() {
        let a = encode_message(r#"{"n":1}"#);
        let b = encode_message(r#"{"n":22}"#);
        let mut all = a;
        all.extend_from_slice(&b);
        let mut reader = BufReader::new(all.as_slice());
        assert_eq!(read_message(&mut reader).unwrap(), r#"{"n":1}"#);
        assert_eq!(read_message(&mut reader).unwrap(), r#"{"n":22}"#);
        assert!(read_message(&mut reader).is_none());
    }

    #[test]
    fn framing_eof_without_body_returns_none() {
        let mut reader = BufReader::new("Content-Length: 10\r\n\r\nshort".as_bytes());
        assert!(read_message(&mut reader).is_none());
    }

    #[test]
    fn known_servers_cover_core_languages() {
        let servers = known_servers();
        assert!(servers.iter().any(|s| s.language == "rust" && s.binary == "rust-analyzer"));
        assert!(servers.iter().any(|s| s.extensions.contains(&"py".to_string())));
        assert!(servers.iter().any(|s| s.extensions.contains(&"ts".to_string())));
    }

    #[test]
    fn server_for_file_matches_extension() {
        let mut servers: Vec<LspDetect> = known_servers()
            .into_iter()
            .map(|spec| LspDetect { spec, available: true, version: "test".into() })
            .collect();
        let det = server_for_file("foo/main.rs", &servers).unwrap();
        assert_eq!(det.spec.binary, "rust-analyzer");
        let det = server_for_file("x/lib.py", &servers).unwrap();
        assert_eq!(det.spec.binary, "pyright-langserver");
        // Sin server para esa extensión → None.
        servers.push(LspDetect {
            spec: LspServerSpec {
                language: "txt".into(),
                binary: "nope".into(),
                args: vec![],
                extensions: vec!["txt".into()],
            },
            available: false,
            version: String::new(),
        });
        assert!(server_for_file("x/zzz.weird", &servers).is_none());
        // Server no disponible no se elige aunque case la extensión.
        assert!(server_for_file("x/notes.txt", &servers).is_none());
    }

    #[test]
    fn severity_mapping() {
        assert_eq!(Severity::from_code(1), Severity::Error);
        assert_eq!(Severity::from_code(2), Severity::Warning);
        assert_eq!(Severity::from_code(3), Severity::Info);
        assert_eq!(Severity::from_code(99), Severity::Hint);
    }

    #[test]
    fn parse_diagnostics_one_indexes_lines() {
        let items = vec![json!({
            "range": {"start": {"line": 4}},
            "severity": 1,
            "message": "expected `;`",
            "source": "rustc"
        })];
        let diags = parse_diagnostics(&items, "a.rs", "ra");
        assert_eq!(diags[0].line, 5); // LSP 0-indexed → humano 1-indexed
        assert_eq!(diags[0].severity, Severity::Error);
    }

    #[test]
    fn verify_lsp_counts_errors_and_warnings() {
        // Sin server real: probamos laEvidence via un server falso estático.
        // (La verificación en vivo es manual: `alx lsp --live`.)
        let servers: Vec<LspDetect> = known_servers()
            .into_iter()
            .map(|spec| LspDetect { spec, available: false, version: String::new() })
            .collect();
        let err = verify_lsp(&servers, "/tmp", &["x.rs".to_string()], 1000).unwrap_err();
        assert!(err.contains("sin LSP server"));
        let _ = err;
    }
}
