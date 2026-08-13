//! alx-cli — lógica reutilizable del binario `alx`.
//!
//! `AppState` guarda las tareas en memoria (Fase 2: skeleton funcional, sin
//! persistencia a disco) y `render_status` produce el texto de estado que el
//! binario pinta. `run_pipeline` integra el pipeline end-to-end: task → DAG →
//! descomposición → harness, ejecutando un gate REAL de comandos por
//! micro-tarea (`alx_gate::run_command`) y un critic loop (`IterationState`,
//! max 2 iteraciones) que re-ejecuta el pipeline desde cero cuando algún gate
//! falla, acumulando feedback. `render_run` pinta el informe con gates reales,
//! iteraciones usadas y el feedback acumulado.

use alx_core::types::{now_ms, Evidence, ModelTier, PhaseId, Recall, RecallSource, Task, TaskStatus};
use alx_critic::{criticize_real, derive_must_checks, iteration_prompt, CriticVerdict, IterationState};
use alx_evolve::{detect_candidates, HarnessRegistry};
use alx_governor::{Ledger, LedgerEntry};
use alx_harness::{Phases, Pipeline};
use alx_mcp::catalog::ToolCatalog;
use alx_mcp::server::handle_line;
use alx_memory::RecallStore;
use alx_night::{build_report, render as render_night};
use alx_task::decompose::decompose;
use alx_task::graph::TaskGraph;
use std::time::Instant;

/// Estado en memoria de la sesión del CLI.
#[derive(Debug, Default)]
pub struct AppState {
    tasks: Vec<Task>,
}

impl AppState {
    /// Estado vacío.
    pub fn new() -> Self {
        Self::default()
    }

    /// Añade una tarea y devuelve su id.
    pub fn add_task(&mut self, task: Task) -> &str {
        self.tasks.push(task);
        self.tasks
            .last()
            .map(|t| t.id.as_str())
            .unwrap_or("")
    }

    /// Número de tareas registradas.
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Vista de las tareas registradas (para listar).
    pub fn tasks(&self) -> &[Task] {
        &self.tasks
    }
}

/// Texto de estado: cabecera con total + recuento por fase.
///
/// Solo se listan las fases con tareas; la cabecera siempre muestra el total.
pub fn render_status(app: &AppState) -> String {
    let mut out = format!("ALEXANDRIA — {} tareas", app.task_count());
    for phase in PhaseId::ALL {
        let n = app.tasks().iter().filter(|t| t.phase == phase).count();
        if n > 0 {
            out.push_str(&format!("\n{}: {} tareas", phase.as_str(), n));
        }
    }
    out
}

/// Resultado de una ejecución del pipeline de demo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResult {
    /// Título de la tarea padre.
    pub task_title: String,
    /// Número de micro-tareas descompuestas.
    pub micro_tasks: usize,
    /// Micro-tareas que terminaron el pipeline en `Done`.
    pub done: usize,
    /// Micro-tareas que terminaron en `Failed`.
    pub failed: usize,
    /// Total de evidencias generadas por el harness.
    pub evidence_count: usize,
    /// Micro-tareas cuyo gate real falló (exit_code != 0).
    pub gate_failures: u32,
    /// Número de pasadas del pipeline que usó el critic loop.
    pub iterations_used: u32,
    /// Feedback acumulado del `IterationState` (críticas de cada iteración).
    pub feedback: Vec<String>,
}

/// Ejecuta el pipeline end-to-end sobre una tarea demo.
///
/// Por cada micro-tarea corre un gate real (`echo "gate ok para: <título>"`)
/// vía `alx_gate::run_command`; si el comando sale con 0 la micro-tarea
/// avanza por el `Pipeline`, si no queda `Failed` y cuenta como `gate_failure`.
/// Un critic loop (`IterationState`, max 2 iteraciones) re-ejecuta el pipeline
/// desde cero (Task nuevo por iteración) cuando hay gates fallidos, acumulando
/// el feedback.
pub fn run_pipeline(title: &str) -> RunResult {
    run_pipeline_with_gate(title, |t| format!("echo \"gate ok para: {t}\""))
}

/// `run_pipeline` con el comando de gate personalizable (tests y sondeo).
/// `gate_cmd` recibe el título de la micro-tarea y devuelve el comando real a
/// ejecutar. Envuelve `run_once` en el critic loop: si tras una pasada hay
/// `gate_failures > 0`, avanza el `IterationState` con el feedback y repite
/// hasta `max_iter` o hasta que no queden fallos.
fn run_pipeline_with_gate(title: &str, gate_cmd: impl Fn(&str) -> String) -> RunResult {
    const MAX_ITER: u32 = 2;
    let mut state = IterationState::new("t-demo", MAX_ITER);
    let mut runs = 0u32;

    loop {
        runs += 1;
        let mut run = run_once(title, &gate_cmd);
        run.iterations_used = runs;
        run.feedback = state.feedback.clone();
        if run.gate_failures == 0 {
            state.mark_passed();
            return run;
        }
        state.advance(format!(
            "{} gates fallaron en iteracion {runs}",
            run.gate_failures
        ));
        run.feedback = state.feedback.clone();
        if !state.should_iterate() {
            return run;
        }
    }
}

/// Una pasada completa del pipeline: crea un `Task` nuevo (el estado de cada
/// micro-tarea se reinicia), lo descompone y corre cada micro-tarea por el
/// `Pipeline` con el resultado de un gate real de comandos.
fn run_once(title: &str, gate_cmd: &dyn Fn(&str) -> String) -> RunResult {
    let now = now_ms();
    let mut graph = TaskGraph::new();
    let parent = Task::new("t-demo".to_string(), title.to_string(), PhaseId::Plan, 15_000, now);
    graph.add(parent.clone());

    let steps = vec![
        ("preparar contexto".to_string(), "archivos listados".to_string()),
        ("ejecutar paso".to_string(), "comando ok".to_string()),
    ];
    let children = decompose(&parent, steps);
    for child in &children {
        graph.add(child.clone());
    }

    let pipeline = Pipeline::new(Phases::default().0);

    let mut done = 0usize;
    let mut failed = 0usize;
    let mut evidence_count = 0usize;
    let mut gate_failures = 0u32;
    for child in &children {
        let task = graph.by_id_mut(&child.id).expect("micro-tarea en el grafo");
        // Gate real: la salida capturada (`stdout_head`) es la evidencia.
        let cmd = gate_cmd(&child.title);
        let outcome = alx_gate::run_command(&cmd, 5000);
        let gate_pass = outcome.exit_code == 0;
        if !gate_pass {
            gate_failures += 1;
        }
        let gate_ev = Evidence::command_output(&cmd, outcome.exit_code, &outcome.stdout_head, gate_pass);
        task.evidence.push(gate_ev);
        evidence_count += 1;
        loop {
            let result = pipeline.run_pipeline_step(task, gate_pass, now_ms());
            evidence_count += result.evidence.len();
            if result.completed {
                match task.status {
                    TaskStatus::Done => done += 1,
                    TaskStatus::Failed => failed += 1,
                    _ => {}
                }
                break;
            }
        }
    }

    RunResult {
        task_title: title.to_string(),
        micro_tasks: children.len(),
        done,
        failed,
        evidence_count,
        gate_failures,
        iterations_used: 0,
        feedback: Vec::new(),
    }
}

/// Informe legible del resultado de una ejecución del pipeline.
///
/// Incluye los gates reales ejecutados (`gates fallados`), las iteraciones del
/// critic loop y, cuando hubo feedback, el prompt de iteración generado con
/// `alx_critic::iteration_prompt`.
pub fn render_run(result: &RunResult) -> String {
    let mut out = format!(
        "## Pipeline run\nTítulo: {}\nMicro-tareas: {}, hechas: {}, fallidas: {}\nEvidencia: {}\ngates fallados: {}\nIteraciones usadas: {}",
        result.task_title,
        result.micro_tasks,
        result.done,
        result.failed,
        result.evidence_count,
        result.gate_failures,
        result.iterations_used
    );
    if !result.feedback.is_empty() {
        let state = IterationState {
            task_id: "t-demo".to_string(),
            iter: result.iterations_used.saturating_sub(1),
            max_iter: 2,
            feedback: result.feedback.clone(),
            passed: result.gate_failures == 0,
        };
        out.push_str(&format!("\n{}", iteration_prompt(&state)));
    }
    out
}

/// Estado de un endpoint de la red real del governor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkStatus {
    /// Nombre del servicio (headroom, routatic, ...).
    pub name: String,
    /// URL del endpoint.
    pub url: String,
    /// Responde a /readyz (curl exit_code 0).
    pub ready: bool,
    /// Código HTTP devuelto (o stdout capturado).
    pub http_code: String,
}

/// Comprueba la red real del governor (iter 41): headroom→mask→routatic
/// (PROVIDER) y fallback omniroute. Cada endpoint único se sondea con
/// `curl -s -m 2 <url>/readyz` vía `alx_gate::run_command`.
pub fn check_network() -> Vec<NetworkStatus> {
    // Infra real verificada (auditoría 14 §3). Orden = cadena canónica.
    let endpoints = [
        ("headroom (compresión)", "http://127.0.0.1:8788"),
        ("cc-model-mask (enmascara)", "http://127.0.0.1:3460"),
        ("routatic (PROVIDER)", "http://127.0.0.1:3456"),
        ("omniroute (fallback)", "http://127.0.0.1:20128"),
    ];

    endpoints
        .iter()
        .map(|(name, url)| {
            let cmd = format!("curl -s -m 2 -o /dev/null -w \"%{{http_code}}\" {url}/readyz");
            let outcome = alx_gate::run_command(&cmd, 5000);
            NetworkStatus {
                name: name.to_string(),
                url: url.to_string(),
                ready: outcome.exit_code == 0,
                http_code: outcome.stdout_head.trim().to_string(),
            }
        })
        .collect()
}

/// Dogfood: verifica el build del workspace actual con un gate real.
///
/// Corre `cargo build` en el cwd del proceso `alx` (desde el workspace de
/// ALEXANDRIA) vía `alx_gate::run_command`; la evidencia capturada es la
/// moneda de verificación del sistema.
pub fn verify_build() -> alx_core::types::Evidence {
    let cmd = "cargo build".to_string();
    let outcome = alx_gate::run_command(&cmd, 120_000);
    let passed = outcome.exit_code == 0;
    alx_core::types::Evidence::command_output(&cmd, outcome.exit_code, &outcome.stdout_head, passed)
}

/// Informe legible del resultado del dogfood build.
pub fn render_build(evidence: &alx_core::types::Evidence) -> String {
    if evidence.passed {
        format!(
            "✓ build OK (exit {})\n{}",
            evidence.exit_code,
            evidence.stdout_head.trim()
        )
    } else {
        format!(
            "✗ build FALLÓ (exit {})\n{}",
            evidence.exit_code,
            evidence.stdout_head.trim()
        )
    }
}

/// Resultado de una ejecución real del pipeline contra la cadena de red
/// (headroom→mask→routatic→deepseek) con ledger de coste.
#[derive(Debug, Clone)]
pub struct RealRunResult {
    /// Resultado del pipeline (micro-tareas, gates, evidencia).
    pub run: RunResult,
    /// Ledger de coste por micro-tarea.
    pub ledger: Ledger,
    /// Respuestas truncadas del modelo por micro-tarea.
    pub responses: Vec<String>,
    /// Veredictos del crítico real por micro-tarea.
    pub verdicts: Vec<CriticVerdict>,
    /// Must-checks aprendidos por el crítico (derive_must_checks).
    pub must_checks: Vec<String>,
    /// Harnesses detectados en el trabajo real (alx-evolve).
    pub harness_detected: Vec<String>,
}

/// Ejecuta el pipeline con llamadas REALES al modelo local vía headroom.
///
/// Por cada micro-tarea construye un envelope mínimo y hace
/// `POST /v1/messages` (curl vía `alx_gate::run_command`). Parsea el `usage`
/// de la respuesta (input/output tokens), registra la entrada en el `Ledger`
/// y anexa la respuesta como evidencia. Si la red no responde o no hay usage,
/// la micro-tarea cuenta como gate fallida (pero el ledger refleja el intento).
pub fn run_pipeline_real(title: &str) -> RealRunResult {
    const HEADROOM: &str = "http://127.0.0.1:8788";
    let now = now_ms();
    let mut graph = TaskGraph::new();
    let parent = Task::new("t-real".to_string(), title.to_string(), PhaseId::Plan, 15_000, now);
    graph.add(parent.clone());

    let steps = vec![
        ("preparar contexto".to_string(), "archivos listados".to_string()),
        ("ejecutar paso".to_string(), "comando ok".to_string()),
    ];
    let children = decompose(&parent, steps);
    for child in &children {
        graph.add(child.clone());
    }
    let pipeline = Pipeline::new(Phases::default().0);

    let mut run = RunResult {
        task_title: title.to_string(),
        micro_tasks: children.len(),
        done: 0,
        failed: 0,
        evidence_count: 0,
        gate_failures: 0,
        iterations_used: 1,
        feedback: Vec::new(),
    };
    let mut ledger = Ledger::new();
    let mut responses = Vec::new();
    let mut memories = RecallStore::new();
    let mut harnesses = HarnessRegistry::new();
    let mut verdicts = Vec::new();
    let mut must_checks: Vec<String> = Vec::new();
    let mut harness_detected: Vec<String> = Vec::new();

    for child in &children {
        let envelope = format!("Tarea: {}. Devuelve solo el resultado en una frase.", child.title);
        let cmd = format!(
            "curl -s -m 30 {HEADROOM}/v1/messages -H 'content-type: application/json' -d '{{\"model\":\"deepseek-v4-flash\",\"max_tokens\":60,\"messages\":[{{\"role\":\"user\",\"content\":\"{envelope}\"}}]}}'"
        );
        let start = Instant::now();
        let outcome = alx_gate::run_command(&cmd, 35_000);
        let latency_ms = start.elapsed().as_millis();

        let (in_tok, out_tok) = parse_usage(&outcome.stdout_head);
        ledger.record(LedgerEntry::new(
            "t-real",
            &child.title,
            ModelTier::T2Medium,
            "headroom→mask→routatic",
            in_tok,
            out_tok,
            latency_ms,
        ));

        // Critic real: la salida del modelo se evalúa contra criterios.
        let response_short: String = outcome.stdout_head.chars().take(300).collect();
        let verdict = criticize_real(
            &response_short,
            &["el resultado responde la tarea", "no inventa evidencia", "es conciso"],
        );

        // critic.learn: must-checks aprendidos → memoria (se inyectan en el futuro).
        for c in derive_must_checks(&verdict.findings) {
            if !must_checks.contains(&c) {
                must_checks.push(c.clone());
            }
            if memories.all().iter().all(|r| r.text != c) {
                let recall = Recall {
                    id: format!("r-mc-{}", memories.all().len() + 1),
                    text: c.clone(),
                    source: RecallSource::Tool,
                    tags: vec!["must_check".to_string()],
                    weight: 1,
                    created: now_ms(),
                };
                memories.add(recall);
            }
        }

        // evolve.detect: harnesses candidatos en el trabajo real.
        let work = format!("Tarea: {}. Respuesta: {response_short}", child.title);
        for cand in detect_candidates(&work) {
            if let Some(id) = harnesses.add_candidate(cand, now_ms()) {
                harness_detected.push(id);
            }
        }
        verdicts.push(verdict.clone());

        let gate_pass = outcome.exit_code == 0 && in_tok > 0 && verdict.approved;
        if !gate_pass {
            let blockers: Vec<String> = verdict
                .findings
                .iter()
                .filter(|f| matches!(f.severity, alx_critic::Severity::Block | alx_critic::Severity::Major))
                .map(|f| f.message.clone())
                .collect();
            if !blockers.is_empty() {
                run.feedback
                    .push(format!("critic bloquea: {}", blockers.join("; ")));
            }
            run.gate_failures += 1;
        }
        responses.push(outcome.stdout_head.chars().take(150).collect());

        let task = graph.by_id_mut(&child.id).expect("micro-tarea en el grafo");
        let head: String = outcome.stdout_head.chars().take(400).collect();
        task.evidence.push(Evidence::command_output(&cmd, outcome.exit_code, &head, gate_pass));
        run.evidence_count += 1;
        loop {
            let r = pipeline.run_pipeline_step(task, gate_pass, now_ms());
            run.evidence_count += r.evidence.len();
            if r.completed {
                match task.status {
                    TaskStatus::Done => run.done += 1,
                    TaskStatus::Failed => run.failed += 1,
                    _ => {}
                }
                break;
            }
        }
    }

    RealRunResult { run, ledger, responses, verdicts, must_checks, harness_detected }
}

/// Extrae (input_tokens, output_tokens) del `usage` de una respuesta
/// Anthropic-compatible. Devuelve (0,0) si no parsea.
fn parse_usage(json: &str) -> (u32, u32) {
    use serde_json::Value;
    let v: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    let usage = &v["usage"];
    let i = usage["input_tokens"].as_u64().unwrap_or(0) as u32;
    let o = usage["output_tokens"].as_u64().unwrap_or(0) as u32;
    (i, o)
}

/// Informe legible de la ejecución real: pipeline + ledger de coste.
pub fn render_real_run(result: &RealRunResult) -> String {
    let mut out = render_run(&result.run);
    let (in_tok, out_tok) = result.ledger.total_tokens();
    out.push_str(&format!(
        "\n\n## Ledger de coste\nLlamadas reales: {}\nTokens: {} in / {} out\nCoste estimado: ${:.6}\n",
        result.ledger.entry_count(),
        in_tok,
        out_tok,
        result.ledger.total_cost_usd()
    ));
    for (i, e) in result.ledger.entries().iter().enumerate() {
        out.push_str(&format!(
            "\n  {} {} — {} in/{} out — ${:.6} — {}ms",
            i + 1,
            e.micro_task,
            e.input_tokens,
            e.output_tokens,
            e.cost_usd,
            e.latency_ms
        ));
    }

    if !result.verdicts.is_empty() {
        out.push_str("\n\n## Crítica real por micro-tarea\n");
        for (i, v) in result.verdicts.iter().enumerate() {
            let mark = if v.approved { "✓" } else { "✗" };
            out.push_str(&format!(
                "\n  {mark} micro-{}: {}",
                i + 1,
                if v.approved { "aprobado" } else { "rechazado" }
            ));
            for f in &v.findings {
                out.push_str(&format!("\n    {:?}: {}", f.severity, f.message));
            }
        }
    }
    if !result.must_checks.is_empty() {
        out.push_str("\n\n## Must-checks aprendidos (memoria)\n");
        for c in &result.must_checks {
            out.push_str(&format!("\n  - {c}"));
        }
    }
    if !result.harness_detected.is_empty() {
        out.push_str("\n\n## Harnesses detectados (evolve)\n");
        for h in &result.harness_detected {
            out.push_str(&format!("\n  - {h}"));
        }
    }
    out
}

/// Informe nocturno real desde el DAG (alx-night).
pub fn render_night_report() -> String {
    let now = now_ms();
    let mut graph = TaskGraph::new();
    graph.add(Task::new("t-n-1".into(), "preparar contexto".into(), PhaseId::Plan, 15_000, now));
    graph.add(Task::new("t-n-2".into(), "ejecutar paso".into(), PhaseId::Build, 15_000, now));
    let _ = graph.transition("t-n-1", TaskStatus::Done, now);
    let report = build_report(&graph, "2026-08-13");
    render_night(&report)
}

/// Estado del plugin PHALANX: config.toml (secciones) + hooks .toml.
/// Resuelve la ruta desde el manifest del crate (funciona desde cualquier cwd).
pub fn render_phalanx_status() -> String {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../phalanx");
    let config_path = base.join("config.toml");
    let hooks_dir = base.join("hooks");
    let config_ok = config_path.exists();
    let hooks = std::fs::read_dir(&hooks_dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().extension().map(|x| x == "toml").unwrap_or(false))
                .count()
        })
        .unwrap_or(0);
    let mut sections = Vec::new();
    if config_ok {
        if let Ok(text) = std::fs::read_to_string(&config_path) {
            for line in text.lines() {
                let l = line.trim();
                if l.starts_with('[') && l.ends_with(']') {
                    sections.push(l.to_string());
                }
            }
        }
    }
    format!(
        "## PHALANX\nconfig.toml: {}\nSecciones: {}\nHooks: {} .toml\n",
        if config_ok { "✓" } else { "✗ falta" },
        sections.join(" "),
        hooks
    )
}

/// Dogfood: ejecuta el pipeline y escribe el informe como artefacto real del
/// repo (`out_dir/<slug>.md`). `real` usa la cadena LLM con critic + ledger.
pub fn feature_run(title: &str, real: bool, out_dir: &str) -> String {
    let report = if real {
        render_real_run(&run_pipeline_real(title))
    } else {
        render_run(&run_pipeline(title))
    };
    let slug = title.to_lowercase().replace(' ', "-");
    let dir = std::path::Path::new(out_dir);
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join(format!("{slug}.md"));
    match std::fs::write(&path, &report) {
        Ok(()) => format!("✓ artefacto escrito: {}\n\n{report}", path.display()),
        Err(e) => format!("✗ no se pudo escribir: {e}\n\n{report}"),
    }
}

/// Sirve el protocolo MCP JSON-RPC por stdio (demo real): lee líneas de
/// stdin, responde `initialize` / `tools/list` / `tools/call`.
pub fn serve_mcp_stdio() -> i32 {
    use std::io::BufRead;
    let catalog = ToolCatalog::alexandria_default();
    for line in std::io::stdin().lock().lines().flatten() {
        if let Some(resp) = handle_line(&catalog, &line) {
            println!("{resp}");
        }
    }
    0
}

/// Informe legible del estado de red.
pub fn render_network(statuses: &[NetworkStatus]) -> String {
    let mut out = String::from(
        "## Red real (governor)\nCadena: headroom:8788 → mask:3460 → routatic:3456 (PROVIDER) → deepseek-v4-flash\nFallback: omniroute:20128 (solo si routatic cae)\n",
    );
    for s in statuses {
        let mark = if s.ready { "✓" } else { "✗" };
        out.push_str(&format!(
            "\n{mark} {} — {} {} (http {})",
            s.name,
            s.url,
            if s.ready { "listo" } else { "NO responde" },
            s.http_code
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use alx_core::types::{now_ms, Task};
    use alx_lib::Alexandria;

    fn task(id: &str, phase: PhaseId) -> Task {
        Task::new(id.to_string(), "tarea".to_string(), phase, 15_000, now_ms())
    }

    #[test]
    fn render_status_empty() {
        let app = AppState::new();
        assert_eq!(app.task_count(), 0);
        let text = render_status(&app);
        assert!(text.contains("ALEXANDRIA — 0 tareas"));
        // Sin tareas no hay líneas por fase.
        assert!(!text.contains(": 1 tareas"));
    }

    #[test]
    fn render_status_one_task_by_phase() {
        let mut app = AppState::new();
        app.add_task(task("t-1", PhaseId::Build));
        assert_eq!(app.task_count(), 1);
        let text = render_status(&app);
        assert!(text.contains("ALEXANDRIA — 1 tareas"));
        assert!(text.contains("Build: 1 tareas"));
    }

    #[test]
    fn add_task_increments_count_and_keeps_phase() {
        let mut app = AppState::new();
        let id = app.add_task(task("t-1", PhaseId::Spec));
        assert_eq!(id, "t-1");
        assert_eq!(app.task_count(), 1);
        assert_eq!(app.tasks()[0].phase, PhaseId::Spec);
    }

    #[test]
    fn run_pipeline_completes_two_micro_tasks() {
        let r = run_pipeline("feature demo");
        assert_eq!(r.task_title, "feature demo");
        assert_eq!(r.micro_tasks, 2);
        assert_eq!(r.done, 2);
        assert_eq!(r.failed, 0);
        assert_eq!(r.gate_failures, 0); // el echo real sale 0 en todas las micro-tareas
        assert_eq!(r.iterations_used, 1); // sin fallos → una sola pasada
        assert!(r.evidence_count > 0);
        assert!(r.feedback.is_empty());
    }

    #[test]
    fn render_run_contains_gate_and_title() {
        let r = run_pipeline("feature demo");
        let text = render_run(&r);
        assert!(text.contains("feature demo"));
        assert!(text.contains("## Pipeline run"));
        assert!(text.contains("hechas: 2"));
        assert!(text.contains("gates fallados: 0"));
        assert!(text.contains("Iteraciones usadas: 1"));
    }

    #[test]
    fn failing_gate_triggers_critic_loop_to_max_iter() {
        // Comando que siempre falla: gate real exit_code 1 → critica → 2ª pasada.
        let r = run_pipeline_with_gate("demo", |_| "exit 1".to_string());
        assert_eq!(r.gate_failures, 2); // las 2 micro-tareas
        assert_eq!(r.done, 0);
        assert_eq!(r.failed, 2);
        assert_eq!(r.iterations_used, 2); // max_iter=2 agotado
        assert_eq!(r.feedback.len(), 2);
        assert!(r.feedback[0].contains("2 gates fallaron en iteracion 1"));
        assert!(r.feedback[1].contains("2 gates fallaron en iteracion 2"));
    }

    #[test]
    fn render_run_uses_iteration_prompt_when_feedback() {
        let r = run_pipeline_with_gate("demo", |_| "exit 1".to_string());
        let text = render_run(&r);
        // iteration_prompt se usa: cabecera de iteración + feedback acumulado.
        assert!(text.contains("Feedback de iteraciones previas"));
        assert!(text.contains("gates fallaron en iteracion"));
        assert!(text.contains("Iteración 2 de 2"));
    }

    #[test]
    fn status_via_facade_mentions_alexandria() {
        let alex = Alexandria::new();
        assert!(alex.status().contains("ALEXANDRIA"));
    }

    #[test]
    fn check_network_returns_four_endpoints() {
        let statuses = check_network();
        assert_eq!(statuses.len(), 4);
        assert!(statuses.iter().any(|s| s.name.contains("routatic")));
        assert!(statuses.iter().any(|s| s.name.contains("omniroute")));
    }

    #[test]
    fn render_network_mentions_chain_and_provider() {
        let statuses = check_network();
        let text = render_network(&statuses);
        assert!(text.contains("PROVIDER"));
        assert!(text.contains("headroom"));
        assert!(text.contains("routatic"));
    }

    #[test]
    fn parse_usage_extracts_tokens() {
        let json = r#"{"id":"x","usage":{"input_tokens":85,"output_tokens":20,"cache_creation_input_tokens":85}}"#;
        let (i, o) = parse_usage(json);
        assert_eq!((i, o), (85, 20));
    }

    #[test]
    fn parse_usage_empty_on_garbage() {
        let (i, o) = parse_usage("no-json");
        assert_eq!((i, o), (0, 0));
    }
}
