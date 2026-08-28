//! alx-proxy — EL proxy de Alexandria (único, multi-proveedor).
//!
//! Punto de entrada de Claude Code y de cualquier cliente Anthropic/OpenAI:
//!
//! - **Un solo puerto** con ambos protocolos: `/v1/messages` (Anthropic) y
//!   `/v1/chat/completions` (OpenAI), traduciendo entre formatos según el
//!   protocolo del proveedor elegido.
//! - **Pool de api-keys** por proveedor con rotación round-robin.
//! - **Rotación de modelos** por proveedor y **failover entre proveedores**
//!   ante 429/5xx, con circuit-breaker (3 fallos → abierto 120 s).
//! - **Routing inteligente por tarea**: reutiliza el clasificador del
//!   governor (riesgo → premium, mecánico → cheap, tamaño → medium) para
//!   elegir proveedor/tier, y respeta `X-Alx-Tier` si el cliente lo manda.
//! - **Máscara de modelo**: el cliente ve `claude-opus-4-6[1m]`; upstream va
//!   el modelo real del proveedor (en JSON y en el primer chunk del stream).
//! - **Ledger**: cada intento queda en `proxy-ledger.jsonl` (feeds de weekly).

pub mod config;
pub mod mask;
pub mod route;
pub mod translate;

pub mod server;

pub use config::{ProxyConfig, Provider};
pub use route::{Candidate, RouteEngine};

/// Protocolo de conversación del cliente o del proveedor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// Anthropic Messages API (/v1/messages).
    Anthropic,
    /// OpenAI Chat Completions (/v1/chat/completions).
    OpenAi,
}

impl Protocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Protocol::Anthropic => "anthropic",
            Protocol::OpenAi => "openai",
        }
    }
}

/// Estima tokens de un texto (regla 4 chars/token, suficiente para
/// count_tokens sin gastar generación upstream — lección routa #3).
pub fn estimate_tokens(text: &str) -> u64 {
    (text.len() as u64 / 4).max(1)
}
