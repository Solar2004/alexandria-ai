//! alx-critic — auto-crítica + iteration loop por hook.
//!
//! El critic revisa la salida de cada fase contra criterios; el iteration loop
//! (R24) obliga a volver a trabajar cuando el hook `iterate.trigger` dispara
//! `IterateRequest(iter, feedback)`. Spec: plan/15-critic.md.

use alx_core::types::Event;
use serde::{Deserialize, Serialize};

/// Estado de iteración de una tarea. Persistido en
/// `state/iteration-state.toml` por el hook iterate.trigger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationState {
    pub task_id: String,
    pub iter: u32,
    pub max_iter: u32,
    /// Feedback acumulado de las iteraciones previas (qué falló / qué mejorar).
    pub feedback: Vec<String>,
    /// El critic aprobó el trabajo → bucle termina.
    pub passed: bool,
}

impl IterationState {
    pub fn new(task_id: impl Into<String>, max_iter: u32) -> Self {
        Self {
            task_id: task_id.into(),
            iter: 0,
            max_iter,
            feedback: Vec::new(),
            passed: false,
        }
    }

    /// ¿Debe el sistema seguir iterando?
    pub fn should_iterate(&self) -> bool {
        !self.passed && self.iter < self.max_iter
    }

    /// Registra una iteración más con su feedback.
    pub fn advance(&mut self, feedback: impl Into<String>) {
        self.iter += 1;
        self.feedback.push(feedback.into());
    }

    /// Marca el trabajo como aprobado → no más iteraciones.
    pub fn mark_passed(&mut self) {
        self.passed = true;
    }

    /// Genera el evento que el hook emite para volver a trabajar.
    pub fn next_request(&self) -> Event {
        Event::IterateRequest(self.iter + 1, self.feedback.clone())
    }
}

/// Convierte el estado en el prompt de iteración: feedback acumulado como
/// instrucción de mejora para la siguiente pasada.
pub fn iteration_prompt(state: &IterationState) -> String {
    if state.feedback.is_empty() {
        format!(
            "Iteración {} de {}. Revisa el trabajo, verifica y mejora antes de terminar.",
            state.iter + 1,
            state.max_iter
        )
    } else {
        format!(
            "Iteración {} de {}. Feedback de iteraciones previas que debes corregir:\n- {}",
            state.iter + 1,
            state.max_iter,
            state.feedback.join("\n- ")
        )
    }
}

// ---------------------------------------------------------------------------
// Critic real: evalúa la salida de una fase contra criterios usando la cadena
// LLM (headroom→mask→routatic→deepseek). Fail-closed: cualquier fallo en la
// llamada o en el parseo del JSON → approved=false con finding Block.
// ---------------------------------------------------------------------------

/// Gravedad de un hallazgo del crítico. `Block`/`Major` bloquean la fase;
/// `Minor`/`Suggestion` son mejoras opcionales.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Block,
    Major,
    Minor,
    Suggestion,
}

/// Un hallazgo del crítico: qué falló (o qué mejorar) y con qué gravedad.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub severity: Severity,
    pub message: String,
}

/// Veredicto del crítico sobre la salida de una fase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CriticVerdict {
    pub approved: bool,
    pub findings: Vec<Finding>,
}

/// Endpoint de la cadena LLM real (headroom → mask → routatic → deepseek).
const LLM_URL: &str = "http://127.0.0.1:8788/v1/messages";

/// Construye el prompt que pide al modelo evaluar `output` contra `criteria`.
/// El modelo debe responder SOLO JSON, sin texto adicional.
pub fn critic_prompt(output: &str, criteria: &[&str]) -> String {
    let mut s = String::from("Eres el crítico de calidad. Evalúa la salida contra estos criterios:\n");
    for c in criteria {
        s.push_str(&format!("- {c}\n"));
    }
    s.push_str("\n<output>\n");
    s.push_str(output);
    s.push_str("\n</output>\n\n");
    s.push_str(
        "Responde SOLO con JSON, sin texto adicional, en este formato exacto:\n\
         {\"approved\":true/false,\"findings\":[{\"severity\":\"Block|Major|Minor|Suggestion\",\"message\":\"...\"}]}\n",
    );
    s
}

/// Parse del JSON del crítico. Severities case-insensitive; desconocida →
/// `Suggestion`. Un finding `Block` siempre fuerza `approved=false`. Si el
/// texto no parsea → fail-closed (`approved=false` con finding Block).
pub fn parse_verdict(json: &str) -> CriticVerdict {
    let slice = extract_json(json);
    let raw: RawVerdict = match serde_json::from_str(slice) {
        Ok(v) => v,
        Err(_) => return fail_closed(),
    };
    let findings: Vec<Finding> = raw
        .findings
        .into_iter()
        .map(|f| Finding {
            severity: severity_from_str(&f.severity),
            message: f.message,
        })
        .collect();
    let blocked = findings.iter().any(|f| f.severity == Severity::Block);
    CriticVerdict {
        approved: raw.approved && !blocked,
        findings,
    }
}

/// Critica contra la cadena real: construye el prompt, lo envía por curl a
/// headroom y extrae `content[0].text` de la respuesta Anthropic. Fail-closed.
pub fn criticize_real(output: &str, criteria: &[&str]) -> CriticVerdict {
    let prompt = critic_prompt(output, criteria);
    let body = serde_json::json!({
        "model": "deepseek-v4-flash",
        "max_tokens": 200,
        "messages": [{ "role": "user", "content": prompt }]
    })
    .to_string();
    let cmd = format!(
        "curl -s -m 30 {LLM_URL} -H 'content-type: application/json' -d {}",
        shell_single_quote(&body)
    );
    let out = alx_gate::run_command(&cmd, 35_000);
    if out.exit_code != 0 {
        return fail_closed();
    }
    let value: serde_json::Value = match serde_json::from_str(&out.stdout_head) {
        Ok(v) => v,
        Err(_) => return fail_closed(),
    };
    let text = value
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|b| b.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");
    if text.is_empty() {
        return fail_closed();
    }
    parse_verdict(text)
}

/// Convierte un hallazgo en un `must_check` aprendido: `"no <message>"` para
/// Block/Major (prohibición), `"considera: <message>"` para Minor/Suggestion.
pub fn learn_from_failure(finding: &Finding) -> String {
    match finding.severity {
        Severity::Block | Severity::Major => format!("no {}", finding.message),
        Severity::Minor | Severity::Suggestion => format!("considera: {}", finding.message),
    }
}

/// Deriva los `must_check` de una lista de hallazgos: uno por finding,
/// deduplicado (HashSet, orden estable de aparición) y máximo 10.
pub fn derive_must_checks(findings: &[Finding]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut checks = Vec::new();
    for f in findings {
        let c = learn_from_failure(f);
        if seen.insert(c.clone()) {
            checks.push(c);
        }
        if checks.len() >= 10 {
            break;
        }
    }
    checks
}

fn fail_closed() -> CriticVerdict {
    CriticVerdict {
        approved: false,
        findings: vec![Finding {
            severity: Severity::Block,
            message: "respuesta del critico inválida".to_string(),
        }],
    }
}

fn severity_from_str(s: &str) -> Severity {
    match s.trim().to_ascii_lowercase().as_str() {
        "block" => Severity::Block,
        "major" => Severity::Major,
        "minor" => Severity::Minor,
        "suggestion" => Severity::Suggestion,
        _ => Severity::Suggestion,
    }
}

/// Extrae el primer `{...}` balanceado por posición (tolerante a fences
/// markdown o prosa alrededor). Si no hay par de llaves, devuelve el texto
/// entero para que serde falle → fail-closed.
fn extract_json(s: &str) -> &str {
    match (s.find('{'), s.rfind('}')) {
        (Some(start), Some(end)) if end >= start => &s[start..end + 1],
        _ => s,
    }
}

/// Envuelve un string en comillas simples de shell, escapando las `'` internas
/// con `'\''` para poder incrustarlo en `-d '...'` sin romper el comando.
fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[derive(Deserialize)]
struct RawVerdict {
    #[serde(default)]
    approved: bool,
    #[serde(default)]
    findings: Vec<RawFinding>,
}

#[derive(Deserialize)]
struct RawFinding {
    #[serde(default)]
    severity: String,
    #[serde(default)]
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_iterates() {
        let s = IterationState::new("t1", 3);
        assert_eq!(s.iter, 0);
        assert!(s.should_iterate());
        assert!(!s.passed);
    }

    #[test]
    fn advance_accumulates_feedback() {
        let mut s = IterationState::new("t1", 3);
        s.advance("build falla: warning de tipo no usado");
        s.advance("lint: falta doc en funcion nueva");
        assert_eq!(s.iter, 2);
        assert_eq!(s.feedback.len(), 2);
        assert!(s.should_iterate());
    }

    #[test]
    fn stops_at_max_iter() {
        let mut s = IterationState::new("t1", 2);
        s.advance("feedback 1");
        s.advance("feedback 2");
        assert_eq!(s.iter, 2);
        assert!(!s.should_iterate()); // max alcanzado
    }

    #[test]
    fn passed_stops_iterating() {
        let mut s = IterationState::new("t1", 5);
        s.mark_passed();
        assert!(!s.should_iterate());
    }

    #[test]
    fn next_request_carries_feedback() {
        let mut s = IterationState::new("t1", 3);
        s.advance("fix X");
        match s.next_request() {
            Event::IterateRequest(iter, fb) => {
                assert_eq!(iter, 2);
                assert_eq!(fb, vec!["fix X".to_string()]);
            }
            other => panic!("evento inesperado: {other:?}"),
        }
    }

    #[test]
    fn iteration_prompt_includes_prior_feedback() {
        let mut s = IterationState::new("t1", 3);
        s.advance("corrige el nombre de la variable");
        let p = iteration_prompt(&s);
        assert!(p.contains("Iteración 2 de 3"));
        assert!(p.contains("corrige el nombre de la variable"));
    }

    #[test]
    fn iteration_prompt_without_feedback() {
        let s = IterationState::new("t1", 3);
        let p = iteration_prompt(&s);
        assert!(p.contains("Iteración 1 de 3"));
        assert!(p.contains("verifica y mejora"));
    }

    #[test]
    fn critic_prompt_includes_criteria_and_asks_json() {
        let criteria = ["build sin warnings", "sin secrets hardcodeados"];
        let p = critic_prompt("salida de la fase", &criteria);
        assert!(p.contains("build sin warnings"));
        assert!(p.contains("sin secrets hardcodeados"));
        assert!(p.contains("JSON"));
        assert!(p.contains("approved"));
        assert!(p.contains("severity"));
    }

    #[test]
    fn parse_verdict_approved_with_no_findings() {
        let v = parse_verdict(r#"{"approved":true,"findings":[]}"#);
        assert!(v.approved);
        assert!(v.findings.is_empty());
    }

    #[test]
    fn parse_verdict_block_finding_forces_reject() {
        let json = r#"{"approved":true,"findings":[{"severity":"Block","message":"criterio no cumplido"}]}"#;
        let v = parse_verdict(json);
        assert!(!v.approved);
        assert_eq!(v.findings.len(), 1);
        assert_eq!(v.findings[0].severity, Severity::Block);
        assert_eq!(v.findings[0].message, "criterio no cumplido");
    }

    #[test]
    fn parse_verdict_severity_case_insensitive() {
        let json = r#"{"approved":false,"findings":[{"severity":"major","message":"fix mayor"}]}"#;
        let v = parse_verdict(json);
        assert_eq!(v.findings[0].severity, Severity::Major);
    }

    #[test]
    fn parse_verdict_unknown_severity_falls_to_suggestion() {
        let json = r#"{"approved":false,"findings":[{"severity":"catastrofico","message":"?"}]}"#;
        let v = parse_verdict(json);
        assert_eq!(v.findings[0].severity, Severity::Suggestion);
    }

    #[test]
    fn parse_verdict_garbage_is_fail_closed() {
        let v = parse_verdict("esto no es json en absoluto");
        assert!(!v.approved);
        assert_eq!(v.findings.len(), 1);
        assert_eq!(v.findings[0].severity, Severity::Block);
        assert_eq!(v.findings[0].message, "respuesta del critico inválida");
    }

    #[test]
    fn parse_verdict_tolerates_markdown_fences() {
        let json = "```json\n{\"approved\":true,\"findings\":[]}\n```";
        let v = parse_verdict(json);
        assert!(v.approved);
    }

    #[test]
    fn learn_from_failure_block_major_are_prohibitions() {
        let block = Finding { severity: Severity::Block, message: "hardcodees secrets".into() };
        let major = Finding { severity: Severity::Major, message: "dejes el build roto".into() };
        assert_eq!(learn_from_failure(&block), "no hardcodees secrets");
        assert_eq!(learn_from_failure(&major), "no dejes el build roto");
    }

    #[test]
    fn learn_from_failure_minor_suggestion_are_considerations() {
        let minor = Finding { severity: Severity::Minor, message: "documentes la funcion".into() };
        let sugg = Finding { severity: Severity::Suggestion, message: "renombrar la variable".into() };
        assert_eq!(learn_from_failure(&minor), "considera: documentes la funcion");
        assert_eq!(learn_from_failure(&sugg), "considera: renombrar la variable");
    }

    #[test]
    fn derive_must_checks_dedup_and_caps_at_10() {
        let mut findings = Vec::new();
        for i in 0..15 {
            findings.push(Finding {
                severity: if i % 2 == 0 { Severity::Block } else { Severity::Major },
                message: format!("fallo numero {i}"),
            });
        }
        // Duplicados exactos (misma severidad + mensaje) → deben deduplicarse.
        findings.push(Finding { severity: Severity::Block, message: "fallo numero 0".into() });
        let checks = derive_must_checks(&findings);
        assert!(checks.len() <= 10, "cap a 10, llego a {}", checks.len());
        let mut uniq = std::collections::HashSet::new();
        for c in &checks {
            assert!(uniq.insert(c.clone()), "check duplicado: {c}");
        }
        // Los primeros se conservan (orden estable), son los primeros 10 únicos.
        assert!(checks.contains(&"no fallo numero 0".to_string()));
        assert_eq!(checks[0], "no fallo numero 0");
    }

    #[test]
    fn derive_must_checks_empty_input() {
        assert!(derive_must_checks(&[]).is_empty());
    }
}
