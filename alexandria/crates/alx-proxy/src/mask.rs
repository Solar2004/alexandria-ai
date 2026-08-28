//! Máscara de modelo: el cliente siempre ve el modelo visible de la config,
//! nunca el modelo real del proveedor (en JSON y en el stream SSE).

use serde_json::Value;

/// Sobrescribe `model` en una respuesta JSON completa (ambos protocolos usan
/// el mismo campo).
pub fn mask_body(v: &mut Value, visible: &str) {
    if let Some(obj) = v.as_object_mut() {
        obj.insert("model".into(), Value::String(visible.into()));
    }
}

/// Reescribe el modelo dentro del primer chunk SSE que contenga `"model"`.
/// Los chunks son JSON compacto por línea; el upstream normalmente repite el
/// modelo exacto que le mandamos, así que el reemplazo textual es seguro. Si
/// no aparece, el chunk pasa intacto (fallo cosmético, no funcional).
pub fn mask_sse_chunk<'a>(
    chunk: &'a [u8],
    upstream_model: &str,
    visible: &str,
    already_masked: bool,
) -> (std::borrow::Cow<'a, [u8]>, bool) {
    if already_masked || upstream_model.is_empty() || upstream_model == visible {
        return (std::borrow::Cow::Borrowed(chunk), already_masked);
    }
    if !chunk.windows(7).any(|w| w == b"\"model\"") {
        return (std::borrow::Cow::Borrowed(chunk), already_masked);
    }
    let text = String::from_utf8_lossy(chunk);
    let a = format!("\"model\":\"{upstream_model}\"");
    let b = format!("\"model\": \"{upstream_model}\"");
    let va = format!("\"model\":\"{visible}\"");
    let vb = format!("\"model\": \"{visible}\"");
    if text.contains(&a) {
        return (std::borrow::Cow::Owned(text.replace(&a, &va).into_bytes()), true);
    }
    if text.contains(&b) {
        return (std::borrow::Cow::Owned(text.replace(&b, &vb).into_bytes()), true);
    }
    (std::borrow::Cow::Borrowed(chunk), already_masked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enmascara_body_json() {
        let mut v: Value =
            serde_json::from_str(r#"{"id":"x","model":"glm-5","content":[]}"#).unwrap();
        mask_body(&mut v, "claude-opus-4-6[1m]");
        assert_eq!(v["model"], "claude-opus-4-6[1m]");
    }

    #[test]
    fn enmascara_primer_chunk_y_despues_no_toca() {
        let c1 = br#"event: message_start
data: {"type":"message_start","message":{"model":"deepseek-v4-flash"}}

"#;
        let (out, done) = mask_sse_chunk(c1, "deepseek-v4-flash", "vis[1m]", false);
        assert!(done);
        let s = String::from_utf8_lossy(&out).to_string();
        assert!(s.contains("\"model\":\"vis[1m]\""));
        assert!(!s.contains("deepseek"));
        // segundo chunk: sin modelo → intacto
        let c2 = br#"data: {"type":"content_block_delta","delta":{"text":"hola"}}

"#;
        let (out2, done2) = mask_sse_chunk(c2, "deepseek-v4-flash", "vis[1m]", done);
        assert!(done2);
        assert_eq!(out2.as_ref(), c2);
    }

    #[test]
    fn chunk_con_modelo_espaciado_tambien_cae() {
        let c = br#"data: {"object":"chat.completion.chunk","model": "gpt-x"}

"#;
        let (out, _) = mask_sse_chunk(c, "gpt-x", "vis", false);
        assert!(String::from_utf8_lossy(&out).contains("\"model\": \"vis\""));
    }
}
