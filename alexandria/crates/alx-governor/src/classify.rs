//! Clasificador de dificultad: señales → score 0..1 → tier.

use alx_core::types::ModelTier;

/// Señales estructurales de dificultad (plan 09 §1). Computadas por el harness
/// antes de crear la tarea; el prompt bruto no basta para todas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassificationInput {
    /// Sin spec, sin test: +0.3.
    pub ambiguous: bool,
    /// Superficie de archivos a tocar: +0.2.
    pub file_surface: bool,
    /// Riesgo (auth, pagos, migrations): +0.3.
    pub risky: bool,
    /// Repetitivo/mechánico (search, format, rename): −0.4.
    pub mechanical: bool,
    /// Ya hay spec + tests verdes previos: −0.3.
    pub has_spec_and_tests: bool,
}

impl ClassificationInput {
    pub fn new(
        ambiguous: bool,
        file_surface: bool,
        risky: bool,
        mechanical: bool,
        has_spec_and_tests: bool,
    ) -> Self {
        Self { ambiguous, file_surface, risky, mechanical, has_spec_and_tests }
    }
}

/// Peso de cada señal según plan 09 §1.
const W_AMBIGUOUS: f64 = 0.3;
const W_FILE_SURFACE: f64 = 0.2;
const W_RISKY: f64 = 0.3;
const W_MECHANICAL: f64 = -0.4;
const W_SPEC_TESTS: f64 = -0.3;

/// Score 0..1 (clampeado) desde las señales. Positivo = más difícil/costoso.
pub fn classify(_prompt: &str, signals: &ClassificationInput) -> f64 {
    let mut score = 0.0;
    if signals.ambiguous {
        score += W_AMBIGUOUS;
    }
    if signals.file_surface {
        score += W_FILE_SURFACE;
    }
    if signals.risky {
        score += W_RISKY;
    }
    if signals.mechanical {
        score += W_MECHANICAL;
    }
    if signals.has_spec_and_tests {
        score += W_SPEC_TESTS;
    }
    score.clamp(0.0, 1.0)
}

/// Score → tier. Límites: <0.3 T1, 0.3–0.7 T2, >0.7 T3.
pub fn tier_for_score(score: f64) -> ModelTier {
    let s = score.clamp(0.0, 1.0);
    if s < 0.3 {
        ModelTier::T1Cheap
    } else if s <= 0.7 {
        ModelTier::T2Medium
    } else {
        ModelTier::T3Premium
    }
}

const RISK_KEYWORDS: [&str; 4] = ["auth", "pago", "migration", "secret"];
const MECH_KEYWORDS: [&str; 3] = ["search", "rename", "format"];

/// Heurística simple sobre el texto del prompt. Directa (no pasa por el score):
/// riesgo → T3 (seguridad manda), mechánico → T1, >500 chars → T2, default T2.
pub fn classify_prompt_text(prompt: &str) -> ModelTier {
    let lower = prompt.to_lowercase();
    if RISK_KEYWORDS.iter().any(|k| lower.contains(k)) {
        return ModelTier::T3Premium;
    }
    if MECH_KEYWORDS.iter().any(|k| lower.contains(k)) {
        return ModelTier::T1Cheap;
    }
    if prompt.len() > 500 {
        return ModelTier::T2Medium;
    }
    ModelTier::T2Medium
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Señales → score esperado (independiente de la implementación).
    fn s(ambiguous: bool, file_surface: bool, risky: bool, mechanical: bool, spec: bool) -> f64 {
        let input = ClassificationInput::new(ambiguous, file_surface, risky, mechanical, spec);
        classify("", &input)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn signals_scores() {
        assert!(close(s(false, false, false, false, false), 0.0));
        assert!(close(s(false, false, false, true, false), 0.0)); // −0.4 clampeado
        assert!(close(s(false, false, false, false, true), 0.0)); // −0.3 clampeado
        assert!(close(s(false, true, false, false, false), 0.2));
        assert!(close(s(true, false, false, false, false), 0.3));
        assert!(close(s(true, true, true, false, false), 0.8));
        assert!(close(s(true, false, true, false, false), 0.6));
        assert!(close(s(false, false, true, true, false), 0.0)); // 0.3−0.4 clampeado
        assert!(close(s(true, true, true, true, true), 0.1));
    }

    #[test]
    fn every_combination_score_in_range() {
        for ambiguous in [false, true] {
            for file_surface in [false, true] {
                for risky in [false, true] {
                    for mechanical in [false, true] {
                        for spec in [false, true] {
                            let input = ClassificationInput::new(
                                ambiguous, file_surface, risky, mechanical, spec,
                            );
                            let score = classify("", &input);
                            assert!((0.0..=1.0).contains(&score));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn tier_boundaries() {
        assert_eq!(tier_for_score(0.0), ModelTier::T1Cheap);
        assert_eq!(tier_for_score(0.29), ModelTier::T1Cheap);
        assert_eq!(tier_for_score(0.3), ModelTier::T2Medium);
        assert_eq!(tier_for_score(0.7), ModelTier::T2Medium);
        assert_eq!(tier_for_score(0.71), ModelTier::T3Premium);
        assert_eq!(tier_for_score(1.0), ModelTier::T3Premium);
        assert_eq!(tier_for_score(-5.0), ModelTier::T1Cheap);
        assert_eq!(tier_for_score(9.0), ModelTier::T3Premium);
    }

    #[test]
    fn classify_maps_signals_to_expected_tier() {
        // Ambigüedad + superficie + riesgo (0.8) → T3.
        assert_eq!(
            tier_for_score(classify("", &ClassificationInput::new(true, true, true, false, false))),
            ModelTier::T3Premium
        );
        // Ambigüedad + riesgo (0.6) → T2.
        assert_eq!(
            tier_for_score(classify("", &ClassificationInput::new(true, false, true, false, false))),
            ModelTier::T2Medium
        );
        // Sólo superficie (0.2) → T1.
        assert_eq!(
            tier_for_score(classify("", &ClassificationInput::new(false, true, false, false, false))),
            ModelTier::T1Cheap
        );
        // Mechánico + spec+tests → T1 (clampeado a 0).
        assert_eq!(
            tier_for_score(classify("", &ClassificationInput::new(false, false, false, true, true))),
            ModelTier::T1Cheap
        );
        // Default → T1.
        assert_eq!(
            tier_for_score(classify("", &ClassificationInput::new(false, false, false, false, false))),
            ModelTier::T1Cheap
        );
    }

    #[test]
    fn classify_prompt_text_cases() {
        assert_eq!(classify_prompt_text("integra el pago por tarjeta"), ModelTier::T3Premium);
        assert_eq!(classify_prompt_text("aplica la migration de la base"), ModelTier::T3Premium);
        assert_eq!(classify_prompt_text("busca secretos en el repo"), ModelTier::T3Premium);
        assert_eq!(classify_prompt_text("rename la funcion foo"), ModelTier::T1Cheap);
        assert_eq!(classify_prompt_text("formatea el codigo"), ModelTier::T1Cheap);
        assert_eq!(classify_prompt_text("search the login module"), ModelTier::T1Cheap);
    }

    #[test]
    fn classify_prompt_text_long_is_t2() {
        let long: String = "a".repeat(600);
        assert_eq!(classify_prompt_text(&long), ModelTier::T2Medium);
        assert_eq!(classify_prompt_text("corta"), ModelTier::T2Medium);
    }

    #[test]
    fn risk_beats_mechanical_in_text() {
        assert_eq!(classify_prompt_text("auth + search juntos"), ModelTier::T3Premium);
    }
}
