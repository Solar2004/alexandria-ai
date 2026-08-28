//! Server JSON-RPC 2.0 sobre stdio — subset MCP Fase 1.

use std::io::{BufRead, Write};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::catalog::ToolCatalog;

/// Versión del protocolo MCP que anuncia `initialize`.
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// Código JSON-RPC para método o tool no encontrado.
pub const METHOD_NOT_FOUND: i64 = -32601;

/// Petición JSON-RPC 2.0 entrante.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub id: serde_json::Value,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// Procesa una línea JSON-RPC contra el catálogo y devuelve la respuesta serializada.
///
/// Devuelve `None` si la línea no es JSON-RPC válido. El `id` de la petición se
/// reutiliza en la respuesta.
pub fn handle_line(catalog: &ToolCatalog, line: &str) -> Option<String> {
    let req: McpRequest = serde_json::from_str(line).ok()?;
    let id = req.id;
    let response = match req.method.as_str() {
        "initialize" => result(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "alx-mcp",
                    "version": env!("CARGO_PKG_VERSION"),
                },
            }),
        ),
        "tools/list" => result(
            id,
            json!({
                "tools": serde_json::to_value(catalog.list()).unwrap_or_default(),
            }),
        ),
        "tools/call" => {
            let name = req.params.get("name").and_then(|n| n.as_str());
            match name.and_then(|n| catalog.by_name(n)) {
                Some(tool) => result(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": format!("ok: {}", tool.name) }],
                    }),
                ),
                None => error(id, METHOD_NOT_FOUND, "tool not found"),
            }
        }
        _ => error(id, METHOD_NOT_FOUND, "method not found"),
    };
    Some(response)
}

/// Bucle de servidor stdio: lee líneas de stdin, responde por stdout.
pub fn serve_stdio(catalog: ToolCatalog) {
    let stdin = std::io::stdin();
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(response) = handle_line(&catalog, trimmed) {
            if writeln!(out, "{response}").is_err() {
                break;
            }
            let _ = out.flush();
        }
    }
}

fn result(id: serde_json::Value, result: serde_json::Value) -> String {
    serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    }))
    .expect("respuesta JSON-RPC serializable")
}

fn error(id: serde_json::Value, code: i64, message: &str) -> String {
    serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    }))
    .expect("error JSON-RPC serializable")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> ToolCatalog {
        ToolCatalog::alexandria_default()
    }

    fn parse(response: &str) -> serde_json::Value {
        serde_json::from_str(response).unwrap()
    }

    #[test]
    fn initialize_returns_protocol_version() {
        let response = handle_line(&catalog(), r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#)
            .expect("respuesta esperada");
        let v = parse(&response);
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["id"], 1);
        assert_eq!(v["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(v["result"]["capabilities"]["tools"], json!({}));
    }

    #[test]
    fn tools_list_returns_all_tools() {
        let response = handle_line(&catalog(), r#"{"jsonrpc":"2.0","id":"a","method":"tools/list"}"#)
            .expect("respuesta esperada");
        let v = parse(&response);
        assert_eq!(v["result"]["tools"].as_array().map(Vec::len), Some(13));
        assert_eq!(v["result"]["tools"][0]["name"], "task.list");
        assert_eq!(v["id"], "a");
    }

    #[test]
    fn tools_call_existing_tool_ok() {
        let response = handle_line(
            &catalog(),
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"task.list"}}"#,
        )
        .expect("respuesta esperada");
        let v = parse(&response);
        assert_eq!(v["result"]["content"][0]["type"], "text");
        assert_eq!(v["result"]["content"][0]["text"], "ok: task.list");
    }

    #[test]
    fn tools_call_missing_tool_returns_error() {
        let response = handle_line(
            &catalog(),
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"nope"}}"#,
        )
        .expect("respuesta esperada");
        let v = parse(&response);
        assert_eq!(v["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(v["error"]["message"], "tool not found");
    }

    #[test]
    fn tools_call_without_name_returns_error() {
        let response = handle_line(
            &catalog(),
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{}}"#,
        )
        .expect("respuesta esperada");
        let v = parse(&response);
        assert_eq!(v["error"]["code"], METHOD_NOT_FOUND);
    }

    #[test]
    fn unknown_method_returns_error() {
        let response = handle_line(&catalog(), r#"{"jsonrpc":"2.0","id":5,"method":"foo"}"#)
            .expect("respuesta esperada");
        let v = parse(&response);
        assert_eq!(v["error"]["code"], METHOD_NOT_FOUND);
    }

    #[test]
    fn invalid_line_returns_none() {
        assert!(handle_line(&catalog(), "esto no es json").is_none());
        assert!(handle_line(&catalog(), "").is_none());
    }
}
