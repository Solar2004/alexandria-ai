//! Ledger de coste — registro append de tokens/coste por micro-tarea.
//!
//! Cuando el pipeline ejecuta una micro-tarea contra la cadena real
//! (headroom→mask→routatic), cada llamada se registra aquí: tokens de entrada
//! y salida, coste estimado y latencia. El ledger alimenta el cost-report del
//! governor (plan 09 §4).

use alx_core::types::ModelTier;
use serde::{Deserialize, Serialize};

/// Entrada del ledger para una llamada a modelo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub task_id: String,
    pub micro_task: String,
    pub tier: ModelTier,
    /// Cadena de red usada (headroom→mask→routatic, o routatic directo).
    pub chain: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cost_usd: f64,
    pub latency_ms: u128,
}

impl LedgerEntry {
    /// Crea la entrada y calcula el coste estimado con `estimate_cost_usd`.
    pub fn new(
        task_id: impl Into<String>,
        micro_task: impl Into<String>,
        tier: ModelTier,
        chain: impl Into<String>,
        input_tokens: u32,
        output_tokens: u32,
        latency_ms: u128,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            micro_task: micro_task.into(),
            tier,
            chain: chain.into(),
            input_tokens,
            output_tokens,
            cost_usd: estimate_cost_usd(input_tokens, output_tokens),
            latency_ms,
        }
    }
}

/// Coste estimado por millón de tokens (placeholder: precios deepseek aprox,
/// locales). Ajustable cuando el governor lea la tarifa real de la cadena.
pub fn estimate_cost_usd(input_tokens: u32, output_tokens: u32) -> f64 {
    const INPUT_PER_M: f64 = 0.20; // $/1M tokens input
    const OUTPUT_PER_M: f64 = 1.00; // $/1M tokens output
    (input_tokens as f64 * INPUT_PER_M + output_tokens as f64 * OUTPUT_PER_M) / 1_000_000.0
}

/// Registro append de costes de la sesión.
#[derive(Debug, Default, Clone)]
pub struct Ledger {
    entries: Vec<LedgerEntry>,
}

impl Ledger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra una entrada al final (append-only).
    pub fn record(&mut self, entry: LedgerEntry) {
        self.entries.push(entry);
    }

    pub fn entries(&self) -> &[LedgerEntry] {
        &self.entries
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Total de tokens (input, output) de todas las entradas.
    pub fn total_tokens(&self) -> (u32, u32) {
        self.entries.iter().fold((0, 0), |(i, o), e| {
            (i.saturating_add(e.input_tokens), o.saturating_add(e.output_tokens))
        })
    }

    /// Coste total estimado en USD.
    pub fn total_cost_usd(&self) -> f64 {
        self.entries.iter().map(|e| e.cost_usd).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(input: u32, output: u32) -> LedgerEntry {
        LedgerEntry::new(
            "t-1",
            "micro-1",
            ModelTier::T2Medium,
            "headroom→mask→routatic",
            input,
            output,
            1500,
        )
    }

    #[test]
    fn cost_estimate_math() {
        // 1M input + 1M output = $1.20
        let c = estimate_cost_usd(1_000_000, 1_000_000);
        assert!((c - 1.20).abs() < 1e-9);
        // 0 tokens = 0 coste
        assert_eq!(estimate_cost_usd(0, 0), 0.0);
    }

    #[test]
    fn ledger_records_and_sums() {
        let mut l = Ledger::new();
        l.record(entry(100, 50));
        l.record(entry(200, 100));
        assert_eq!(l.entry_count(), 2);
        assert_eq!(l.total_tokens(), (300, 150));
        let expected = estimate_cost_usd(300, 150);
        assert!((l.total_cost_usd() - expected).abs() < 1e-12);
    }

    #[test]
    fn empty_ledger() {
        let l = Ledger::new();
        assert_eq!(l.entry_count(), 0);
        assert_eq!(l.total_tokens(), (0, 0));
        assert_eq!(l.total_cost_usd(), 0.0);
    }
}
