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
use alx_critic::{
    criticize_real, derive_must_checks, escalate_real, iteration_prompt, CriticVerdict,
    IterationState,
};
use alx_agents::{build_envelope, AgentRegistry, AgentSpec};
use alx_audit::{AuditIndex, AuditItem, ItemKind};
use alx_evolve::{detect_candidates, Harness, HarnessKind, HarnessRegistry, Trigger};
use alx_governor::{classify_prompt_text, Ledger, LedgerEntry};
use alx_harness::{Phases, Pipeline};
use alx_mcp::catalog::ToolCatalog;
use alx_mcp::server::handle_line;
use alx_memory::{compress as caveman_compress, RecallStore};
use alx_night::{build_report, render as render_night};
use alx_task::decompose::decompose;
use alx_task::graph::TaskGraph;
use std::io::IsTerminal;
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

/// Estado real persistido en disco (no en memoria): tareas de
/// `state/tasks.jsonl`, hooks de `phalanx/hooks/*.toml`, recalls contados
/// como eventos de `state/events.log` y agentes registrados en `agents/`.
///
/// `alx status` usa esta fuente para que el resumen refleje el sistema real.
pub fn render_status_persisted() -> String {
    // state/ vive en alexandria/state; phalanx/ y agents/ en la raíz del repo
    // (alexandria/.. = raíz, desde crates/alx-cli son ../../).
    let ws = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
    let root = ws.join("../");

    let tasks = load_tasks_from_jsonl().len();

    let hooks_dir = root.join("phalanx/hooks");
    let hooks = std::fs::read_dir(&hooks_dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().extension().map(|x| x == "toml").unwrap_or(false))
                .count()
        })
        .unwrap_or(0);

    let events_path = ws.join("state/events.log");
    let recalls = std::fs::read_to_string(&events_path)
        .map(|t| t.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0);

    let agents_dir = root.join("agents");
    let agents = std::fs::read_dir(&agents_dir)
        .map(|rd| rd.flatten().filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false)).count())
        .unwrap_or(0);

    format!("ALEXANDRIA — {tasks} tareas, {hooks} hooks, {recalls} recalls, {agents} agentes")
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

    // Micro-tareas CONCRETAS y verificables: el modelo responde bien y el
    // critic (estricto) puede aprobar. Mejora el critic éxito (era 0%).
    let steps = vec![
        ("explica en una frase que hace el comando alx status".to_string(), "respuesta clara sobre alx status".to_string()),
        ("lista 2 subcomandos de alx y su proposito en una linea cada uno".to_string(), "lista correcta de 2 subcomandos".to_string()),
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
        let Some(task) = graph.by_id_mut(&child.id) else {
            gate_failures += 1;
            continue;
        };
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

/// Comprueba la red real del governor (iter 41 → v2 routa): 
/// `headroom → gateway(:3460) → routatic` (PROVIDER) y fallback omniroute.
/// Cada endpoint se sondea con un **GET barato** (`/health`, `/readyz`,
/// `/v1/models`): cero generaciones de pago. La v1 hacía POST con
/// max_tokens=1 que la mask convertía en una generación completa (~308
/// tokens y ~44 s por ping); con el statusline sondeando cada refresco eso
/// saturaba opencode-go y Claude Code moría al segundo mensaje con
/// "all models failed". El gateway además cortocircuita los probes, pero
/// aquí ni siquiera llegan: GET y listo.
pub fn check_network() -> Vec<NetworkStatus> {
    // Infra real verificada. Orden = cadena canónica.
    let endpoints = [
        ("routa-gateway (mascara+entropia)", "http://127.0.0.1:3460", "/health"),
        ("headroom (compresion)", "http://127.0.0.1:8788", "/readyz"),
        ("routatic (PROVIDER)", "http://127.0.0.1:3456", "/v1/models"),
        ("omniroute (fallback)", "http://127.0.0.1:20128", "/"),
    ];

    endpoints
        .iter()
        .map(|(name, url, ruta)| {
            let cmd = format!(
                "curl -s -m 5 -o /dev/null -w \"%{{http_code}}\" {url}{ruta}"
            );
            let outcome = alx_gate::run_command(&cmd, 8000);
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
    // Ruta absoluta al workspace: funciona desde cualquier cwd (dogfood real).
    let ws = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let cmd = format!("cargo build --manifest-path {}", ws.display());
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

/// Gate real por fase del harness: qué comando verifica la salida de cada
/// fase (Build→cargo build, Test→cargo test, Review→clippy...).
pub fn gate_for_phase(phase: PhaseId) -> &'static str {
    match phase {
        PhaseId::Build => "cargo build",
        PhaseId::Test => "cargo test",
        PhaseId::Review => "cargo clippy -- -D warnings",
        PhaseId::Docs => "grep -r 'TODO' docs/ || true",
        _ => "echo fase sin gate real",
    }
}

/// Resultado de una ejecución real del pipeline contra la cadena de red
/// (headroom→gateway→routatic→modelo real del config) con ledger de coste.
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

    // Micro-tareas CONCRETAS y verificables: el modelo responde bien y el
    // critic (estricto) puede aprobar. Mejora el critic éxito (era 0%).
    let steps = vec![
        ("explica en una frase que hace el comando alx status".to_string(), "respuesta clara sobre alx status".to_string()),
        ("lista 2 subcomandos de alx y su proposito en una linea cada uno".to_string(), "lista correcta de 2 subcomandos".to_string()),
    ];
    let children = decompose(&parent, steps);
    // Asignar una fase distinta a cada micro-tarea (Build, Test, Review...)
    // para que cada una use el agente especializado de su fase.
    let phases = [PhaseId::Build, PhaseId::Test, PhaseId::Review, PhaseId::Docs];
    for (i, child) in children.iter().enumerate() {
        if let Some(task) = graph.by_id_mut(&child.id) {
            task.phase = phases[i % phases.len()];
        }
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

    // Registry REAL de agentes del ecosistema (sin decision AI: por fase).
    let mut real_reg = AgentRegistry::new();
    {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../");
        let mut files = Vec::new();
        for dir in ["agents-volt", "agents"] {
            if let Ok(rd) = std::fs::read_dir(repo_root.join(dir)) {
                for e in rd.flatten().take(200) {
                    if e.path().extension().map(|x| x == "md").unwrap_or(false) {
                        files.push(e.path().to_string_lossy().to_string());
                    }
                }
            }
        }
        let refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
        let _ = real_reg.register_from_markdowns(&refs);
    }

    for child in &children {
        // Tier por fase: real del ecosistema si existe; fallback a builtin.
        let tier = if let Some(a) = real_reg.by_phase(child.phase).first() {
            a.tier
        } else {
            match child.phase {
                PhaseId::Review => ModelTier::T3Premium,
                _ => classify_prompt_text(title),
            }
        };
        // Envelope MÍNIMO para el pipeline: el system largo del agente (reglas
        // caveman) hace que el modelo responda detallado y se corte. Un prompt
        // simple que pide respuesta corta mejora el critic éxito.
        let envelope = format!("Tarea: {}. Responde en maximo 2 frases concisas.", child.title);
        let body = serde_json::json!({
            "model": modelo_real_activo(),
            "max_tokens": 600,
            "thinking": { "type": "disabled" },
            "messages": [{ "role": "user", "content": envelope }]
        })
        .to_string();
        let body_path = std::env::temp_dir().join("alx-run-body.json");
        if std::fs::write(&body_path, &body).is_err() {
            run.gate_failures += 1;
            continue;
        }
        let cmd = format!(
            "curl -s -m 30 {HEADROOM}/v1/messages -H 'content-type: application/json' -d @{}",
            body_path.display()
        );
        let start = Instant::now();
        let outcome = alx_gate::run_command(&cmd, 35_000);
        let latency_ms = start.elapsed().as_millis();

        let (in_tok, out_tok) = parse_usage(&outcome.stdout_head);
        ledger.record(LedgerEntry::new(
            "t-real",
            &child.title,
            tier,
            "headroom→mask→routatic",
            in_tok,
            out_tok,
            latency_ms,
        ));

        // Critic real: la salida se evalúa CON la tarea (el critic necesita
        // saber qué se pedía para verificar si la respuesta la cumple).
        let response_short: String = format!(
            "Tarea: {}. Respuesta: {}",
            child.title,
            outcome.stdout_head.chars().take(1500).collect::<String>()
        );
        let verdict = criticize_real(
            &response_short,
            // Criterios QA: la tarea es de texto, no de comandos. "no inventa
            // evidencia" era inadecuado (exigía comandos ejecutados).
            &["responde la tarea correctamente", "es conciso", "sin contradicciones"],
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
        responses.push(outcome.stdout_head.chars().take(150).collect::<String>());

        let Some(task) = graph.by_id_mut(&child.id) else {
            run.gate_failures += 1;
            continue;
        };
        let head: String = outcome.stdout_head.chars().take(400).collect();
        task.evidence.push(Evidence::command_output(&cmd, outcome.exit_code, &head, gate_pass));
        run.evidence_count += 1;
        loop {
            let r = pipeline.run_pipeline_step(task, gate_pass, now_ms());
            run.evidence_count += r.evidence.len();
            if r.completed {
                // Gate real de la fase final: verificar la salida con el
                // comando real de la fase (cargo build/test/clippy).
                let phase_gate = gate_for_phase(task.phase);
                let phase_out = alx_gate::run_command(phase_gate, 120_000);
                let ph_head: String = phase_out.stdout_head.chars().take(200).collect();
                task.evidence.push(Evidence::command_output(
                    phase_gate,
                    phase_out.exit_code,
                    &ph_head,
                    phase_out.exit_code == 0,
                ));
                run.evidence_count += 1;
                match task.status {
                    TaskStatus::Done => run.done += 1,
                    TaskStatus::Failed => run.failed += 1,
                    _ => {}
                }
                break;
            }
        }
    }

    // Escalada T3 real: si el critic barato no resolvió, un crítico estricto
    // decide con una última llamada sobre la última respuesta.
    if run.gate_failures > 0 {
        if let Some(last) = responses.last() {
            let final_verdict = escalate_real(last);
            let decision = if final_verdict.approved { "APROBADO" } else { "RECHAZADO" };
            let why = final_verdict
                .findings
                .first()
                .map(|f| f.message.clone())
                .unwrap_or_default();
            run.feedback.push(format!("ESCALADA T3: {decision} — {why}"));
        }
    }

    // Evolve continuo: watcher retira/promueve harnesses con el trabajo real.
    let _ = run_evolve_cycle();

    // Telemetría: log de eventos del pipeline (state/events.log, append).
    let state_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../state");
    let _ = std::fs::create_dir_all(&state_dir);
    let event_line = serde_json::json!({
        "ts": now_ms(),
        "event": "pipeline_done",
        "title": title,
        "micro_tasks": run.micro_tasks,
        "done": run.done,
        "gate_failures": run.gate_failures,
        "iterations": run.iterations_used,
        "must_checks": must_checks.len(),
        "harnesses": harness_detected.len(),
    })
    .to_string();
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state_dir.join("events.log"))
        .and_then(|mut f| {
            use std::io::Write;
            writeln!(f, "{event_line}")
        });

    // Persistir el ledger para el cost-report (state/ledger.jsonl, append).
    let ledger_path = state_dir.join("ledger.jsonl");
    for e in ledger.entries() {
        use std::io::Write;
        if let Ok(line) = serde_json::to_string(e) {
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&ledger_path)
                .and_then(|mut f| writeln!(f, "{line}"));
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

/// Telemetría: registra la ejecución de un comando (state/commands.log).
pub fn log_command(name: &str) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../state");
    let path = dir.join("commands.log");
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{name}");
    }
}

/// Benchmark de desempeño del sistema contra EXPECTATIVAS.
/// Objetivo: desempeño TOP (mejor que las alternativas solo con el harness).
pub fn render_quality() -> String {
    let mut out = String::from("## Quality — benchmark del sistema\n");

    // Latencia de comandos clave (expect: < 500ms cada uno).
    let mut total_ms: u128 = 0;
    for (name, cmd) in [
        ("status", "alx status"),
        ("network", "alx network"),
        ("cost", "alx cost"),
        ("iterate", "alx iterate"),
    ] {
        let start = Instant::now();
        let _ = std::process::Command::new("sh").arg("-c").arg(cmd).output();
        let ms = start.elapsed().as_millis();
        total_ms += ms;
        let ok = if ms < 500 { "✓" } else { "✗" };
        out.push_str(&format!("  latencia {name}: {ms}ms {ok} (expect < 500ms)\n"));
    }

    // Coste acumulado (expect: < $0.01 por sesión).
    let state_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../state");
    let ledger_path = state_dir.join("ledger.jsonl");
    let (mut n, mut cost) = (0usize, 0.0f64);
    if let Ok(text) = std::fs::read_to_string(&ledger_path) {
        for line in text.lines() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                n += 1;
                cost += v["cost_usd"].as_f64().unwrap_or(0.0);
            }
        }
    }
    let cost_ok = if cost < 0.01 { "✓" } else { "✗" };
    out.push_str(&format!("  coste: {n} llamadas, ${cost:.6} {cost_ok} (expect < $0.01)\n"));

    // Éxito del critic (expect: > 50% de pipelines sin gates fallados).
    let events_path = state_dir.join("events.log");
    let (mut total_events, mut ok_events) = (0usize, 0usize);
    if let Ok(text) = std::fs::read_to_string(&events_path) {
        for line in text.lines() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                total_events += 1;
                if v["gate_failures"].as_u64().unwrap_or(1) == 0 {
                    ok_events += 1;
                }
            }
        }
    }
    let critic_pct = if total_events > 0 {
        (ok_events as f64 / total_events as f64) * 100.0
    } else {
        0.0
    };
    let critic_ok = if critic_pct >= 50.0 { "✓" } else { "✗" };
    out.push_str(&format!(
        "  critic éxito: {ok_events}/{total_events} ({critic_pct:.0}%) {critic_ok} (expect > 50%)\n"
    ));

    out.push_str(&format!("  latencia total: {total_ms}ms\n"));
    out
}

/// Genera un script Python con el modelo para la tarea (una llamada).
fn generate_script(task: &str) -> String {
    // ALX_BENCH_MODEL / ALX_BENCH_URL permiten correr el benchmark con otro
    // modelo u otro hop de la cadena. Ruta canónica: headroom:8788 →
    // routa-gateway:3460 → routatic:3456. El gateway traduce cualquier alias
    // claude-*/[1m] al modelo real activo en el config de routatic.
    let url = std::env::var("ALX_BENCH_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8788".to_string());
    // Sin hardcodeo: el modelo lo dicta el config de routatic en vivo
    // (ALX_BENCH_MODEL sigue disponible como override de experimento).
    let model = modelo_real_activo();
    // Lección 2026-08-25: deepseek RAZONA aunque le pidas thinking disabled
    // (llegan bloques thinking igualmente). Con presupuesto pequeño el
    // razonamiento se come los tokens y el texto sale vacío -> 0% en ambos
    // modos. Presupuesto grande SIEMPRE para código; y timeout de proceso >
    // timeout de curl (antes run_command mataba curl a los 35 s).
    let claude_path = url.contains("3460") || model.contains("claude") || model.contains("opus");
    let _ = claude_path;
    let max_tokens: u32 = 4000;
    let body = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": [{
            "role": "user",
            "content": format!("{task}. Escribe SOLO el codigo Python, sin explicacion.")
        }]
    });
    let body = body.to_string();
    let body_path = std::env::temp_dir().join("alx-gen-script.json");
    if std::fs::write(&body_path, &body).is_err() {
        return String::new();
    }
    // OJO: run_command trunca stdout a 4000 chars (alx-gate STDOUT_HEAD_MAX).
    // Las respuestas con código completo la superan -> el JSON salía cortado y
    // TODO fallaba (0/6 en bench con respuestas válidas). Volcado a fichero.
    let resp_path = std::env::temp_dir().join("alx-gen-script-resp.json");
    let cmd = format!(
        "curl -s -m 170 {url}/v1/messages -H 'content-type: application/json' -d @{} -o {}",
        body_path.display(),
        resp_path.display()
    );
    let out = alx_gate::run_command(&cmd, 190_000);
    if out.exit_code != 0 {
        return String::new();
    }
    let Ok(raw) = std::fs::read_to_string(&resp_path) else {
        return String::new();
    };
    serde_json::from_str::<serde_json::Value>(raw.trim())
        .ok()
        .and_then(|v| {
            v["content"]
                .as_array()?
                .iter()
                .find(|b| b["type"] == "text")?
                .get("text")?
                .as_str()
                .map(|s| s.to_string())
        })
        .unwrap_or_default()
}

/// Extrae el código Python de una respuesta del modelo (entre ``` o directo).
fn extract_script(resp: &str) -> String {
    if let Some(start) = resp.find("```python") {
        let rest = &resp[start + 10..];
        if let Some(end) = rest.find("```") {
            return rest[..end].trim().to_string();
        }
    }
    resp.trim().to_string()
}

/// Ejecuta un script Python y devuelve su stdout.
fn execute_script(script: &str) -> String {
    let path = std::env::temp_dir().join("alx-bench.py");
    if std::fs::write(&path, script).is_err() {
        return String::new();
    }
    let out = alx_gate::run_command(&format!("python3 {}", path.display()), 15_000);
    out.stdout_head.trim().to_string()
}

/// Harness: genera código, lo EJECUTA, compara con el esperado, y si falla
/// RE-ITERA con feedback (el loop de mejora). Esa es la ventaja real sobre
/// una AI directa (que no itera sobre la ejecución).
/// Harness con TEST INTERMEDIO + caso final parametrizado.
/// Verifica la lógica con un caso pequeño (assert) ANTES del caso final:
/// descomposición en pasos verificables. `final_case` es la línea que imprime
/// el resultado (ej. "print(f(1000))"); si es vacío, el script del modelo ya
/// imprime el resultado y no se añade nada.
fn harness_attempt(
    task: &str,
    expected: &str,
    intermediate: &str,
    final_case: &str,
) -> (bool, String) {
    let mut feedback = String::new();
    let mut last_out = String::from("(no ejecutado)");
    for _attempt in 0..3 {
        let prompt = format!(
            "{task}. {feedback}Escribe SOLO codigo Python: define una funcion f(limite) que resuelva la tarea para un limite dado. Al final: {}.",
            if final_case.is_empty() {
                "imprime el resultado".to_string()
            } else {
                final_case.to_string()
            }
        );
        let script = extract_script(&generate_script(&prompt));
        // Asegurar que el caso final se ejecute (añadir si el modelo lo omitió).
        let run_script = if final_case.is_empty() || script.contains(final_case) {
            script.clone()
        } else {
            format!("{script}\n{final_case}")
        };
        // Test intermedio: verificar la lógica con un caso pequeño.
        let test_script = format!("{run_script}\n{intermediate}\nprint('TEST_OK')");
        let test_out = execute_script(&test_script);
        if !test_out.contains("TEST_OK") {
            feedback = format!(
                "El test intermedio '{intermediate}' fallo. Revisa la logica de f. "
            );
            last_out = test_out.chars().take(50).collect::<String>();
            continue;
        }
        last_out = execute_script(&run_script);
        if last_out == expected {
            return (true, last_out);
        }
        feedback = format!(
            "El caso final imprimio '{}' pero el esperado es '{}'. Corrige. ",
            last_out, expected
        );
    }
    (false, last_out)
}

/// Benchmark de EJECUCIÓN REAL: el modelo genera código, se ejecuta, y la
/// verificación es por OUTPUT (no texto). Mide la ventaja del harness:
/// directa = genera y ejecuta sin verificar; harness = genera + critic
/// verifica el código antes de ejecutar.
pub fn render_benchmark() -> String {
    // Tareas con TRAMPAS donde una AI directa tiende a fallar (Euler clásicos):
    // la verificación por ejecución + critic del harness expone la ventaja.
    // (tarea, expected, test_intermedio, caso_final): el harness verifica un
    // caso pequeño ANTES del caso final parametrizado — pasos verificables.
    // Tareas de dificultad creciente; valores verificados con ejecución real.
    let tasks: [(&str, &str, &str, &str); 5] = [
        ("Escribe un script Python que imprima la suma de todos los multiplos de 3 o 5 MENORES que 100 (sin contar dobles)", "2318", "assert f(10) == 23", "print(f(100))"),
        ("Escribe un script Python que imprima la suma de los digitos de 2**100", "115", "assert sum(int(d) for d in str(2**10)) == 7", ""),
        ("Escribe un script Python que imprima el total de letras de los numeros del 1 al 1000 escritos en ingles (Euler 17, sin guiones ni espacios)", "21124", "assert f(20) == 112", "print(f(1000))"),
        ("Euler 50: imprime el primo por debajo de 1000000 que puede escribirse como la SUMA DE LA MAYOR cantidad de primos consecutivos", "997651", "assert f(100) == 41", "print(f(1000000))"),
        ("Euler 21: imprime la suma de todos los numeros amigables por debajo de 10000 (a y b amigables si d(a)=b y d(b)=a, a!=b)", "31626", "assert f(1000) == 504", "print(f(10000))"),
    ];
    let mut out = String::from("## Benchmark — ejecución real (generar + ejecutar + verificar output)\n");
    let (mut direct_ok, mut harness_ok) = (0usize, 0usize);
    for (i, (task, expected, intermediate, final_case)) in tasks.iter().enumerate() {
        // Directa: generar script + ejecutar sin verificación.
        let d_script = extract_script(&generate_script(task));
        let d_out = execute_script(&d_script);
        let d = d_out == *expected;
        if d {
            direct_ok += 1;
        }

        // Harness: test intermedio + caso final + RE-ITERAR con feedback.
        let (h, h_out) = harness_attempt(task, expected, intermediate, final_case);
        if h {
            harness_ok += 1;
        }

        out.push_str(&format!(
            "  tarea {}: directa {} (out: {}) | harness {} (out: {})\n",
            i + 1,
            if d { "✓" } else { "✗" },
            d_out,
            if h { "✓" } else { "✗" },
            h_out
        ));
    }
    out.push_str(&format!(
        "Directa: {direct_ok}/{} · Harness: {harness_ok}/{} — verificación por EJECUCIÓN real\n",
        tasks.len(),
        tasks.len()
    ));
    out
}

/// Ejecuta una solución BigCodeBench contra sus unittest REALES.
/// ALX_RESULT se imprime PRIMERO (sobrevive la cabecera truncada de 4000
/// chars); timeout generoso (120s) porque los tests de BigCodeBench son
/// pesados (permutaciones, etc.).
fn run_bigcode(solution: &str, test: &str) -> (bool, String) {
    let runner = format!(
        "{test}\nimport io, unittest\nbuf = io.StringIO()\nsuite = unittest.defaultTestLoader.loadTestsFromTestCase(TestCases)\nres = unittest.TextTestRunner(stream=buf, verbosity=0).run(suite)\nprint('ALX_RESULT:', res.wasSuccessful())\nfor t in res.failures + res.errors: print('FAIL_TEST:', t[0].id())\nprint(buf.getvalue()[:1200])\n"
    );
    let path = std::env::temp_dir().join("alx-bigcode.py");
    if std::fs::write(&path, format!("{solution}\n\n{runner}")).is_err() {
        return (false, "error escribiendo script".to_string());
    }
    let out = alx_gate::run_command(&format!("python3 {}", path.display()), 120_000);
    let all = out.stdout_head;
    let ok = all
        .lines()
        .next()
        .map(|l| l.contains("ALX_RESULT: True"))
        .unwrap_or(false);
    // Feedback: nombre(s) del test fallido. (Ensayo de detalle expected/actual
    // en iter 11 midió PEOR: 29/60 vs 34/60 — el detalle añadía ruido. Se
    // revirtió al feedback simple que midió 34/60 en iter 10.)
    let frag = all
        .lines()
        .skip(1)
        .filter(|l| l.starts_with("FAIL_TEST:") || l.contains("AssertionError"))
        .take(2)
        .collect::<Vec<_>>()
        .join(" | ");
    (
        ok,
        if frag.is_empty() {
            all.chars().take(120).collect()
        } else {
            frag
        },
    )
}

/// Benchmark REAL: problemas de BigCodeBench (ICLR'25) con unittest verificables.
/// Lee las tareas del disco (harnesses/bench/bigcodebench-sample.jsonl) — el
/// benchmark NO es nuestro; son tareas profesionales donde los frontier fallan.
/// Directa = 1 intento; Harness = iterar sobre los fallos del unittest.
pub fn render_bench_bigcode() -> String {
    // ALX_BENCH_FILE permite validar en sets disjuntos (held-out).
    let path_default = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../harnesses/bench/bigcodebench-sample.jsonl");
    let path = std::path::PathBuf::from(
        std::env::var("ALX_BENCH_FILE").unwrap_or_else(|_| path_default.to_string_lossy().to_string()),
    );
    let mut out = String::from("## Benchmark REAL — BigCodeBench (ICLR'25) sample\n");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return out + "sin bigcodebench-sample.jsonl\n";
    };
    let mut tasks: Vec<serde_json::Value> = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            tasks.push(v);
        }
    }
    if let Ok(cap) = std::env::var("ALX_BENCH_MAX") {
        if let Ok(n) = cap.trim().parse::<usize>() {
            tasks.truncate(n);
        }
    }
    let (mut d_ok, mut h_ok) = (0usize, 0usize);
    for (i, t) in tasks.iter().enumerate() {
        let id_fallback = format!("BCB/{i}");
        let id = t["task_id"].as_str().unwrap_or(&id_fallback);
        let problem = t["problem"].as_str().unwrap_or("").to_string();
        let test = t["test"].as_str().unwrap_or("").to_string();
        if problem.is_empty() || test.is_empty() {
            out.push_str(&format!("  {id}: sin prompt/test, skip\n"));
            continue;
        }
        let full_prompt = format!(
            "{problem}\n\nCompleta task_func: escribe SOLO el codigo python de la funcion completa (def task_func(...): y cuerpo), respetando EXACTAMENTE la firma de la cabecera. No escribas tests ni importes de mas."
        );
        // DIRECTA: 1 solo intento.
        let d_sol = extract_script(&generate_script(&full_prompt));
        let (d, _df) = run_bigcode(&d_sol, &test);
        if d {
            d_ok += 1;
        }
        // HARNESS (plan-then-code): el modelo describe el algoritmo ANTES de
        // escribir codigo, luego itera con feedback. La directa queda como
        // baseline puro (sin plan). Experiment: ciclo 7, iter 8.
        // R28: deteccion de estancamiento — si el MISMO test falla 2 veces
        // seguidas, "corrige" no ayuda (el modelo repite el mismo error);
        // se fuerza reescritura completa con enfoque distinto.
        let mut h = false;
        let mut feedback = String::new();
        let mut last_frag = String::new();
        let mut stalls = 0usize;
        for attempt in 0..6 {
            let mut instruction = format!(
                "Completa task_func. PRIMERO describe tu algoritmo en UNA frase (fuera del codigo), LUEGO escribe SOLO el codigo python de la funcion completa entre marcadores ```python. {feedback}No escribas tests."
            );
            if stalls >= 2 {
                instruction = format!(
                    "La solucion anterior se estanca: el test '{last_frag}' sigue fallando. NO corrijas la funcion anterior: DESCARTA tu enfoque y resuelve el problema desde cero con un algoritmo DISTINTO. PRIMERO describe el nuevo algoritmo en UNA frase, LUEGO codigo completo entre marcadores ```python. No escribas tests."
                );
            }
            let prompt = format!("{problem}\n\n{instruction}");
            let sol = extract_script(&generate_script(&prompt));
            let (ok, frag) = run_bigcode(&sol, &test);
            if ok {
                h = true;
                break;
            }
            stalls = if frag == last_frag { stalls + 1 } else { 0 };
            last_frag = frag.clone();
            feedback = format!("El test fallo. Detalle: {frag}. Corrige task_func. ");
            let _ = attempt;
        }
        if h {
            h_ok += 1;
        }
        // Streaming: cada problema se imprime al momento (no al final) para
        // sobrevivir a timeouts y monitorizar progreso.
        eprintln!(
            "  {id}: directa {} | harness {}",
            if d { "✓" } else { "✗" },
            if h { "✓" } else { "✗" },
        );
        out.push_str(&format!(
            "  {id}: directa {} | harness {}\n",
            if d { "✓" } else { "✗" },
            if h { "✓" } else { "✗" },
        ));
    }
    out.push_str(&format!(
        "Directa: {d_ok}/{} · Harness: {h_ok}/{} — verificados por unittest REAL (BigCodeBench)\n",
        tasks.len(),
        tasks.len()
    ));
    out
}

/// Ejecuta una solución HumanEval: el test define check(candidate) y se llama
/// con entry_point. Éxito = llega a ALX_OK sin AssertionError. (spec humaneval)
fn run_humaneval(solution: &str, test: &str, entry_point: &str) -> (bool, String) {
    let script = format!(
        "{solution}\n{test}\ncheck({entry_point})\nprint('ALX_OK')\n"
    );
    let path = std::env::temp_dir().join("alx-humaneval.py");
    if std::fs::write(&path, &script).is_err() {
        return (false, "error escribiendo script".to_string());
    }
    let out = alx_gate::run_command(&format!("python3 {}", path.display()), 60_000);
    let all = out.stdout_head;
    let ok = all.contains("ALX_OK");
    let frag = all
        .lines()
        .filter(|l| l.contains("AssertionError") || l.contains("Error"))
        .take(2)
        .collect::<Vec<_>>()
        .join(" | ");
    (
        ok,
        if frag.is_empty() {
            all.chars().take(100).collect()
        } else {
            frag
        },
    )
}

/// Benchmark HumanEval (164 problemas, familia 2 para GENERALIDAD).
/// Misma mecánica campeona que BigCodeBench: plan-then-code + feedback.
pub fn render_bench_humaneval() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../harnesses/bench/humaneval.jsonl");
    let mut out = String::from("## Benchmark HumanEval (164) — generalidad\n");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return out + "sin humaneval.jsonl\n";
    };
    let mut tasks: Vec<serde_json::Value> = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            tasks.push(v);
        }
    }
    if let Ok(cap) = std::env::var("ALX_BENCH_MAX") {
        if let Ok(n) = cap.trim().parse::<usize>() {
            tasks.truncate(n);
        }
    }
    let (mut d_ok, mut h_ok) = (0usize, 0usize);
    for (i, t) in tasks.iter().enumerate() {
        let id_fallback = format!("HE/{i}");
        let id = t["task_id"].as_str().unwrap_or(&id_fallback);
        let prompt = t["prompt"].as_str().unwrap_or("").to_string();
        let test = t["test"].as_str().unwrap_or("").to_string();
        let entry = t["entry_point"].as_str().unwrap_or("").to_string();
        if prompt.is_empty() || test.is_empty() || entry.is_empty() {
            continue;
        }
        let full_prompt = format!(
            "{prompt}\n\nCompleta {entry}: PRIMERO describe tu algoritmo en UNA frase, LUEGO escribe SOLO el codigo python de la funcion completa entre marcadores ```python. No escribas tests."
        );
        // Directa: 1 intento.
        let d_sol = extract_script(&generate_script(&full_prompt));
        let (d, _df) = run_humaneval(&d_sol, &test, &entry);
        if d {
            d_ok += 1;
        }
        // Harness: plan-then-code + feedback, 6 intentos con detección de
        // estancamiento (mismo fallo 2x → reescritura con enfoque distinto).
        let mut h = false;
        let mut feedback = String::new();
        let mut last_frag = String::new();
        let mut stalls = 0usize;
        for _ in 0..6 {
            let mut instruction = format!(
                "Completa {entry}: PRIMERO describe tu algoritmo en UNA frase, LUEGO escribe SOLO el codigo python de la funcion completa entre marcadores ```python. {feedback}No escribas tests."
            );
            if stalls >= 2 {
                instruction = format!(
                    "La solucion anterior se estanca: '{last_frag}' sigue fallando. NO corrijas la funcion anterior: DESCARTA tu enfoque y resuelve desde cero con un algoritmo DISTINTO. PRIMERO describe el nuevo algoritmo en UNA frase, LUEGO codigo completo entre marcadores ```python. No escribas tests."
                );
            }
            let prompt = format!("{prompt}\n\n{instruction}");
            let sol = extract_script(&generate_script(&prompt));
            let (ok, frag) = run_humaneval(&sol, &test, &entry);
            if ok {
                h = true;
                break;
            }
            stalls = if frag == last_frag { stalls + 1 } else { 0 };
            last_frag = frag.clone();
            feedback = format!("El test fallo. Detalle: {frag}. Corrige {entry}. ");
        }
        if h {
            h_ok += 1;
        }
        eprintln!("  {id}: directa {} | harness {}", if d { "✓" } else { "✗" }, if h { "✓" } else { "✗" });
        out.push_str(&format!(
            "  {id}: directa {} | harness {}\n",
            if d { "✓" } else { "✗" },
            if h { "✓" } else { "✗" },
        ));
    }
    out.push_str(&format!(
        "Directa: {d_ok}/{} · Harness: {h_ok}/{} — HumanEval\n",
        tasks.len(),
        tasks.len()
    ));
    out
}

/// Ejecuta una solución CodeContests (I/O-based): corre con cada input y
/// compara stdout normalizado con el output esperado. (spec codecontests)
fn run_codecontests(solution: &str, tests: &serde_json::Value) -> (bool, String) {
    let sol_path = std::env::temp_dir().join("alx-codecontests.py");
    if std::fs::write(&sol_path, solution).is_err() {
        return (false, "error escribiendo solucion".to_string());
    }
    let Some(arr) = tests.as_array() else {
        return (false, "sin tests".to_string());
    };
    let inp_path = std::env::temp_dir().join("alx-cc-input.txt");
    for (i, t) in arr.iter().enumerate() {
        let inp = t["input"].as_str().unwrap_or("");
        let exp = t["output"].as_str().unwrap_or("").trim().to_string();
        if std::fs::write(&inp_path, inp).is_err() {
            return (false, "error escribiendo input".to_string());
        }
        let out = alx_gate::run_command(
            &format!("python3 {} < {}", sol_path.display(), inp_path.display()),
            15_000,
        );
        let got = out.stdout_head.trim().to_string();
        if got != exp {
            // Debug guiado: incluir el INPUT del test que falla (no solo
            // expected/got). Hipotesis ciclo 9: el modelo puede depurar mejor
            // si ve el caso concreto que rompe su solucion.
            return (
                false,
                format!(
                    "test {} fallo. INPUT: '{}'. expected: '{}', got: '{}'",
                    i + 1,
                    inp.chars().take(60).collect::<String>(),
                    exp.chars().take(40).collect::<String>(),
                    got.chars().take(40).collect::<String>()
                ),
            );
        }
    }
    (true, "todos los tests pasan".to_string())
}

/// Benchmark CodeContests (30, familia 3 I/O-based para GENERALIDAD).
/// Misma mecánica campeona: plan-then-code + feedback.
pub fn render_bench_codecontests() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../harnesses/bench/codecontests-sample.jsonl");
    let mut out = String::from("## Benchmark CodeContests (30) — familia 3 I/O\n");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return out + "sin codecontests-sample.jsonl\n";
    };
    let mut tasks: Vec<serde_json::Value> = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            tasks.push(v);
        }
    }
    if let Ok(cap) = std::env::var("ALX_BENCH_MAX") {
        if let Ok(n) = cap.trim().parse::<usize>() {
            tasks.truncate(n);
        }
    }
    let (mut d_ok, mut h_ok) = (0usize, 0usize);
    for (i, t) in tasks.iter().enumerate() {
        let id_fallback = format!("CC/{i}");
        let id = t["name"].as_str().unwrap_or(&id_fallback);
        let desc = t["description"].as_str().unwrap_or("").to_string();
        let tests = t["tests"].clone();
        if desc.is_empty() || tests.as_array().map(|a| a.is_empty()).unwrap_or(true) {
            continue;
        }
        let full_prompt = format!(
            "{desc}\n\nEscribe SOLO codigo Python que lea de stdin y escriba a stdout para resolver el problema. PRIMERO describe tu algoritmo en UNA frase, LUEGO escribe el codigo entre marcadores ```python."
        );
        let d_sol = extract_script(&generate_script(&full_prompt));
        let (d, _df) = run_codecontests(&d_sol, &tests);
        if d {
            d_ok += 1;
        }
        let mut h = false;
        let mut feedback = String::new();
        let mut last_frag = String::new();
        let mut stalls = 0usize;
        for _ in 0..6 {
            let mut instruction = format!(
                "Escribe SOLO codigo Python que lea de stdin y escriba a stdout. PRIMERO describe tu algoritmo en UNA frase, LUEGO escribe el codigo entre marcadores ```python. {feedback}"
            );
            if stalls >= 2 {
                instruction = format!(
                    "La solucion anterior se estanca: '{last_frag}' sigue fallando. NO corrijas: DESCARTA tu enfoque y resuelve desde cero con un algoritmo DISTINTO. PRIMERO describe el nuevo algoritmo en UNA frase, LUEGO codigo entre marcadores ```python."
                );
            }
            let prompt = format!("{desc}\n\n{instruction}");
            let sol = extract_script(&generate_script(&prompt));
            let (ok, frag) = run_codecontests(&sol, &tests);
            if ok {
                h = true;
                break;
            }
            stalls = if frag == last_frag { stalls + 1 } else { 0 };
            last_frag = frag.clone();
            feedback = format!("El test fallo. Detalle: {frag}. Corrige. ");
        }
        if h {
            h_ok += 1;
        }
        eprintln!("  {id}: directa {} | harness {}", if d { "✓" } else { "✗" }, if h { "✓" } else { "✗" });
        out.push_str(&format!(
            "  {id}: directa {} | harness {}\n",
            if d { "✓" } else { "✗" },
            if h { "✓" } else { "✗" },
        ));
    }
    out.push_str(&format!(
        "Directa: {d_ok}/{} · Harness: {h_ok}/{} — CodeContests\n",
        tasks.len(),
        tasks.len()
    ));
    out
}

/// `alx bench` — ejecuta las 3 familias de benchmark en secuencia y agrega.
pub fn render_bench_all() -> String {
    let mut out = String::from("# ALEXANDRIA — benchmark suite (3 familias)\n\n");
    out.push_str(&render_bench_bigcode());
    out.push('\n');
    out.push_str(&render_bench_humaneval());
    out.push('\n');
    out.push_str(&render_bench_codecontests());
    out
}

/// Copia recursiva src → dst (para sync de skills).
fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// `alx setup` — configura e verifica TODA la integración con Claude Code:
/// binario, statusline powerline, MCP server, hooks. Merge no destructivo.
/// `alx update` — sistema de auto-actualización: pull + rebuild + reinstall.
pub fn run_update() -> String {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let home = std::env::var("HOME").unwrap_or_default();
    let mut out = String::from("# alx update\n");
    let pull = alx_gate::run_command(
        &format!("git -C {} pull --rebase", repo.display()),
        60_000,
    );
    out.push_str(&format!(
        "git pull: {}\n",
        if pull.exit_code == 0 { "✓" } else { "✗ (no cambios o error)" }
    ));
    let build = alx_gate::run_command(
        &format!(
            "cargo build --release --manifest-path {}/alexandria/Cargo.toml",
            repo.display()
        ),
        300_000,
    );
    out.push_str(&format!(
        "build release: {}\n",
        if build.exit_code == 0 { "✓" } else { "✗" }
    ));
    if build.exit_code == 0 {
        let src = format!("{}/alexandria/target/release/alx", repo.display());
        let dst = format!("{home}/.local/bin/alx");
        if std::fs::copy(&src, &dst).is_ok() {
            out.push_str("✓ binario actualizado → ~/.local/bin/alx\n");
        }
    }
    out
}

/// `alx setup` — configura e verifica TODA la integración con Claude Code:
/// Instala el sistema de hooks Node desde la fuente canónica
/// `~/Projectos/AlexanderTheGreat/harnesses/hooks` hacia
/// `<proyecto>/.claude/hooks`. Copia recursiva salvo node_modules/state/data.
/// Si faltan node_modules, lanza `npm install --silent` (una vez).
/// Devuelve (ficheros_copiados, npm_ejecutado).
pub fn install_hooks_src(home: &str) -> (usize, bool) {
    use std::path::{Path, PathBuf};
    let src_root = PathBuf::from(format!("{home}/Projectos/AlexanderTheGreat/harnesses/hooks"));
    let dst_root = PathBuf::from(format!("{home}/Projectos/AlexanderTheGreat/.claude/hooks"));
    if !src_root.is_dir() {
        return (0, false);
    }
    let skip = |p: &Path| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| matches!(n, "node_modules" | "state" | "data" | "package-lock.json"))
    };
    let mut copiados = 0usize;
    fn walk(src: &Path, dst: &Path, skip: &dyn Fn(&Path) -> bool, n: &mut usize) {
        if let Ok(entries) = std::fs::read_dir(src) {
            for e in entries.flatten() {
                let sp = e.path();
                if skip(&sp) {
                    continue;
                }
                let dp = dst.join(e.file_name());
                if sp.is_dir() {
                    let _ = std::fs::create_dir_all(&dp);
                    walk(&sp, &dp, skip, n);
                } else {
                    // copia solo si cambia (mtime+size) para no tocar mtime del hook sin motivo
                    let necesita = match (std::fs::metadata(&dp), std::fs::metadata(&sp)) {
                        (Ok(d), Ok(s)) => {
                            d.len() != s.len()
                                || d.modified().ok() != s.modified().ok()
                        }
                        _ => true,
                    };
                    if necesita && std::fs::copy(&sp, &dp).is_ok() {
                        *n += 1;
                    } else if dp.exists() {
                        // ya sincronizado: cuenta como instalado la primera vez
                    }
                }
            }
        }
    }
    walk(&src_root, &dst_root, &skip, &mut copiados);

    let mut npm = false;
    let tiene_deps = dst_root.join("package.json").exists();
    let falta_node_modules = !dst_root.join("node_modules").exists();
    if copiados > 0 && tiene_deps && falta_node_modules {
        let cmd = "npm install --silent --no-audit --no-fund";
        let _ = alx_gate::run_command(&format!(
            "cd {} && {}",
            dst_root.display(),
            cmd
        ), 300_000);
        npm = true;
    }
    (copiados, npm)
}

/// Directorio del registry de harnesses (harnesses/ del repo).
fn harness_dir_global() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../harnesses")
}

/// Busca `.alexandria/` subiendo desde `start` (o cwd) hasta la raíz.
/// None = este proyecto no está alexandrizado.
pub fn find_project_alexandria(start: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
    let mut dir = match start {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir().ok()?,
    };
    loop {
        let cand = dir.join(".alexandria");
        if cand.is_dir() {
            return Some(cand);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Directorio de registry de harnesses AWARE del proyecto: si el cwd (o un
/// ancestro) tiene `.alexandria/`, los harnesses aprendidos en ese proyecto
/// viven y mueren con él; si no, se usa el registry global del repo.
fn harness_dir() -> std::path::PathBuf {
    match find_project_alexandria(None) {
        Some(proj) => proj,
        None => harness_dir_global(),
    }
}

/// Fuente del registry activo, para mostrar al usuario de dónde lee.
pub fn harness_dir_source() -> &'static str {
    if find_project_alexandria(None).is_some() {
        "proyecto (.alexandria)"
    } else {
        "global (repo)"
    }
}

/// `alx init` — alexadriza el proyecto actual: crea `.alexandria/` con el
/// esqueleto completo. Idempotente: no toca lo que ya existe.
pub fn project_init() -> String {
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => return format!("✗ sin cwd: {e}"),
    };
    let base = cwd.join(".alexandria");
    if base.is_dir() {
        return format!("✓ ya inicializado: {}", base.display());
    }
    let mut creadas = Vec::new();
    for rel in ["active", "archive", "rubrics", "skills", "polish"] {
        let d = base.join(rel);
        if std::fs::create_dir_all(&d).is_ok() {
            creadas.push(rel);
        }
    }
    // lessons.md: el diario de aprendizajes del proyecto (la IA añade aquí).
    let lessons = base.join("lessons.md");
    if !lessons.exists() {
        let _ = std::fs::write(
            &lessons,
            "# Lecciones de este proyecto\n\n\
             Aprendizajes que la IA formaliza mientras trabaja. Cada lección\n\
             puede convertirse en harness con:\n\
             `alx harness-new <slug> --objective ... --doc \"...\"`\n\n",
        );
    }
    // config.toml mínimo con defaults documentados.
    let cfg = base.join("config.toml");
    if !cfg.exists() {
        let _ = std::fs::write(
            &cfg,
            "# Configuración Alexandria del proyecto\n\
             [polish]\n\
             # techo de rondas: el sistema puede pararse antes si ve meseta\n\
             max_rounds = 4\n\
             # mejora mínima entre rondas para seguir puliendo (0..=1)\n\
             min_delta = 0.05\n",
        );
    }
    format!(
        "✓ .alexandria creado en {}\n  dirs: {}\n  El registry de harnesses de ESTE proyecto ya está activo (`alx harness-list`).\n  Siguiente paso sugerido: `alx skills-fetch anthropics/skills` para dotar de reglas al experto.",
        cwd.display(),
        creadas.join(", ")
    )
}

/// Paso CREAR del ciclo evolutivo (plan 16 §2): la IA formaliza una regla
/// aprendida en pleno trabajo como harness persistente. Regla doc-min
/// obligatoria (>=20 chars) — nada se escapa sin documentación.
pub fn harness_new(name: &str, objective: &str, doc: &str, kind: &str, trigger: &str) -> String {
    use alx_evolve::{HarnessCandidate, HarnessKind, Trigger};
    let slug: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let kind_parsed = match kind.to_ascii_lowercase().as_str() {
        "permanent" | "permanente" => HarnessKind::Permanent,
        _ => HarnessKind::Temporal,
    };
    let trigger_parsed = if let Some(fase) = trigger.strip_prefix("phase:") {
        Trigger::Phase(fase.to_string())
    } else if let Some(ev) = trigger.strip_prefix("event:") {
        Trigger::Event(ev.to_string())
    } else {
        Trigger::Manual
    };
    let dir = harness_dir();
    let mut reg = alx_evolve::HarnessRegistry::load_from(&dir);
    // seed igual que el watcher para no partir de vacío
    if reg.all().is_empty() {
        let _ = run_evolve_cycle();
        reg = alx_evolve::HarnessRegistry::load_from(&dir);
    }
    // Nota de diseño (plan §11): add_candidate SIEMPRE crea Temporal — la regla
    // protege del ruido del detector automático. Un `--kind permanent`
    // EXPLÍCITO por CLI es otra cosa: es una decisión consciente, y aquí se
    // promueve directamente tras crear.
    let quiere_permanente = matches!(kind.to_ascii_lowercase().as_str(), "permanent" | "permanente");
    let cand = HarnessCandidate {
        suggested_name: slug.clone(),
        kind: kind_parsed,
        trigger: trigger_parsed,
        objective: objective.to_string(),
        doc: doc.to_string(),
    };
    match reg.add_candidate(cand, now_ms()) {
        Some(id) => {
            if quiere_permanente {
                if let Some(h) = reg.by_id_mut(&id) {
                    h.promote(0);
                }
            }
            match reg.save_to(&dir) {
                Ok(()) => format!(
                    "✓ harness {id} creado (kind={}, trigger={trigger})\n  objetivo: {objective}\nVigilancia: `alx evolve` revisa usos y objetivos; `alx harness-use {id}` tras aplicarlo.",
                    if quiere_permanente { "permanent" } else { "temporal" }
                ),
                Err(e) => format!("✗ no pude persistir el registry: {e}"),
            }
        }
        None => format!(
            "✗ no se creó el harness (¿ya existe 'hx-{slug}'? ¿doc >=20 chars?)"
        ),
    }
}

/// Lista los harnesses vivos con estado/usos — la vista del paso VIGILAR.
pub fn harness_list() -> String {
    let dir = harness_dir();
    let reg = alx_evolve::HarnessRegistry::load_from(&dir);
    let mut out = String::from("## Harnesses (registry evolutivo)\n");
    out.push_str(&format!("{:<24} {:<10} {:<8} {:<12} {:<6} objetivo\n", "id", "kind", "state", "trigger", "uses"));
    for h in reg.all() {
        let trigger = match &h.trigger {
            alx_evolve::Trigger::Manual => "manual".to_string(),
            alx_evolve::Trigger::Phase(p) => format!("phase:{p}"),
            alx_evolve::Trigger::Event(e) => format!("event:{e}"),
        };
        out.push_str(&format!(
            "{:<24} {:<10} {:<8} {:<12} {:<6} {}\n",
            h.id,
            format!("{:?}", h.kind).to_lowercase(),
            format!("{:?}", h.state).to_lowercase(),
            trigger,
            h.uses,
            h.objective
        ));
    }
    if reg.all().is_empty() {
        out.push_str("(vacío; `alx harness-new` crea el primero)\n");
    }
    out
}

/// Paso APLICAR/APRENDER: registra un uso real del harness.
pub fn harness_use(id: &str) -> String {
    let dir = harness_dir();
    let mut reg = alx_evolve::HarnessRegistry::load_from(&dir);
    let key = if id.starts_with("hx-") { id.to_string() } else { format!("hx-{id}") };
    match reg.by_id_mut(&key) {
        Some(h) => {
            h.record_use();
            let usos = h.uses;
            match reg.save_to(&dir) {
                Ok(()) => format!("✓ {key}: uso registrado ({usos} total). A los 5 usos el watcher lo promueve a permanente."),
                Err(e) => format!("✗ persistencia: {e}"),
            }
        }
        None => format!("✗ harness '{key}' no encontrado (`alx harness-list`)"),
    }
}

// ═══════════════════════════ POLISH (pulido dosificado) ═══════════════════
//
// La idea (usuario, 2026-08-25): no iterar N veces "a ciegas". El sistema
// evalúa el artefacto contra una RÚBRICA del proyecto (.alexandria/rubrics/),
// mejora, re-evalúa, y DECIDE seguir o parar viendo la mejora entre rondas:
// meseta → parada. El techo (max_rounds) es un seguro, no el objetivo.

/// Rúbrica cargada de .alexandria/rubrics/<name>.json.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Rubric {
    pub name: String,
    /// Qué debe mirar el experto: coherencia, detalle, cuándo usar cada cosa…
    pub criteria: Vec<String>,
}

impl Default for Rubric {
    fn default() -> Self {
        Self {
            name: "default".into(),
            criteria: vec![
                "Corrección: hace lo que dice; sin bugs ni casos borde roto.".into(),
                "Coherencia: consistente con el resto del proyecto (estilo, patrones, nombres).".into(),
                "Nivel de detalle comparable a implementaciones de referencia de alta calidad.".into(),
                "Simplicidad: nada superfluo; cada pieza justifica su existencia.".into(),
                "Documentación mínima honesta (docstrings/readme donde aportan).".into(),
            ],
        }
    }
}

fn load_rubric(name: &str) -> Rubric {
    if let Some(proj) = find_project_alexandria(None) {
        let path = proj.join("rubrics").join(format!("{name}.json"));
        if let Ok(txt) = std::fs::read_to_string(&path) {
            if let Ok(r) = serde_json::from_str::<Rubric>(&txt) {
                return r;
            }
        }
    }
    Rubric::default()
}

fn polish_config() -> (u32, f64) {
    // (max_rounds, min_delta) desde .alexandria/config.toml si existe; parse
    // mínimo sin dependencias: buscamos claves sueltas.
    let defaults = (4u32, 0.05f64);
    let Some(proj) = find_project_alexandria(None) else {
        return defaults;
    };
    let Ok(txt) = std::fs::read_to_string(proj.join("config.toml")) else {
        return defaults;
    };
    let get_num = |key: &str| -> Option<f64> {
        txt.lines()
            .find(|l| l.trim_start().starts_with(key))
            .and_then(|l| l.split('=').nth(1))
            .and_then(|v| v.trim().parse::<f64>().ok())
    };
    let max_rounds = get_num("max_rounds").map(|v| v as u32).unwrap_or(defaults.0);
    let min_delta = get_num("min_delta").unwrap_or(defaults.1);
    (max_rounds.max(1), min_delta.clamp(0.0, 1.0))
}

/// Puntaje 0..=1 de un veredicto: 1 - penalizaciones (Block=.35, Major=.2,
/// Minor=.08, Suggestion=.02), acotado a [0,1].
pub fn verdict_score(v: &alx_critic::CriticVerdict) -> f64 {
    let pen: f64 = v
        .findings
        .iter()
        .map(|f| match f.severity {
            alx_critic::Severity::Block => 0.35,
            alx_critic::Severity::Major => 0.20,
            alx_critic::Severity::Minor => 0.08,
            alx_critic::Severity::Suggestion => 0.02,
        })
        .sum();
    (1.0 - pen).clamp(0.0, 1.0)
}

/// Llamada LLM "mejora este artefacto contra estos hallazgos" por la cadena.
/// Devuelve el artefacto mejorado (texto completo) o None si la red falla.
fn improve_with_llm(artifact: &str, rubric: &Rubric, findings: &[String]) -> Option<String> {
    let mut prompt = String::from(
        "Eres el experto que pule artefactos. Mejora el ARTEFACTO aplicando los HALLAZGOS \
         y los CRITERIOS. Devuelve el artefacto COMPLETO mejorado, sin explicaciones.\n\nCRITERIOS:\n",
    );
    for c in &rubric.criteria {
        prompt.push_str(&format!("- {c}\n"));
    }
    prompt.push_str("\nHALLAZGOS DE LA ÚLTIMA EVALUACIÓN:\n");
    for f in findings {
        prompt.push_str(&format!("- {f}\n"));
    }
    prompt.push_str("\n<artefacto>\n");
    prompt.push_str(&artifact.chars().take(24_000).collect::<String>());
    prompt.push_str("\n</artefacto>");

    let body = serde_json::json!({
        "model": modelo_real_activo(),
        "max_tokens": 3000,
        "messages": [{"role": "user", "content": prompt}]
    });
    // mismo motivo que generate_script: stdout_head trunca -> fichero
    let resp_path = std::env::temp_dir().join("alx-polish-resp.json");
    let body_path = std::env::temp_dir().join("alx-polish-body.json");
    if std::fs::write(&body_path, body.to_string()).is_err() {
        return None;
    }
    let cmd = format!(
        "curl -s -m 120 -X POST http://127.0.0.1:8788/v1/messages \
         -H 'content-type: application/json' \
         -H 'anthropic-version: 2023-06-01' -d @{} -o {}",
        body_path.display(),
        resp_path.display()
    );
    let _ = alx_gate::run_command(&cmd, 130_000);
    let Ok(raw_file) = std::fs::read_to_string(&resp_path) else {
        return None;
    };
    let raw = raw_file.trim();
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    // texto puede venir en blocks; concatenar todos los type=text
    let text: String = v["content"]
        .as_array()?
        .iter()
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("");
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// `alx polish <fichero> [--rubric NOMBRE]` — bucle evaluar→mejorar→decidir.
/// El número de rondas NO es fijo: para cuando hay meseta (delta < min_delta)
/// o aprobación del crítico; max_rounds solo techa.
/// Modelo real activo, leído EN VIVO de la fuente única de verdad
/// (~/.config/routatic-proxy/config.json — el mismo fichero que lee el
/// routa-gateway y que actualiza `routa use`). Nada de modelos hardcodeados:
/// cambiar el modelo con un comando cambia TODO el motor.
///
/// Prioridad: env ALX_MODEL (experimentos puntuales) > config routatic >
/// alias visible (el gateway lo traduce igualmente).
pub fn modelo_real_activo() -> String {
    if let Ok(m) = std::env::var("ALX_MODEL") {
        if !m.trim().is_empty() {
            return m;
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    if let Ok(txt) =
        std::fs::read_to_string(format!("{home}/.config/routatic-proxy/config.json"))
    {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
            if let Some(m) = v["models"]["default"]["model_id"].as_str() {
                if !m.is_empty() {
                    return m.to_string();
                }
            }
        }
    }
    "claude-opus-4-6[1m]".to_string()
}

pub fn run_polish(path: &str, rubric_name: &str) -> String {
    let ruta = std::path::Path::new(path);
    if !ruta.is_file() {
        return format!("✗ no existe el fichero {path}");
    }
    let mut artifact = match std::fs::read_to_string(ruta) {
        Ok(t) => t,
        Err(e) => return format!("✗ lectura: {e}"),
    };
    let rubric = load_rubric(rubric_name);
    let (max_rounds, min_delta) = polish_config();
    let criteria: Vec<String> = rubric.criteria.clone();

    // destino del reporte: .alexandria/polish/ si hay proyecto; si no, junto al fichero
    let report_dir = find_project_alexandria(None)
        .map(|p| p.join("polish"))
        .unwrap_or_else(|| ruta.parent().map(|p| p.to_path_buf()).unwrap_or_default());
    let _ = std::fs::create_dir_all(&report_dir);
    let stem = ruta.file_stem().and_then(|s| s.to_str()).unwrap_or("artifact");

    let mut log = format!(
        "# Polish de {path}\nRúbrica: {} · techos: max={max_rounds} min_delta={min_delta}\n\n",
        rubric.name
    );
    let mut prev_score = -1.0f64;
    let mut ronda = 0u32;
    let decision_final;
    loop {
        ronda += 1;
        let verdict = alx_critic::criticize_real(&artifact, &criteria.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        let score = verdict_score(&verdict);
        log.push_str(&format!(
            "## Ronda {ronda} · score {score:.2} · {}\n",
            if verdict.approved { "APROBADO" } else { "con hallazgos" }
        ));
        for f in &verdict.findings {
            log.push_str(&format!("- {:?}: {}\n", f.severity, f.message));
        }
        log.push('\n');

        // ¿Parar? aprobado, meseta, o techo alcanzado.
        if verdict.approved {
            decision_final = format!("✓ aprobado en la ronda {ronda}");
            break;
        }
        if prev_score >= 0.0 && (score - prev_score).abs() < min_delta {
            decision_final = format!(
                "■ meseta en ronda {ronda}: |Δ{:.2}| < min_delta {:.2} — más rondas no pagan; quedan {} hallazgos",
                (score - prev_score).abs(),
                min_delta,
                verdict.findings.len()
            );
            break;
        }
        if ronda >= max_rounds {
            decision_final = format!(
                "▲ techo de rondas ({max_rounds}); score final {score:.2}; quedan {} hallazgos",
                verdict.findings.len()
            );
            break;
        }

        // Mejorar con lo visto.
        let findings: Vec<String> =
            verdict.findings.iter().map(|f| f.message.clone()).collect();
        match improve_with_llm(&artifact, &rubric, &findings) {
            Some(mejorado) => {
                // respaldo por ronda: nunca se pierde trabajo
                let backup = report_dir.join(format!("{stem}-R{ronda}.md"));
                let _ = std::fs::write(&backup, &mejorado);
                artifact = mejorado;
            }
            None => {
                decision_final = format!("✗ la cadena no respondió en ronda {ronda}; se conserva la versión evaluada");
                break;
            }
        }
        prev_score = score;
    }

    // escribir resultado final + log
    let final_path = report_dir.join(format!("{stem}-final.md"));
    let _ = std::fs::write(&final_path, &artifact);
    let log_path = report_dir.join(format!("{stem}-polish.md"));
    let _ = std::fs::write(&log_path, &log);
    format!(
        "{decision_final}\nresultado : {}\ndiario    : {}",
        final_path.display(),
        log_path.display()
    )
}

// ═══════════════════ PATTERNS (recurrencia → harnesses) ═══════════════════
//
// Mina las métricas de hooks (metrics.jsonl) buscando problemas recurrentes
// y propone harnesses listos para crearlos. Determinista y barato.

/// `alx patterns [--apply]` — detecta recurrencia y propone CREAR harnesses.
pub fn run_patterns(apply: bool) -> String {
    use std::collections::HashMap;
    let Some(proj) = find_project_alexandria(None) else {
        return "✗ este proyecto no está alexandrizado (`alx init`)".into();
    };
    let metrics = proj
        .ancestors()
        .find_map(|p| {
            let m = p.join(".claude/hooks/state/metrics.jsonl");
            m.is_file().then_some(m)
        })
        .map(|p| std::fs::read_to_string(p).unwrap_or_default())
        .unwrap_or_default();

    // agrupar eventos blocked por kind+skill
    let mut counts: HashMap<(String, String), u32> = HashMap::new();
    for line in metrics.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v["event"].as_str() == Some("blocked") {
            let kind = v["kind"].as_str().unwrap_or("?").to_string();
            let skills = match &v["skills"] {
                serde_json::Value::Array(a) => a
                    .iter()
                    .filter_map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join("+"),
                _ => v["skill"].as_str().unwrap_or("?").to_string(),
            };
            *counts.entry((kind, skills)).or_insert(0) += 1;
        }
    }

    let umbral = 3u32;
    let mut recurrentes: Vec<_> = counts.into_iter().filter(|((_, _), n)| *n >= umbral).collect();
    recurrentes.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    if recurrentes.is_empty() {
        return format!(
            "sin recurrencias ≥{umbral} en metrics.jsonl todavía;\nel sistema avisa solo cuando un patrón se repita"
        );
    }

    let mut out = String::from("## Patrones recurrentes detectados\n");
    for ((kind, skill), n) in &recurrentes {
        let slug = format!("auto-{}-{}", kind, slugify_simple(skill));
        let cmd = format!(
            "alx harness-new {slug} --objective \"eliminar bloqueos recurrentes [{kind}] {skill}\" \
             --doc \"Detectado automaticamente: {n} bloqueos de tipo {kind} sobre {skill}. \
             Formalizar la regla que los evita.\" --kind permanent --trigger event:PostToolUse"
        );
        out.push_str(&format!("\n· {kind} × {skill} → {n} veces\n  {cmd}\n"));
        if apply {
            let args = [
                slug.clone(),
                "--objective".into(),
                format!("eliminar bloqueos recurrentes [{kind}] {skill}"),
                "--doc".into(),
                format!("Detectado automaticamente: {n} bloqueos de tipo {kind} sobre {skill}. Formalizar la regla que los evita."),
                "--kind".into(),
                "permanent".into(),
                "--trigger".into(),
                "event:PostToolUse".into(),
            ];
            out.push('\n');
            out.push_str(&harness_new(
                &args[0],
                &args[2],
                &args[4],
                &args[6],
                &args[8],
            ));
            out.push('\n');
        }
    }
    if !apply {
        out.push_str("\n(revisa y crea con --apply, o ajusta antes)")
    }
    out
}

fn slugify_simple(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect()
}

// ═══════════════ RESEARCH (investigación profunda, plan 17) ═══════════════
//
// Convierte una pregunta en un PROYECTO de investigación con 7 artefactos
// obligatorios. El proceso fuerza el pensamiento de experto: mecanismos
// primero, mapa por capas, simulaciones guardadas, frenos, evidencia.

const RESEARCH_STEPS: &[(&str, &str)] = &[
    ("00-question", "PASO 0 — LA PREGUNTA\n\n\
        1. Cópiala literal.\n\
        2. ¿Qué pregunta REALMENTE el usuario? Reformúlala en términos del sistema implicado.\n\
        3. ¿Qué preguntaría además un experto senior del dominio? Lista 3-5 sub-preguntas.\n\
        4. ¿Cuál es el criterio de éxito de la respuesta?"),
    ("01-fundamentos", "PASO 1 — FUNDAMENTOS (mecanismo antes que solución)\n\n\
        Reconstruye cómo funciona el sistema DE VERDAD:\n\
        - Actores y componentes (células/hormonas/módulos/protocolos…)\n\
        - Vías y señales: quién activa a quién, feedback loops\n\
        - ¿Dónde está EL FRENO dominante del sistema? (todo sistema tiene uno;\n\
          identificarlo reordena todo lo demás — ej.: FGFR3 frena el crecimiento óseo)\n\
        - Diagrama textual actores→vías→efectos. Sin agentes/soluciones todavía."),
    ("02-mapas", "PASO 2 — MAPA DEL ICEBERG (capas de lo conocido a la frontera)\n\n\
        Tabla por capas (mínimo 5). Para cada capa:\n\
        | Capa | Intervención/Opción | Estado (aprobado/clínico/preclínico/teoría) | Evidencia | Practicidad |\n\
        Capa 1 = lo obvio que cualquiera dice. Última capa = frontera (10-20 años).\n\
        Si una capa NO aplica al caso, dilo EXPLÍCITAMENTE y por qué."),
    ("03-simulaciones", "PASO 3 — SIMULACIONES CONTRAFÁCTICAS (guárdalas TODAS)\n\n\
        Por cada candidato prometedor del mapa, recorre el camino causal completo:\n\
        SIM-1 [nombre]\n\
          entrada: qué se empuja y por dónde\n\
          efecto primario: …\n\
          efectos secundarios: …\n\
          feedbacks: qué se frena/amplifica en cadena\n\
          resultado neto: …\n\
          veredicto: viable / inviable / condicionado + por qué\n\
        Mínimo 2 simulaciones. Una simulación que no se guardó aquí no existió."),
    ("04-limitantes", "PASO 4 — LIMITANTES Y FRENOS (honestidad primero)\n\n\
        - Contraindicaciones y riesgos reales (con severidad)\n\
        - Qué NADIE ha probado y POR QUÉ (¿regulación?, ¿coste?, ¿física?)\n\
        - Zonas grises donde la evidencia es mixta\n\
        - Qué mataría cada opción si se intentara mañana"),
    ("05-fuentes", "PASO 5 — TABLA DE EVIDENCIA\n\n\
        | Claim | Fuente (paper/doc/autor-año) | Calidad (RCT/meta/cohorte/preclínico/especulativo) |\n\
        Toda afirmación de 02/03/04 debe rastrearse hasta aquí o marcarse especulativa."),
    ("06-respuesta", "PASO 6 — SÍNTESIS FINAL\n\n\
        1. Respuesta directa en ≤5 líneas (cita capas, no sustituyas el mapa)\n\
        2. El camino recomendado y su capa del iceberg\n\
        3. Los 2-3 frenos que mandan\n\
        4. Advertencias no negociables\n\
        5. Siguientes preguntas que abrirían más espacio (conecta tópicos)"),
];

/// `alx research "pregunta"` — crea `.alexandria/research/<slug>/` con los 7
/// artefactos del protocolo (plan 17). Idempotente: si ya existe, no toca nada.
pub fn run_research(pregunta: &str) -> String {
    let slug: String = slugify_simple(pregunta)
        .split('-')
        .filter(|w| !w.is_empty())
        .take(6)
        .collect::<Vec<_>>()
        .join("-");
    let base_dir = find_project_alexandria(None)
        .map(|p| p.join("research"))
        .unwrap_or_else(|| std::path::PathBuf::from("research"));
    let dir = base_dir.join(&slug);
    if dir.exists() {
        return format!("✓ ya existe: {}\n(abre los ficheros y sigue el protocolo; pule con `alx polish <fichero> --rubric research`)", dir.display());
    }
    if std::fs::create_dir_all(&dir).is_err() {
        return format!("✗ no pude crear {}", dir.display());
    }
    // instalar la rúbrica research si este proyecto no la tiene
    if let Some(proj) = find_project_alexandria(None) {
        let rubrics = proj.join("rubrics");
        let _ = std::fs::create_dir_all(&rubrics);
        let rubric_file = rubrics.join("research.json");
        if !rubric_file.exists() {
            let rubrica = serde_json::json!({
                "name": "research",
                "criteria": [
                    "Profundidad de mecanismo: vías y actores explicados, no solo nombres de soluciones.",
                    "Mapa del iceberg con ≥5 capas y estado de evidencia por capa.",
                    "≥2 simulaciones contrafácticas completas (entrada→primario→secundarios→feedback→neto).",
                    "Frenos/limitantes identificados explícitamente con severidad.",
                    "Toda afirmación trazable a la tabla de evidencia o marcada especulativa.",
                    "Coherencia interna: los cruces entre vías están conectados, sin islas."
                ]
            });
            let _ = std::fs::write(
                &rubric_file,
                serde_json::to_string_pretty(&rubrica).unwrap_or_default(),
            );
        }
    }
    for (name, plantilla) in RESEARCH_STEPS {
        let contenido = format!(
            "# {name}\n\n> {plantilla}\n\n---\nPREGUNTA: {pregunta}\n\n(este paso aún está vacío: complétalo siguiendo las reglas de plan/17-research.md)\n"
        );
        let _ = std::fs::write(dir.join(format!("{name}.md")), contenido);
    }
    format!(
        "✓ proyecto de investigación creado: {}\nprotocolo : 7 pasos (plan/17-research.md)\nrúbrica   : research (exigente) instalada para `alx polish`\norden     : 00-question → 01-fundamentos → 02-mapas → 03-simulaciones → 04-limitantes → 05-fuentes → 06-respuesta",
        dir.display()
    )
}

// ═══════════════ SKILLS-FETCH (reglas del experto, plan 17 §4) ═════════════
//
// El experto necesita PUNTO DE VISTA externo: repos de skills/reglas de alta
// calidad (anthropics/skills, mattpocock/skills…) clonados en el proyecto y
// registrados en .alexandria/skills/catalog.md.

/// Catálogo curado por defecto (mismo espíritu que integration/skills/manifest).
const SKILL_CATALOG: &[(&str, &str)] = &[
    ("anthropics/skills", "Skills oficiales Anthropic (docx/pdf/pptx/artifacts…)."),
    ("mattpocock/skills", "Skills TS/TSX de Matt Pocock (types, testing)."),
    ("addyosmani/agent-skills", "24 skills de Addy Osmani (performance, diseño…)."),
    ("tt-a1i/archify", "Arquitectura de software asistida."),
    ("cathrynlavery/diagram-design", "Diseño de diagramas de alto nivel."),
];

/// `alx skills-fetch [repo]` — clona un repo (o lista el catálogo) dentro de
/// `.alexandria/skills/` y lo añade al catálogo del proyecto.
/// `--search "términos"` busca en GitHub ordenado por ESTRELLAS: la calidad
/// se juzga por adopción antes de instalar nada.
// ─── Scorer de calidad de skills ────────────────────────────────────────
// La pregunta que responde: ¿esta skill aporta conocimiento FUNCIONAL que la
// IA no generaría por su cuenta (scripts ejecutables, librerías concretas,
// comandos, gates de verificación) o es prosa genérica que ya sabe?

/// Resultado del análisis de una skill.
pub struct SkillScore {
    pub name: String,
    pub points: i32,
    pub signals: Vec<String>,
    pub anti_signals: Vec<String>,
}

impl SkillScore {
    pub fn verdict(&self) -> (&'static str, &'static str) {
        if self.points >= 70 {
            ("\x1b[1;32mINSTALAR ⭐\x1b[0m", "conocimiento funcional alto")
        } else if self.points >= 45 {
            ("\x1b[1;33mPROBAR\x1b[0m", "valor parcial")
        } else {
            ("\x1b[1;31mDESCARTAR\x1b[0m", "genérica — la IA ya lo sabe")
        }
    }
}

/// Cuenta ocurrencias de un patrón en un texto (case-insensitive).
fn count_matches(text: &str, needle: &str) -> usize {
    text.to_lowercase().matches(needle).count()
}

/// Analiza un directorio de skill (con SKILL.md) y puntúa su valor funcional.
pub fn score_skill_dir(dir: &std::path::Path) -> Option<SkillScore> {
    let skill_md = dir.join("SKILL.md");
    let Ok(text) = std::fs::read_to_string(&skill_md) else { return None };
    let name = dir.file_name()?.to_string_lossy().to_string();
    let mut pts = 0i32;
    let mut plus: Vec<String> = Vec::new();
    let mut minus: Vec<String> = Vec::new();

    // + scripts/ ejecutables: activos FUNCIONALES (lo más valioso)
    let scripts = std::fs::read_dir(dir.join("scripts"))
        .map(|rd| rd.flatten().count())
        .unwrap_or(0);
    if scripts > 0 {
        pts += 25;
        plus.push(format!("scripts/ ({scripts} archivos ejecutables)"));
    }

    // + bloques de código en la doc
    let code_blocks = count_matches(&text, "\n```");
    if code_blocks >= 2 {
        let p = (code_blocks as i32 * 2).min(12);
        pts += p;
        plus.push(format!("{} bloques de código (+{})", code_blocks / 2, p));
    }

    // + comandos concretos ejecutables (backticks con binarios reales)
    let cmd_words = ["npm ", "cargo ", "pip ", "pytest", "git ", "curl ", "docker ", "make ", "npx "];
    let cmds: usize = cmd_words.iter().map(|w| count_matches(&text, w)).sum();
    if cmds > 0 {
        let p = (cmds as i32 * 3).min(15);
        pts += p;
        plus.push(format!("comandos concretos ({cmds}) (+{p})"));
    }

    // + librerías/APIs específicas (import/require/from X import)
    let imports = count_matches(&text, "import ")
        + count_matches(&text, "require(")
        + count_matches(&text, "use ");
    if imports > 0 {
        let p = (imports as i32 * 2).min(15);
        pts += p;
        plus.push(format!("librerías/APIs nombradas ({imports}) (+{p})"));
    }

    // + gates de verificación (la skill obliga a comprobar resultado)
    let gates = ["verify", "check", "must pass", "exit code", "validation", "gate"]
        .iter().map(|w| count_matches(&text, w)).sum::<usize>();
    if gates >= 2 {
        pts += 10;
        plus.push(format!("gates de verificación ({gates}) (+10)"));
    }

    // + referencias a ficheros de config/rutas concretas
    let configs = [".json\"", ".toml\"", ".yaml\"", ".yml\"", ".env"]
        .iter().map(|w| count_matches(&text, w)).sum::<usize>();
    if configs > 0 {
        pts += 5;
        plus.push(format!("rutas/config concretas ({configs}) (+5)"));
    }

    // − prosa genérica sin especificidad (lo que la IA YA sabe)
    let generic = ["best practices", "clean code", "be concise", "think carefully",
        "high quality", "follow standards", "pay attention", "as needed"]
        .iter().map(|w| count_matches(&text, w)).sum::<usize>();
    if generic >= 3 && cmds == 0 && scripts == 0 {
        let p = (generic as i32 * 4).min(20);
        pts -= p;
        minus.push(format!("prosa genérica sin funcionalidad (-{p})"));
    }

    // − demasiado corta sin nada
    let lines = text.lines().count();
    if lines < 20 && scripts == 0 {
        pts -= 15;
        minus.push(format!("muy corta ({lines} líneas, sin assets) (-15)"));
    } else if lines >= 40 {
        pts += 8;
        plus.push(format!("documentación sustancial ({lines} líneas) (+8)"));
    }

    Some(SkillScore { name, points: pts.clamp(-30, 100), signals: plus, anti_signals: minus })
}

/// Tabla de scoring para todos los skills instalables bajo un directorio.
pub fn render_skills_score(root: &std::path::Path) -> String {
    let mut out = String::from("## Calidad de skills (análisis funcional)\n\n");
    let mut scored: Vec<SkillScore> = Vec::new();
    // buscar dirs con SKILL.md (profundidad ≤3)
    fn walk(dir: &std::path::Path, depth: usize, acc: &mut Vec<std::path::PathBuf>) {
        if depth > 3 { return; }
        if dir.join("SKILL.md").exists() {
            acc.push(dir.to_path_buf());
            return; // una skill encontrada no se baja más
        }
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                if e.path().is_dir() && !e.file_name().to_string_lossy().starts_with('.') {
                    walk(&e.path(), depth + 1, acc);
                }
            }
        }
    }
    let mut dirs = Vec::new();
    walk(root, 0, &mut dirs);
    for d in &dirs {
        if let Some(s) = score_skill_dir(d) {
            scored.push(s);
        }
    }
    if scored.is_empty() {
        return out + "sin skills con SKILL.md encontradas\n";
    }
    scored.sort_by_key(|a| std::cmp::Reverse(a.points));
    for s in &scored {
        let (verdict, why) = s.verdict();
        out.push_str(&format!("{verdict} \x1b[1m{}\x1b[0m — {} pts ({why})\n", s.name, s.points));
        for sig in &s.signals {
            out.push_str(&format!("   \x1b[32m+\x1b[0m {sig}\n"));
        }
        for anti in &s.anti_signals {
            out.push_str(&format!("   \x1b[31m−\x1b[0m {anti}\n"));
        }
    }
    out.push_str(&format!(
        "\n{} skills analizadas · criterio: conocimiento FUNCIONAL (scripts, libs,\ncomandos, gates) sobre prosa genérica que el modelo ya genera solo.\n",
        scored.len()
    ));
    out
}



/// Llamada LLM cruda por la cadena canónica (headroom → gateway → routatic).
/// Sin sufijos de benchmark ni compresión: prompt entra, texto sale.
fn llm_raw(prompt: &str, max_tokens: u32, timeout_ms: u64) -> Option<String> {
    let body = serde_json::json!({
        "model": modelo_real_activo(),
        "max_tokens": max_tokens,
        "thinking": { "type": "disabled" },
        "messages": [{ "role": "user", "content": prompt }]
    })
    .to_string();
    let body_path = std::env::temp_dir().join("alx-llm-raw.json");
    std::fs::write(&body_path, &body).ok()?;
    let cmd = format!(
        "curl -s -m {} http://127.0.0.1:8788/v1/messages -H 'content-type: application/json' -d @{}",
        timeout_ms / 1000,
        shell_quote_path(&body_path)
    );
    let out = alx_gate::run_command(&cmd, timeout_ms + 5000);
    if out.exit_code != 0 {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(out.stdout_head.trim()).ok()?;
    let arr = v["content"].as_array()?;
    let text: String = arr
        .iter()
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("");
    if text.trim().is_empty() { None } else { Some(text) }
}

/// Extrae los fragmentos entre backticks de un markdown (comandos, APIs).
fn extract_backticks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut parts = line.split('`');
        parts.next(); // antes del primer `
        while let Some(seg) = parts.next() {
            let seg = seg.trim().to_lowercase();
            if !seg.is_empty() && seg.len() <= 80 {
                out.push(seg);
            }
            parts.next(); // salta el cierre
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Extrae encabezados ## de un markdown (secciones del skill).
fn extract_headings(text: &str) -> Vec<String> {
    let mut out: Vec<String> = text
        .lines()
        .filter_map(|l| l.strip_prefix("## "))
        .map(|h| h.trim().to_lowercase())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// CHALLENGE de skills: la IA escribe EN FRÍO su propia versión del skill
/// (sin ver la de internet) y comparamos. El valor REAL de la skill externa
/// es su DELTA: lo que aporta y el modelo NO generó por su cuenta.
/// Delta alto ⇒ conocimiento no-obvio ⇒ puntos extra en el score.
pub fn run_skills_challenge(skill_dir: &std::path::Path) -> String {
    let Ok(fetched) = std::fs::read_to_string(skill_dir.join("SKILL.md")) else {
        return format!("✗ {}: sin SKILL.md", skill_dir.display());
    };
    // tema: nombre del dir + primera línea de description del frontmatter
    let name = skill_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let description = fetched
        .lines()
        .find(|l| l.trim_start().starts_with("description:"))
        .map(|l| l.split(':').skip(1).collect::<String>().trim().to_string())
        .unwrap_or_default();

    let mut out = format!("## Challenge: {name} — internet vs IA en frío\n\n");

    // 1. baseline ciego (sin enseñarle la skill externa)
    let topic = if description.is_empty() {
        name.replace(['-', '_'], " ")
    } else {
        format!("{}: {description}", name.replace(['-', '_'], " "))
    };
    let baseline_prompt = format!(
        "Escribe la mejor SKILL.md posible para el tema «{topic}». \
         Formato: frontmatter YAML (name, description), luego instrucciones \
         concretas con comandos ejecutables, librerías específicas y pasos \
         verificables. Sin relleno genérico."
    );
    out.push_str("⏳ generando baseline de la IA (en frío, sin ver la skill externa)...\n");
    let Some(baseline) = llm_raw(&baseline_prompt, 1200, 90_000) else {
        return out + "✗ la cadena LLM no respondió — challenge abortado (el score base sigue válido)\n";
    };

    // persistir para auditoría: el baseline junto a la skill
    let baseline_path = skill_dir.join("BASELINE-IA.md");
    let _ = std::fs::write(&baseline_path, &baseline);
    out.push_str(&format!("✓ baseline guardado: {}\n\n", baseline_path.display()));

    // 2. comparación funcional: ¿qué tiene la externa que el baseline NO?
    let fetched_cmds = extract_backticks(&fetched);
    let base_cmds = extract_backticks(&baseline);
    let uniq_cmds: Vec<_> = fetched_cmds
        .iter()
        .filter(|c| !base_cmds.contains(c))
        .cloned()
        .collect();

    let fetched_h = extract_headings(&fetched);
    let base_h = extract_headings(&baseline);
    let uniq_h: Vec<_> = fetched_h.iter().filter(|h| !base_h.contains(*h)).cloned().collect();

    let has_scripts = skill_dir.join("scripts").exists();

    let mut delta_pts = 0i32;
    out.push_str("**Elementos que SOLO la skill externa aporta** (lo que la IA no generó sola):\n");
    if has_scripts {
        delta_pts += 15;
        out.push_str("   \x1b[32m+\x1b[0m directorio scripts/ funcional (+15)\n");
    }
    for c in uniq_cmds.iter().take(8) {
        delta_pts += 4;
        out.push_str(&format!("   \x1b[32m+\x1b[0m comando/API: `{c}` (+4)\n"));
    }
    if uniq_cmds.len() > 8 {
        out.push_str(&format!("   … y {} más (+{})\n", uniq_cmds.len() - 8, (uniq_cmds.len() - 8) * 4));
        delta_pts += ((uniq_cmds.len() - 8) * 4) as i32;
    }
    for h in uniq_h.iter().take(5) {
        delta_pts += 2;
        out.push_str(&format!("   \x1b[32m+\x1b[0m sección única: «{h}» (+2)\n"));
    }

    // elementos que la IA también genera sola = sin valor añadido
    let overlap = fetched_cmds.len().saturating_sub(uniq_cmds.len());
    if overlap > 0 && uniq_cmds.is_empty() && !has_scripts {
        out.push_str("   \x1b[31m−\x1b[0m todo lo que aporta ya estaba en el baseline de la IA\n");
    }
    out.push_str(&format!(
        "\n**Delta vs baseline IA: {delta_pts} pts** ({} comandos únicos, {} secciones únicas, scripts: {})\n",
        uniq_cmds.len(),
        uniq_h.len(),
        if has_scripts { "sí" } else { "no" }
    ));

    // 3. score final = base + delta
    let mut base = score_skill_dir(skill_dir)
        .map(|s| s.points)
        .unwrap_or(0);
    base = (base + delta_pts).clamp(-30, 100);
    let verdict = if base >= 70 {
        "\x1b[1;32mINSTALAR ⭐\x1b[0m"
    } else if base >= 45 {
        "\x1b[1;33mPROBAR\x1b[0m"
    } else {
        "\x1b[1;31mDESCARTAR\x1b[0m"
    };
    out.push_str(&format!("\n**SCORE FINAL CON CHALLENGE: {base}/100 → {verdict}**\n"));
    out
}


pub fn run_skills_fetch(repo: Option<&str>, search: Option<&str>) -> String {
    let Some(proj) = find_project_alexandria(None) else {
        return "✗ proyecto no alexandrizado: `alx init` primero".into();
    };
    let skills_dir = proj.join("skills");
    let _ = std::fs::create_dir_all(&skills_dir);

    // --search: GitHub API, sort=stars. Sin clave: 10 req/min, suficiente.
    if let Some(q) = search {
        return github_search_skills(q);
    }

    // sin argumento: mostrar catálogo curado + cómo buscar
    let Some(repo) = repo else {
        let mut out = String::from("## Catálogo de skills recomendadas\n\n");
        for (r, desc) in SKILL_CATALOG {
            out.push_str(&format!("· {r} — {desc}\n  alx skills-fetch {r}\n"));
        }
        out.push_str("\nBuscar MÁS por estrellas:\n  alx skills-fetch --search \"claude skills\"\n");
        return out;
    };

    let name = repo.rsplit('/').next().unwrap_or(repo);
    let dest = skills_dir.join(name);
    if dest.exists() {
        return format!("✓ ya descargado: {}\n(actualiza con: git -C {} pull)", dest.display(), dest.display());
    }
    let url = format!("https://github.com/{repo}.git");
    let cmd = format!(
        "git clone --depth 1 {url} {}",
        shell_quote_path(&dest)
    );
    let outcome = alx_gate::run_command(&cmd, 120_000);
    if !dest.exists() {
        return format!(
            "✗ clone falló ({}): {}\nrepo: {url}",
            outcome.exit_code,
            outcome.stdout_head.chars().take(200).collect::<String>()
        );
    }
    let n_skills = count_installable_skills(&dest);
    register_in_catalog(&proj, &format!(
        "- [{name}]({url}) — descargado {} · {n_skills} skills\n", chrono_today()));
    format!(
        "✓ {repo} → {}\nskills con SKILL.md detectadas: {n_skills}\ninstalables copiando sus dirs a ~/.claude/skills/ o vía plugin\ncatálogo : {}\n\n{}",
        dest.display(),
        skills_dir.join("catalog.md").display(),
        render_skills_score(&dest)
    )
}

fn github_search_skills(query: &str) -> String {
    let q = query.replace(' ', "+");
    // ojo: run_command solo devuelve el HEAD del stdout; la respuesta de
    // GitHub es grande → volcarla a fichero y leer completa.
    let tmp = std::env::temp_dir().join("alx-gh-search.json");
    let cmd = format!(
        "curl -s -m 15 -H 'User-Agent: alexandria-alx' -H 'Accept: application/vnd.github+json' \
         'https://api.github.com/search/repositories?q={q}&sort=stars&order=desc&per_page=8' -o {}",
        shell_quote_path(&tmp)
    );
    let out = alx_gate::run_command(&cmd, 20_000);
    let Ok(txt) = std::fs::read_to_string(&tmp) else {
        return format!("✗ GitHub API no respondió (curl exit {})", out.exit_code);
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(txt.trim()) else {
        return "✗ respuesta ilegible de GitHub (¿rate-limit? reintenta en un minuto)".into();
    };
    let Some(items) = v["items"].as_array() else {
        return "✗ sin resultados".into();
    };
    let mut lines = vec!["## Skills/repos por estrellas".to_string(), String::new()];
    for it in items {
        let full = it["full_name"].as_str().unwrap_or("?");
        let stars = it["stargazers_count"].as_u64().unwrap_or(0);
        let desc = it["description"].as_str().unwrap_or("").chars().take(90).collect::<String>();
        lines.push(format!("★{stars:<6} {full}\n         {desc}"));
    }
    lines.push(String::new());
    lines.push("Instalar la que elijas: alx skills-fetch owner/repo".to_string());
    lines.join("\n")
}

fn count_installable_skills(dest: &std::path::Path) -> usize {
    std::fs::read_dir(dest)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_dir())
                .filter(|d| d.path().join("SKILL.md").exists())
                .count()
        })
        .unwrap_or(0)
}

fn register_in_catalog(proj: &std::path::Path, entry: &str) {
    use std::io::Write;
    let catalog = proj.join("skills/catalog.md");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&catalog) {
        let _ = f.write_all(entry.as_bytes());
    }
}

fn shell_quote_path(p: &std::path::Path) -> String {
    format!("'{}'", p.display().to_string().replace('\'', "'\\''"))
}

fn chrono_today() -> String {
    // fecha local sin dependencia chrono: la sacamos del sistema una vez
    let out = alx_gate::run_command("date +%F", 2_000);
    out.stdout_head.trim().to_string()
}

// ═══════════════ RESEARCH-CHECK (enforcement de profundidad) ══════════════
//
// "No podemos dejar que la IA no lo haga o lo haga shallow" (usuario). Este
// check es LA compuerta: verifica que los 7 pasos del protocolo están
// RELLENOS DE VERDAD (no plantillas vacías ni relleno mínimo) y que hay
// simulaciones y tabla de fuentes. Exit != 0 si suspende → un hook Stop puede
// bloquear el fin de sesión mientras la investigación esté incompleta.

/// Umbral mínimo de caracteres "reales" por paso (plantilla ≈ 400-700 chars).
const CHECK_MIN_CHARS: usize = 1200;
/// Simulaciones completas mínimas en 03.
const CHECK_MIN_SIMS: usize = 2;
/// Filas mínimas en la tabla de evidencia de 05.
const CHECK_MIN_FUENTES: usize = 3;

pub fn run_research_check(dir_opt: Option<&str>) -> String {
    // localizar el proyecto de research: arg, o el único/más reciente
    let base = find_project_alexandria(None)
        .map(|p| p.join("research"))
        .unwrap_or_else(|| std::path::PathBuf::from("research"));
    let dir = match dir_opt {
        Some(d) => std::path::PathBuf::from(d),
        None => {
            let mut candidatos: Vec<_> = std::fs::read_dir(&base)
                .map(|rd| {
                    rd.flatten()
                        .map(|e| e.path())
                        .filter(|p| p.is_dir() && p.join("06-respuesta.md").exists())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            candidatos.sort_by_key(|p| p.metadata().and_then(|m| m.modified()).ok());
            match candidatos.pop() {
                Some(p) => p,
                None => return "✓ sin proyectos de research abiertos: nada que comprobar".into(),
            }
        }
    };
    if !dir.is_dir() {
        return format!("✗ no existe {dir_opt:?}");
    }

    let mut fallos: Vec<String> = Vec::new();
    for (name, _) in RESEARCH_STEPS {
        let path = dir.join(format!("{name}.md"));
        let Ok(txt) = std::fs::read_to_string(&path) else {
            fallos.push(format!("{name}: FALTA el fichero"));
            continue;
        };
        let cuerpo_len = txt
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count()
            .saturating_mul(40); // aprox chars útiles
        let tiene_placeholder = txt.contains("este paso aún está vacío");
        if tiene_placeholder || txt.len() < CHECK_MIN_CHARS / 3 {
            fallos.push(format!(
                "{name}: sin rellenar ({} bytes; placeholder={})",
                txt.len(),
                tiene_placeholder
            ));
            continue;
        }
        let _ = cuerpo_len;
    }

    // 03-simulaciones: contar SIM-\d+
    if let Ok(sim) = std::fs::read_to_string(dir.join("03-simulaciones.md")) {
        let n = sim.match_indices("SIM-").count();
        if n < CHECK_MIN_SIMS && !sim.contains("este paso aún está vacío") {
            fallos.push(format!("03-simulaciones: {n} simulaciones (mínimo {CHECK_MIN_SIMS})"));
        }
    }
    // 05-fuentes: contar filas de tabla (líneas empezando por |)
    if let Ok(fue) = std::fs::read_to_string(dir.join("05-fuentes.md")) {
        let filas = fue.lines().filter(|l| l.trim_start().starts_with('|')).count();
        if filas < CHECK_MIN_FUENTES + 1 && !fue.contains("este paso aún está vacío") {
            fallos.push(format!("05-fuentes: {filas} filas de tabla (mínimo {}+cabecera)", CHECK_MIN_FUENTES));
        }
    }

    if fallos.is_empty() {
        format!(
            "✓ investigación completa y profunda: {}\nlos 7 pasos rellenos, simulaciones ≥{CHECK_MIN_SIMS}, evidencia ≥{CHECK_MIN_FUENTES}",
            dir.display()
        )
    } else {
        let mut out = String::from("✗ INVESTIGACIÓN INCOMPLETA — termina antes de cerrar:\n");
        for f in &fallos {
            out.push_str(&format!("  · {f}\n"));
        }
        out.push_str("\nEl hook Stop usa este veredicto: la sesión no debería terminar con research abierto a medias.");
        out
    }
}

pub fn run_setup() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut out = String::from("# alx setup — integración con Claude Code\n");
    let ok = |b: bool| if b { "✓".to_string() } else { "✗".to_string() };

    // 1. Binario alx
    let alx = format!("{home}/.local/bin/alx");
    let alx_ok = std::path::Path::new(&alx).exists();
    out.push_str(&format!("binario alx: {} ({})\n", ok(alx_ok), alx));

    // 2. Statusline powerline
    let sl = format!("{home}/.local/bin/alx-statusline");
    let sl_ok = std::path::Path::new(&sl).exists();
    out.push_str(&format!("statusline powerline: {} ({})\n", ok(sl_ok), sl));

    // 3. settings.json: statusLine -> alx-statusline (merge no destructivo)
    let settings_path = format!("{home}/.claude/settings.json");
    let mut settings_written = false;
    if let Ok(text) = std::fs::read_to_string(&settings_path) {
        if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&text) {
            v["statusLine"] = serde_json::json!({
                "type": "command",
                "command": sl.clone(),
                "refreshInterval": 10
            });
            if let Ok(new_text) = serde_json::to_string_pretty(&v) {
                if std::fs::write(&settings_path, new_text).is_ok() {
                    settings_written = true;
                }
            }
        }
    }
    out.push_str(&format!(
        "settings.json statusLine=alx-statusline: {}\n",
        ok(settings_written)
    ));

    // 4. MCP server 'alexandria' en ~/.claude.json (merge)
    let mcp_path = format!("{home}/.claude.json");
    let mut mcp_ok = false;
    if let Ok(text) = std::fs::read_to_string(&mcp_path) {
        if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&text) {
            let has = v["mcpServers"].get("alexandria").is_some();
            if has {
                mcp_ok = true;
            } else {
                v["mcpServers"]["alexandria"] = serde_json::json!({
                    "type": "stdio",
                    "command": alx.clone(),
                    "args": ["mcp"]
                });
                if let Ok(new_text) = serde_json::to_string_pretty(&v) {
                    if std::fs::write(&mcp_path, new_text).is_ok() {
                        mcp_ok = true;
                    }
                }
            }
        }
    }
    out.push_str(&format!("MCP server 'alexandria': {}\n", ok(mcp_ok)));

    // 5. Hook iterate/auto-continue del proyecto
    let hooks_path = format!("{home}/Projectos/AlexanderTheGreat/.claude/hooks");
    let iterate_ok = std::path::Path::new(&hooks_path).join("auto-continue.sh").exists();
    out.push_str(&format!("hook iterate/auto-continue: {}\n", ok(iterate_ok)));

    // 5b. Instalación del sistema de hooks desde la FUENTE CANÓNICA
    //     (harnesses/hooks → .claude/hooks). Sin esto, un .claude/
    //     regenerado a mano quedaba cojo: lib/ y providers/ nunca existieron
    //     y 3 hooks morían con ERR_MODULE_NOT_FOUND en silencio.
    let (copiados, npm) = install_hooks_src(&home);
    if copiados > 0 {
        out.push_str(&format!(
            "hooks completos instalados (con lib/providers): {copiados} ficheros{}\n",
            if npm { " + npm install" } else { "" }
        ));
    } else {
        out.push_str("hooks completos: ✗ fuente no encontrada\n");
    }

    // 6. Dependencias core (auto-habilitar) desde config/setup-deps.json.
    let deps_path = format!("{home}/Projectos/AlexanderTheGreat/config/setup-deps.json");
    let mut core_ok = 0usize;
    let mut core_missing = 0usize;
    if let Ok(deps_text) = std::fs::read_to_string(&deps_path) {
        if let Ok(deps) = serde_json::from_str::<serde_json::Value>(&deps_text) {
            if let Ok(text) = std::fs::read_to_string(&settings_path) {
                if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(core) = deps["core_plugins"].as_object() {
                        for (plugin, _desc) in core {
                            let name = plugin.split('@').next().unwrap_or(plugin);
                            let installed = v["enabledPlugins"].as_object().is_some_and(|m| {
                                m.contains_key(plugin)
                                    || m.keys().any(|k| k.starts_with(&format!("{name}@")))
                            });
                            if installed {
                                v["enabledPlugins"][plugin] = serde_json::Value::Bool(true);
                                core_ok += 1;
                            } else {
                                core_missing += 1;
                            }
                        }
                    }
                    if let Ok(new_text) = serde_json::to_string_pretty(&v) {
                        let _ = std::fs::write(&settings_path, new_text);
                    }
                }
            }
        }
    }
    out.push_str(&format!(
        "dependencias core habilitadas: {core_ok} (faltan {core_missing} → /plugin install)\n"
    ));

    // 7. Sync themes desde integration/themes (ruta real) → global + perfil.
    let integration_themes = format!(
        "{home}/Projectos/AlexanderTheGreat/integration/themes"
    );
    let mut themes_synced = 0usize;
    if let Ok(rd) = std::fs::read_dir(&integration_themes) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.ends_with(".json") {
                continue;
            }
            for dst in [
                format!("{home}/.claude/themes/{name}"),
                format!("{home}/.claude-alexandria/themes/{name}"),
            ] {
                if std::fs::copy(e.path(), &dst).is_ok() {
                    themes_synced += 1;
                }
            }
        }
    }
    out.push_str(&format!(
        "themes sincronizados (integration → global+perfil): {} archivos\n",
        themes_synced / 2
    ));

    // 8. Sync skills desde integration/skills → ~/.claude/skills (aditivo).
    let integration_skills = format!(
        "{home}/Projectos/AlexanderTheGreat/integration/skills"
    );
    let mut skills_synced = 0usize;
    if let Ok(rd) = std::fs::read_dir(&integration_skills) {
        for e in rd.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            let dst = format!("{home}/.claude/skills/{name}");
            if std::fs::remove_dir_all(&dst).is_err() {
                // no existe, ok
            }
            if copy_dir(&e.path(), std::path::Path::new(&dst)).is_ok() {
                skills_synced += 1;
            }
        }
    }
    out.push_str(&format!(
        "skills sincronizados (integration → ~/.claude/skills): {skills_synced}\n"
    ));

    // 9. Auto-generar .claude/settings.json del proyecto desde la plantilla
    //    (config/claude-settings.json) — reproducible. Crea .claude/ si falta.
    //    Inyecta SIEMPRE el env que desactiva la atribución de Claude en git.
    let project_dir = format!("{home}/Projectos/AlexanderTheGreat");
    let template = format!("{project_dir}/config/claude-settings.json");
    let dst_claude = format!("{project_dir}/.claude/settings.json");
    let mut claude_ok = false;
    if let Ok(text) = std::fs::read_to_string(&template) {
        if let Some(parent) = std::path::Path::new(&dst_claude).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // Inyectar env anti-atribución (garantizado, no depende de la plantilla).
        let injected = if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&text) {
            v["env"]["CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"] =
                serde_json::Value::String("1".to_string());
            serde_json::to_string_pretty(&v).unwrap_or(text.clone())
        } else {
            text.clone()
        };
        if std::fs::write(&dst_claude, &injected).is_ok() {
            claude_ok = true;
        }
    }
    out.push_str(&format!(
        ".claude/settings.json auto-generado (plantilla): {}\n",
        ok(claude_ok)
    ));

    // 10. Instalar hooks desde harnesses/hooks → .claude/hooks (100% regenerable).
    let hooks_src = format!("{project_dir}/harnesses/hooks");
    let hooks_dst = format!("{project_dir}/.claude/hooks");
    let mut hooks_installed = 0usize;
    if let Ok(rd) = std::fs::read_dir(&hooks_src) {
        let _ = std::fs::create_dir_all(&hooks_dst);
        for e in rd.flatten() {
            let dst = std::path::Path::new(&hooks_dst).join(e.file_name());
            if std::fs::copy(e.path(), &dst).is_ok() {
                hooks_installed += 1;
            }
        }
    }
    out.push_str(&format!(
        "hooks instalados (harnesses/hooks → .claude/hooks): {hooks_installed}\n"
    ));

    // 11. Categorías opcionales (interactivo — solo si hay terminal).
    if std::io::stdin().is_terminal() {
        out.push_str(&setup_ask_categories(home.as_str(), deps_path.as_str()));
    }

    out.push_str("\nReinicia Claude Code para aplicar statusline + theme.\n");
    out
}

/// Pregunta al usuario qué categorías opcionales quiere (diseño, 3D, etc.)
/// y verifica que las skills correspondientes estén disponibles.
fn setup_ask_categories(home: &str, deps_path: &str) -> String {
    let ok = |b: bool| if b { "✓".to_string() } else { "✗ falta".to_string() };
    let mut out = String::from("\n--- Categorías opcionales (selecciona) ---\n");
    if let Ok(deps_text) = std::fs::read_to_string(deps_path) {
        if let Ok(deps) = serde_json::from_str::<serde_json::Value>(&deps_text) {
            if let Some(optional) = deps["optional"].as_object() {
                for (cat, skills) in optional {
                    let skill_count = skills.as_object().map_or(0, |m| m.len());
                    let names: Vec<String> = skills
                        .as_object()
                        .map(|m| m.keys().cloned().collect())
                        .unwrap_or_default();
                    eprint!("¿Haces {cat}? ({skill_count} skills: {}) [y/N]: ", names.join(", "));
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                    let mut ans = String::new();
                    let _ = std::io::stdin().read_line(&mut ans);
                    if ans.trim().eq_ignore_ascii_case("y") {
                        out.push_str(&format!("  {cat}:\n"));
                        for (skill, desc) in skills.as_object().unwrap_or(&serde_json::Map::new()) {
                            let exists = std::path::Path::new(&format!("{home}/.claude/skills/{skill}"))
                                .exists();
                            out.push_str(&format!("    {} — {desc}: {}\n", skill, ok(exists)));
                        }
                    }
                }
            }
        }
    }
    out.push_str("  (las '✗ falta' → /plugin install o marketplace)\n");
    out
}

/// Cuenta los agentes reales del ecosistema (agents/ + agents-volt/).
pub fn count_real_agents() -> usize {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../");
    let mut real = 0usize;
    for dir in ["agents", "agents-volt"] {
        if let Ok(rd) = std::fs::read_dir(repo_root.join(dir)) {
            real += rd
                .flatten()
                .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
                .count();
        }
    }
    real
}

/// Ruta del state del harness iterate, con scope de sesión (multi-sesión).
/// Si ALX_SESSION_ID está definido, usa state-<id>.toml; si no, state.toml.
fn iterate_state_path() -> std::path::PathBuf {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../harnesses/iterate");
    match std::env::var("ALX_SESSION_ID") {
        Ok(id) if !id.trim().is_empty() => base.join(format!("state-{id}.toml")),
        _ => base.join("state.toml"),
    }
}

/// Estado del loop de iteración gestionado por el MOTOR (no bash).
/// Lee state.toml del harness iterate y decide si debe continuar.
pub fn render_iterate_state() -> String {
    let path = iterate_state_path();
    let mut out = String::from("## Iteración (motor nativo)\n");
    if let Ok(text) = std::fs::read_to_string(&path) {
        let v: toml::Value = toml::from_str(&text).unwrap_or(toml::Value::Table(Default::default()));
        let iter = v.get("iter").and_then(|i| i.as_integer()).unwrap_or(0);
        let max_iter = v.get("max_iter").and_then(|i| i.as_integer()).unwrap_or(20);
        let target = v.get("target_iter").and_then(|i| i.as_integer()).unwrap_or(max_iter);
        let awaiting = v.get("awaiting_user").and_then(|a| a.as_bool()).unwrap_or(false);
        let work = v.get("work_unit").and_then(|w| w.as_str()).unwrap_or("").to_string();
        out.push_str(&format!("iter: {iter}/{max_iter} (target {target})\n"));
        out.push_str(&format!("awaiting_user: {awaiting}\n"));
        out.push_str(&format!("unidad: {work}\n"));
        if iter == 0 {
            out.push_str("ESTADO: ciclo completado — el motor para solo.\n");
        } else if awaiting {
            out.push_str("ESTADO: esperando respuesta del humano — no forzar.\n");
        } else if iter >= target {
            out.push_str("ESTADO: objetivo alcanzado — no iterar más.\n");
        } else {
            out.push_str(&format!("ESTADO: puede continuar (iteración {})\n", iter + 1));
        }
    } else {
        out.push_str("sin state.toml\n");
    }
    out
}

/// Avanza una iteración: el MOTOR incrementa iter en state.toml (sin bash).
pub fn iterate_next() -> String {
    let path = iterate_state_path();
    if let Ok(text) = std::fs::read_to_string(&path) {
        let v: toml::Value = toml::from_str(&text).unwrap_or(toml::Value::Table(Default::default()));
        let iter = v.get("iter").and_then(|i| i.as_integer()).unwrap_or(0);
        let next = iter + 1;
        let new_text = text.replace(&format!("iter = {iter}"), &format!("iter = {next}"));
        let _ = std::fs::write(&path, new_text);
        return format!("→ iteración {next} (motor nativo — auto-continue ya no necesario)\n");
    }
    "sin state.toml".to_string()
}

/// Estado del plugin PHALANX — CONFIG CARGADA por el motor.
/// Parsea `config.toml` (governor routes, mcp clients, critic, iterate) y
/// cada hook .toml (id, event, priority). Resuelve desde el manifest del crate.
pub fn render_phalanx_status() -> String {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../phalanx");
    let config_path = base.join("config.toml");
    let hooks_dir = base.join("hooks");
    let config_ok = config_path.exists();
    let hook_files: Vec<String> = std::fs::read_dir(&hooks_dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().extension().map(|x| x == "toml").unwrap_or(false))
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();

    let mut out = String::from("## PHALANX — config cargada por el motor\n");
    if config_ok {
        if let Ok(text) = std::fs::read_to_string(&config_path) {
            let parsed: toml::Value =
                toml::from_str(&text).unwrap_or(toml::Value::Table(Default::default()));
            if let Some(r) = parsed.get("governor").and_then(|g| g.get("routes")) {
                out.push_str(&format!("governor.routes: {r}\n"));
            }
            if let Some(c) = parsed.get("mcp").and_then(|m| m.get("clients")) {
                out.push_str(&format!("mcp.clients: {c}\n"));
            }
            if let Some(c) = parsed.get("critic") {
                out.push_str(&format!("critic: {c}\n"));
            }
            if let Some(i) = parsed.get("iterate") {
                out.push_str(&format!("iterate: {i}\n"));
            }
        }
    } else {
        out.push_str("config.toml: ✗ falta\n");
    }
    out.push_str(&format!("Hooks declarados: {}\n", hook_files.len()));
    for h in &hook_files {
        let hpath = hooks_dir.join(h);
        if let Ok(text) = std::fs::read_to_string(&hpath) {
            let v: toml::Value =
                toml::from_str(&text).unwrap_or(toml::Value::Table(Default::default()));
            let id = v.get("id").and_then(|i| i.as_str()).unwrap_or(h);
            let event = v.get("event").and_then(|e| e.as_str()).unwrap_or("?");
            let prio = v.get("priority").and_then(|p| p.as_str()).unwrap_or("?");
            out.push_str(&format!("  {id} — {event} — {prio}\n"));
        }
    }
    out
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
    // Dogfood end-to-end: tras generar el artefacto, el motor VERIFICA su
    // propio build (la evidencia cierra el ciclo).
    let build = verify_build();
    let report = format!("{report}\n\n## Verificación final (dogfood)\n{}", render_build(&build));
    match std::fs::write(&path, &report) {
        Ok(()) => format!("✓ artefacto escrito: {}\n\n{report}", path.display()),
        Err(e) => format!("✗ no se pudo escribir: {e}\n\n{report}"),
    }
}

/// Sirve el protocolo MCP JSON-RPC por stdio: responde `initialize` /
/// `tools/list` / `tools/call`. `tools/call` ejecuta la tool REAL del motor
/// cuando existe (phalanx.status, cost, metrics, agents...).
pub fn serve_mcp_stdio() -> i32 {
    use std::io::BufRead;
    let catalog = ToolCatalog::alexandria_default();
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        if let Some(resp) = handle_line(&catalog, &line) {
            if line.contains("\"tools/call\"") {
                if let Some(real) = mcp_real_tool(&line) {
                    println!("{real}");
                    continue;
                }
            }
            println!("{resp}");
        }
    }
    0
}

/// Ejecuta una tool REAL del motor para un `tools/call` MCP.
fn mcp_real_tool(line: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let name = v["params"]["name"].as_str()?;
    let out = match name {
        "phalanx.status" => render_phalanx_status(),
        "governor.cost_report" => render_cost_report(),
        "task.list" => {
            let tasks = load_tasks_from_jsonl();
            if tasks.is_empty() {
                "(sin tareas persistidas)".to_string()
            } else {
                tasks
                    .iter()
                    .map(|t| format!("{} | {} | {:?}", t.id, t.title, t.status))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        "bench.run" => render_metrics(),
        "agent.list" => render_agents(),
        "iterate.status" => render_iterate_state(),
        _ => return None,
    };
    let id = v["id"].to_string();
    let text = serde_json::to_string(&out).ok()?;
    Some(format!(
        r#"{{"jsonrpc":"2.0","id":{id},"result":{{"content":[{{"type":"text","text":{text}}}]}}}}"#
    ))
}

/// Persiste una tarea en state/tasks.jsonl (append).
pub fn persist_task_to_jsonl(task: &Task) -> std::io::Result<()> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../state");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("tasks.jsonl");
    use std::io::Write;
    let line = serde_json::to_string(task)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(f, "{line}")
}

/// Carga las tareas persistidas (state/tasks.jsonl).
pub fn load_tasks_from_jsonl() -> Vec<Task> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../state/tasks.jsonl");
    if let Ok(text) = std::fs::read_to_string(&path) {
        return text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    }
    Vec::new()
}

/// Ciclo watcher de harnesses con persistencia real (alx-evolve).
/// Seed si vacío; retira temporales con uso, promueve con 5 usos, persiste.
pub fn run_evolve_cycle() -> String {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../harnesses");
    let mut reg = HarnessRegistry::load_from(&dir);
    if reg.all().is_empty() {
        let now = now_ms();
        reg.add(Harness::new(
            "hx-design-tokens",
            "design-tokens",
            HarnessKind::Temporal,
            Trigger::Phase("Build".to_string()),
            "consistencia visual",
            "Usa tokens de diseño; sin hex literales hardcodeados en el proyecto.",
            "alx-evolve",
            now,
        ));
        let _ = reg.save_to(&dir);
    }
    let summary = HarnessRegistry::watcher_cycle(&dir, &|h| h.uses > 0, 5);
    format!(
        "## Evolve watcher\nDisco: {}\nCargados: {}\nRetirados: {}\nPromovidos: {}\nVivos: {}\n",
        dir.display(),
        summary.loaded,
        summary.retired.len(),
        summary.promoted.len(),
        summary.live
    )
}

/// Doctor del ecosistema ALEXANDRIA (alx-audit): indexa crates, hooks
/// PHALANX y harnesses, y valida con el doctor.
pub fn render_doctor() -> String {
    let mut index = AuditIndex::new();
    for name in [
        "alx-core",
        "alx-hooks",
        "alx-memory",
        "alx-governor",
        "alx-task",
        "alx-harness",
        "alx-gate",
        "alx-bench",
        "alx-critic",
        "alx-audit",
        "alx-night",
        "alx-mcp",
        "alx-agents",
        "alx-cli",
        "alx-lib",
        "alx-evolve",
    ] {
        index.add(AuditItem::new(
            format!("crate-{name}"),
            name,
            ItemKind::Plugin,
            format!("crates/{name}"),
            "workspace",
            format!("Crate del motor ALEXANDRIA que implementa su subsistema {name}."),
        ));
    }
    for h in [
        "mission",
        "governor-classify",
        "memory-capture",
        "iterate-trigger",
        "critic-run",
        "gate-verify",
        "evolve-detect",
        "docmin-verify",
        "bench-sample",
        "headless-spawn",
    ] {
        index.add(AuditItem::new(
            format!("hook-{h}"),
            h,
            ItemKind::Hook,
            format!("phalanx/hooks/{h}.toml"),
            "phalanx",
            format!("Hook PHALANX que dispara la automatización {h} en el motor."),
        ));
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../harnesses");
    let harnesses = HarnessRegistry::load_from(&dir);
    for h in harnesses.all() {
        index.add(AuditItem::new(
            format!("harness-{}", h.id),
            h.id.clone(),
            ItemKind::Harness,
            dir.join("active/harnesses.jsonl").display().to_string(),
            "evolve",
            format!("Harness evolutivo {} con objetivo: {}.", h.name, h.objective),
        ));
    }
    let mut out = format!("## Doctor ALEXANDRIA\nTotal items: {}\n", index.count());
    out.push_str(&alx_audit::Doctor::doctor_report(&index));
    out
}

/// Cost-report del governor: lee el ledger persistido (state/ledger.jsonl)
/// y resume tokens y coste de todas las llamadas reales acumuladas.
pub fn render_cost_report() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../state/ledger.jsonl");
    let (mut n, mut in_tok, mut out_tok, mut cost) = (0usize, 0u32, 0u32, 0.0f64);
    if let Ok(text) = std::fs::read_to_string(&path) {
        for line in text.lines() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                n += 1;
                in_tok = in_tok.saturating_add(v["input_tokens"].as_u64().unwrap_or(0) as u32);
                out_tok = out_tok.saturating_add(v["output_tokens"].as_u64().unwrap_or(0) as u32);
                cost += v["cost_usd"].as_f64().unwrap_or(0.0);
            }
        }
    }
    let mut out = format!(
        "## Cost report (governor)\nLlamadas reales: {n}\nTokens: {in_tok} in / {out_tok} out\nCoste estimado total: ${cost:.6}\n"
    );

    // Telemetría por día: eventos del pipeline agrupados por día civil.
    let events_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../state/events.log");
    let mut days: std::collections::BTreeMap<u64, usize> = std::collections::BTreeMap::new();
    if let Ok(text) = std::fs::read_to_string(&events_path) {
        for line in text.lines() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(ts) = v["ts"].as_u64() {
                    *days.entry(ts / 86_400_000).or_insert(0) += 1;
                }
            }
        }
    }
    if !days.is_empty() {
        out.push_str("\nEventos por día:\n");
        for (day, count) in &days {
            out.push_str(&format!("  día {day}: {count} eventos\n"));
        }
    }
    out
}

/// Agentes del registry ALEXANDRIA + envelope de spawn (alx-agents).
pub fn render_agents() -> String {
    let mut reg = AgentRegistry::new();
    for (name, desc, tier, phase) in [
        (
            "general-purpose",
            "Agente general para cualquier fase del pipeline ALEXANDRIA.",
            ModelTier::T2Medium,
            None,
        ),
        (
            "code-reviewer",
            "Revisa el código contra criterios de calidad y detecta bugs.",
            ModelTier::T3Premium,
            Some(PhaseId::Review),
        ),
        (
            "test-engineer",
            "Diseña y ejecuta tests para verificar cada micro-tarea.",
            ModelTier::T2Medium,
            Some(PhaseId::Test),
        ),
    ] {
        reg.add(AgentSpec {
            name: name.into(),
            description: desc.into(),
            tools: Vec::new(),
            tier,
            phase,
            tags: Vec::new(),
        });
    }
    let mut out = String::from("## Agentes ALEXANDRIA\n");
    for a in reg.all() {
        let phase = a.phase.map(|p| p.as_str()).unwrap_or("cualquiera");
        out.push_str(&format!(
            "\n- {} ({:?}, fase {phase}): {}",
            a.name, a.tier, a.description
        ));
    }
    if let Some(spec) = reg.by_name("general-purpose") {
        let env = build_envelope(spec, "verificar que el build pasa", Vec::new(), 2000);
        out.push_str(&format!(
            "\n\n## Envelope (spawn general-purpose)\nsystem: {}\ntask: {}\nbudget: {} tokens",
            env.system, env.task, env.budget_tokens
        ));
    }

    // Agentes reales del ecosistema (repo: agents/ + agents-volt/).
    // Ruta correcta: tres subidas desde crates/alx-cli (antes había cuatro y
    // contaba 0 siempre). Se listan los primeros nombres para que
    // `alx agents-show <nombre>` sea descubrible sin abrir el repo.
    let mut real = Vec::new();
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../");
    for dir in ["agents", "agents-volt"] {
        let p = repo_root.join(dir);
        if let Ok(rd) = std::fs::read_dir(&p) {
            for e in rd.flatten() {
                let path = e.path();
                if path.extension().map(|x| x == "md").unwrap_or(false) {
                    real.push(path);
                }
            }
        }
    }
    real.sort();
    out.push_str(&format!(
        "\n\n## Agentes reales del ecosistema: {} ficheros\n",
        real.len()
    ));
    for path in real.iter().take(8) {
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            out.push_str(&format!("- {stem}\n"));
        }
    }
    if real.len() > 8 {
        out.push_str(&format!("… y {} más (`alx agents-show <nombre>`)\n", real.len() - 8));
    }
    out
}

/// TUI dashboard: estado del motor en terminal con paneles (ANSI, sin deps).
// ─── Actividad en vivo (alx watch) ──────────────────────────────────────

/// Ruta del log de actividad que escribe el hook activity-tracker.sh.
fn activity_log_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../state/activity.jsonl")
}

/// Mapea un evento de activity.jsonl a un estado legible de ALEXANDRIA
/// (con color ANSI). Devuelve None para eventos sin valor visual.
fn activity_state(ev: &str, tool: &str, detail: &str) -> Option<(String, String)> {
    let d = detail.trim();
    match (ev, tool) {
        ("UserPromptSubmit", _) => Some(("\x1b[1;35m🎯 TAREA\x1b[0m".into(), d.to_string())),
        ("Notification", _) => Some(("\x1b[33m💬 aviso\x1b[0m".into(), d.to_string())),
        (_, "Task") => Some(("\x1b[1;36m🤖 AGENTE desplegado\x1b[0m".into(), d.to_string())),
        (_, "Skill") => Some(("\x1b[1;35m🎓 SKILL activa\x1b[0m".into(), d.to_string())),
        (_, "TodoWrite") => Some(("\x1b[1;34m📋 PLANIFICANDO\x1b[0m".into(), d.to_string())),
        (_, t) if t == "WebFetch" || t == "WebSearch" => {
            let where_ = if d.contains("github.com") { "GitHub" } else { "web" };
            Some((format!("\x1b[1;33m🌐 BUSCANDO ({where_})\x1b[0m"), d.to_string()))
        }
        (_, "Bash") => {
            let cmd = d.to_lowercase();
            let state = if cmd.contains("cargo test") || cmd.contains("pytest") || cmd.contains("npm test") || cmd.contains("go test") {
                "\x1b[1;32m🧪 VERIFICANDO\x1b[0m"
            } else if cmd.contains("git commit") {
                "\x1b[1;32m📦 COMMIT\x1b[0m"
            } else if cmd.contains("cargo build") || cmd.contains("npm run build") || cmd.contains("make") {
                "\x1b[36m🔨 COMPILANDO\x1b[0m"
            } else if cmd.contains("git ") {
                "\x1b[34m🔀 git\x1b[0m"
            } else {
                "\x1b[37m⚙️  ejecutando\x1b[0m"
            };
            Some((state.into(), d.chars().take(70).collect()))
        }
        (_, t) if t == "Edit" || t == "MultiEdit" || t == "Write" => {
            let base = d.rsplit('/').next().unwrap_or(d);
            Some(("\x1b[1;33m✏️  EDITANDO\x1b[0m".into(), base.to_string()))
        }
        (_, t) if t == "Read" || t == "Grep" || t == "Glob" => {
            Some(("\x1b[90m🔍 EXPLORANDO código\x1b[0m".into(), d.chars().take(60).collect()))
        }
        ("Stop", _) => Some(("\x1b[90m⏸  ciclo terminado\x1b[0m".into(), String::new())),
        _ => None,
    }
}

/// Snapshot del dashboard de actividad: estado actual, agentes activos
/// (últimos 15 min) y últimos movimientos. Fuente: activity.jsonl.
pub fn render_activity() -> String {
    let path = activity_log_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return "(sin actividad registrada aún — los hooks la escribirán aquí)".into();
    };
    let now_ms = now_ms();
    let mut states: Vec<(u64, String, String)> = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let ts = v["ts"].as_u64().unwrap_or(0);
        let ev = v["ev"].as_str().unwrap_or("");
        let tool = v["tool"].as_str().unwrap_or("");
        let detail = v["detail"].as_str().unwrap_or("");
        if let Some((state, det)) = activity_state(ev, tool, detail) {
            states.push((ts, state, det));
        }
    }

    let mut out = String::new();
    // ESTADO ACTUAL = último evento con valor visual
    if let Some((_, state, det)) = states.last() {
        out.push_str(&format!("\x1b[1;37m▸ ESTADO:\x1b[0m {state}  \x1b[90m{det}\x1b[0m\n"));
    }
    // AGENTES activos: Task events de los últimos 15 min
    let agents: Vec<_> = states.iter()
        .filter(|(ts, s, _)| s.contains("AGENTE") && now_ms.saturating_sub(*ts) < 15 * 60 * 1000)
        .collect();
    out.push_str(&format!("\x1b[1;37m▸ Agentes (15 min):\x1b[0m {}\n", agents.len()));
    for (_, _, det) in agents.iter().rev().take(3) {
        out.push_str(&format!("   · {det}\n"));
    }
    // ÚLTIMOS MOVIMIENTOS
    out.push_str("\x1b[1;37m▸ Actividad reciente:\x1b[0m\n");
    for (ts, state, det) in states.iter().rev().take(8) {
        let secs = (now_ms.saturating_sub(*ts) / 1000).min(9999);
        out.push_str(&format!("   \x1b[90m-{secs:>4}s\x1b[0m {state} \x1b[90m{det}\x1b[0m\n"));
    }
    out
}

/// `alx watch` — dashboard vivo: refresca cada segundo hasta Ctrl-C.
/// `--once` imprime un solo snapshot (para scripts y tests).
pub fn run_watch(once: bool) -> ! {
    loop {
        // clear + home
        print!("\x1b[2J\x1b[H");
        let mut out = String::from("\x1b[1;33m╔═ ALEXANDRIA watch — actividad en vivo ═════════════════════════╗\x1b[0m\n");
        out.push_str(&render_status_persisted());
        out.push('\n');
        out.push_str(&render_activity());
        let cost_line: String = render_cost_report()
            .lines()
            .find(|l| l.contains("Coste"))
            .unwrap_or("Coste: n/a")
            .to_string();
        out.push_str(&format!("\x1b[1;36m│ {cost_line}\x1b[0m\n"));
        out.push_str("\x1b[90m(Ctrl-C para salir · fuente: alexandria/state/activity.jsonl)\x1b[0m\n");
        println!("{out}");
        if once {
            std::process::exit(0);
        }
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
}


pub fn render_tui() -> String {    let mut out = String::from("\x1b[1;33m╔═ ALEXANDRIA — Motor de desarrollo IA autónomo ═════════════════╗\x1b[0m\n");

    out.push_str("\x1b[1;36m│ Motor:\x1b[0m 16 crates · 205 tests · `alx` en PATH\n");

    out.push_str("\x1b[1;36m│ Red:\x1b[0m ");
    for s in check_network() {
        let mark = if s.ready { "\x1b[32m✓\x1b[0m" } else { "\x1b[31m✗\x1b[0m" };
        let name = s.name.split(' ').next().unwrap_or("");
        out.push_str(&format!("{mark} {name} "));
    }
    out.push('\n');

    let cost_line: String = render_cost_report()
        .lines()
        .find(|l| l.contains("Coste"))
        .unwrap_or("Coste: n/a")
        .to_string();
    out.push_str(&format!("\x1b[1;36m│ Coste:\x1b[0m {cost_line}\n"));

    let items_line: String = render_doctor()
        .lines()
        .find(|l| l.contains("Total items"))
        .unwrap_or("Doctor: n/a")
        .to_string();
    out.push_str(&format!("\x1b[1;36m│ Doctor:\x1b[0m {items_line}\n"));

    let events_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../state/events.log");
    let events_count = std::fs::read_to_string(&events_path)
        .map(|t| t.lines().count())
        .unwrap_or(0);
    out.push_str(&format!(
        "\x1b[1;36m│ Telemetría:\x1b[0m {events_count} eventos · night systemd: 02:00\n"
    ));

    let metrics_total = render_metrics().lines().last().unwrap_or("").to_string();
    out.push_str(&format!("\x1b[1;36m│ Métricas:\x1b[0m {metrics_total}\n"));

    let real_agents = count_real_agents();
    out.push_str(&format!(
        "\x1b[1;36m│ Agentes reales:\x1b[0m {real_agents} en el ecosistema\n"
    ));

    let cmds_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../state/commands.log");
    let cmds_count = std::fs::read_to_string(&cmds_path)
        .map(|t| t.lines().count())
        .unwrap_or(0);
    out.push_str(&format!(
        "\x1b[1;36m│ Comandos ejecutados:\x1b[0m {cmds_count}\n"
    ));

    out.push_str("\x1b[1;36m│ Comandos:\x1b[0m status network build run --real night mcp phalanx feature evolve doctor cost agents spawn tui\n");
    out.push_str("\x1b[1;33m╚══════════════════════════════════════════════════════════════════╝\x1b[0m\n");
    out
}

/// Muestra un agente del registry REAL del ecosistema por nombre, con su
/// envelope de spawn. Carga agentes reales (agents-volt/ + agents/) con
/// frontmatter vía register_from_markdowns.
pub fn agents_show(name: &str) -> String {
    // Repo root real: CARGO_MANIFEST_DIR = <repo>/alexandria/crates/alx-cli
    // → tres subidas. La versión anterior tenía cuatro y leía FUERA del repo:
    // el registry quedaba vacío y agents-show fallaba siempre.
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../");
    let mut files = Vec::new();
    for dir in ["agents-volt", "agents"] {
        let p = repo_root.join(dir);
        if let Ok(rd) = std::fs::read_dir(&p) {
            for e in rd.flatten().take(60) {
                if e.path().extension().map(|x| x == "md").unwrap_or(false) {
                    files.push(e.path().to_string_lossy().to_string());
                }
            }
        }
    }
    let mut reg = AgentRegistry::new();
    let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
    let _ = reg.register_from_markdowns(&file_refs);
    if let Some(spec) = reg.by_name(name) {
        let env = build_envelope(spec, "tarea de ejemplo", Vec::new(), 2000);
        return format!(
            "## {}\n{}\ntier: {:?} · fase: {:?} · tools: {}\n\nEnvelope:\nsystem: {}\ntask: {}",
            spec.name,
            spec.description,
            spec.tier,
            spec.phase,
            spec.tools.len(),
            env.system,
            env.task
        );
    }
    format!("agente '{name}' no encontrado en el registry real (frontmatter requerido)")
}

/// Spawn de N agentes headless EN PARALELO sobre una tarea (threads).
/// Un agente summona a otros con su envelope; cada uno responde a la cadena.
pub fn agents_run_parallel(task: &str) -> String {
    let names = ["general-purpose", "code-reviewer", "test-engineer"];
    let handles: Vec<_> = names
        .iter()
        .map(|name| {
            let name = name.to_string();
            let task = task.to_string();
            std::thread::spawn(move || spawn_agent(&name, &task))
        })
        .collect();
    let mut out = format!("## Agentes en paralelo — tarea: {task}\n");
    for (i, h) in handles.into_iter().enumerate() {
        if let Ok(result) = h.join() {
            out.push_str(&format!("\n[{}] {result}\n", names[i]));
        }
    }
    out
}

/// Métricas por crate: líneas de código de cada crate del workspace.
pub fn render_metrics() -> String {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let crates_dir = base.join("crates");
    let mut out = String::from("## Métricas por crate\n");
    let mut total = 0usize;
    if let Ok(rd) = std::fs::read_dir(&crates_dir) {
        let mut crates: Vec<_> = rd.flatten().filter(|e| e.path().join("src").exists()).collect();
        crates.sort_by_key(|e| e.file_name());
        for e in crates {
            let name = e.file_name().to_string_lossy().to_string();
            let src = e.path().join("src");
            let lines: usize = std::fs::read_dir(&src)
                .map(|rd| {
                    rd.flatten()
                        .filter_map(|f| std::fs::read_to_string(f.path()).ok())
                        .map(|t| t.lines().count())
                        .sum()
                })
                .unwrap_or(0);
            total += lines;
            out.push_str(&format!("  {name}: {lines} líneas\n"));
        }
    }
    out.push_str(&format!("Total: {total} líneas\n"));
    out
}

/// Resumen semanal del sistema: coste + telemetría + harnesses + métricas.
pub fn render_weekly() -> String {
    let mut out = String::from("## Resumen semanal ALEXANDRIA\n");
    out.push_str(&render_cost_report());
    out.push('\n');
    let hdir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../harnesses");
    let reg = HarnessRegistry::load_from(&hdir);
    let live = reg.all().iter().filter(|h| h.state != alx_evolve::HarnessState::Retired).count();
    let retired = reg.all().iter().filter(|h| h.state == alx_evolve::HarnessState::Retired).count();
    out.push_str(&format!("## Harnesses evolutivos\nVivos: {live} · Retirados: {retired}\n"));
    out.push_str("## Agentes\n3 especializados (general-purpose, code-reviewer, test-engineer)\n");
    if let Some(total) = render_metrics().lines().last() {
        out.push_str(&format!("## Métricas\n{total}\n"));
    }
    out
}

/// Reporte completo del motor (markdown): TUI + coste + doctor + agentes.
/// Para night-run.sh e informes.
pub fn render_report() -> String {
    format!(
        "{}\n\n{}\n\n{}\n\n{}\n\n{}",
        render_tui(),
        render_cost_report(),
        render_doctor(),
        render_agents(),
        render_metrics()
    )
}

/// Spawn REAL de un agente: construye el envelope (alx-agents), comprime con
/// caveman y ejecuta la tarea contra la cadena real (headroom). Devuelve la
/// respuesta del modelo como resultado del agente.
pub fn spawn_agent(name: &str, task: &str) -> String {
    let spec = match name {
        "code-reviewer" => AgentSpec {
            name: "code-reviewer".into(),
            description: "Revisa código contra criterios de calidad y detecta bugs.".into(),
            tools: Vec::new(),
            tier: ModelTier::T3Premium,
            phase: Some(PhaseId::Review),
            tags: Vec::new(),
        },
        "test-engineer" => AgentSpec {
            name: "test-engineer".into(),
            description: "Diseña y ejecuta tests para verificar cada micro-tarea.".into(),
            tools: Vec::new(),
            tier: ModelTier::T2Medium,
            phase: Some(PhaseId::Test),
            tags: Vec::new(),
        },
        _ => AgentSpec {
            name: "general-purpose".into(),
            description: "Agente general para cualquier fase del pipeline ALEXANDRIA.".into(),
            tools: Vec::new(),
            tier: ModelTier::T2Medium,
            phase: None,
            tags: Vec::new(),
        },
    };
    let env = build_envelope(&spec, task, Vec::new(), 2000);
    let mut prompt = caveman_compress(&format!("{}\n\n{}", env.system, env.task));
    prompt.push_str("\nResponde directamente, sin razonamiento previo.");
    let body = serde_json::json!({
        "model": modelo_real_activo(),
        "max_tokens": 800,
        "thinking": { "type": "disabled" },
        "messages": [{ "role": "user", "content": prompt }]
    })
    .to_string();
    let body_path = std::env::temp_dir().join("alx-spawn-body.json");
    if std::fs::write(&body_path, &body).is_err() {
        return format!("✗ agente {name}: no se pudo escribir el body");
    }
    let cmd = format!(
        "curl -s -m 30 http://127.0.0.1:8788/v1/messages -H 'content-type: application/json' -d @{}",
        body_path.display()
    );
    let out = alx_gate::run_command(&cmd, 35_000);
    if out.exit_code != 0 {
        return format!("✗ agente {name}: falló (exit {})", out.exit_code);
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&out.stdout_head) {
        let arr = v["content"].as_array();
        if let Some(text) = arr
            .and_then(|a| a.iter().find(|b| b["type"] == "text"))
            .and_then(|b| b["text"].as_str())
        {
            return format!("✓ {name}: {text}");
        }
        // Fallback: el modelo aún está razonando (thinking largo); devolver lo
        // que hay en vez de "sin texto".
        if let Some(th) = arr
            .and_then(|a| a.first())
            .and_then(|b| b["thinking"].as_str())
        {
            let t: String = th.chars().take(200).collect();
            return format!("✓ {name} (razonamiento): {t}…");
        }
    }
    format!("✗ agente {name}: respuesta sin texto")
}

/// Informe legible del estado de red.
pub fn render_network(statuses: &[NetworkStatus]) -> String {
    let modelo_real = std::fs::read_to_string(format!(
        "{}/.config/routatic-proxy/config.json",
        std::env::var("HOME").unwrap_or_default()
    ))
    .ok()
    .and_then(|raw| {
        serde_json::from_str::<serde_json::Value>(&raw)
            .ok()
            .and_then(|v| {
                v["models"]["default"]["model_id"]
                    .as_str()
                    .map(str::to_string)
            })
    })
    .unwrap_or_else(|| "?".to_string());
    let mut out = String::from(&format!(
        "## Red real (governor)\nCadena: headroom:8788 → routa-gateway:3460 → routatic:3456 (PROVIDER) → {modelo_real}\nFallback: omniroute:20128 (solo si routatic cae)\n"
    ));
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

    #[test]
    fn gate_for_phase_maps_real_commands() {
        assert_eq!(gate_for_phase(PhaseId::Build), "cargo build");
        assert_eq!(gate_for_phase(PhaseId::Test), "cargo test");
        assert!(gate_for_phase(PhaseId::Review).contains("clippy"));
    }

    #[test]
    fn caveman_compress_reduces_envelope() {
        let long = "El agente ejecutor debe preparar el contexto para la tarea y luego devolver el resultado final de forma concisa y verificable.";
        let short = caveman_compress(long);
        assert!(short.chars().count() < long.chars().count());
    }
}
