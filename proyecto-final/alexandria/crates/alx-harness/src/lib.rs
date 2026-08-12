//! alx-harness — pipeline de fases: contratos, compuertas y runner reanudable.
//!
//! Define el contrato de cada fase (`Phase`, `Artifact`, `GateSpec`), la
//! secuencia estándar de 8 fases (`Phases::default`, Ingest→Ship) y el runner
//! `Pipeline::run_pipeline_step`, que avanza un `Task` fase a fase según pase
//! su compuerta, anexando `Evidence` a la tarea.
//!
//! La ejecución real de agentes llega en otra fase del roadmap; aquí solo se
//! modela el avance (gate verde → siguiente fase) y el fallo (gate roja →
//! reintento hasta `retries`, luego `TaskStatus::Failed`).

use alx_core::types::{Evidence, PhaseId, Task, TaskStatus};
use serde::{Deserialize, Serialize};

/// Tipo de artefacto que una fase produce o consume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactKind {
    Markdown,
    Diff,
    Test,
    Report,
    CommandOutput,
}

/// Artefacto de fase: ruta en disco + tipo + quién lo produjo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub path: String,
    pub kind: ArtifactKind,
    pub produced_by: String,
}

/// Compuerta de verificación: comando(s) que prueban la salida de la fase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateSpec {
    pub command: String,
    pub args: Vec<String>,
    pub expected_exit: i32,
}

impl GateSpec {
    /// Parse simple: `"echo phase ok"` → command `echo`, args `[phase, ok]`,
    /// `expected_exit` 0 (éxito).
    pub fn from_command(cmd: &str) -> Self {
        let mut parts = cmd.split_whitespace();
        let command = parts.next().unwrap_or("").to_string();
        let args: Vec<String> = parts.map(|s| s.to_string()).collect();
        Self { command, args, expected_exit: 0 }
    }

    /// Línea de comando completa (comando + args), para evidencia y logs.
    pub fn full_command(&self) -> String {
        if self.args.is_empty() {
            self.command.clone()
        } else {
            format!("{} {}", self.command, self.args.join(" "))
        }
    }
}

/// Contrato de una fase del pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Phase {
    pub id: PhaseId,
    pub output_artifacts: Vec<Artifact>,
    pub gate: GateSpec,
    /// Reintentos permitidos tras un fallo de compuerta (0 = falla a la primera).
    pub retries: u8,
}

impl Phase {
    pub fn new(id: PhaseId, gate: GateSpec, retries: u8) -> Self {
        Self { id, output_artifacts: Vec::new(), gate, retries }
    }
}

/// La secuencia estándar de fases del pipeline (Ingest→Ship).
///
/// `Default` monta las 8 fases de `PhaseId::ALL` en orden, cada una con una
/// compuerta placeholder (`echo <fase> ok`) y 2 reintentos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phases(pub Vec<Phase>);

impl Default for Phases {
    fn default() -> Self {
        Phases(
            PhaseId::ALL
                .iter()
                .map(|id| {
                    let cmd = format!("echo {} ok", id.as_str().to_lowercase());
                    Phase::new(*id, GateSpec::from_command(&cmd), 2)
                })
                .collect(),
        )
    }
}

/// Resultado de un paso del runner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResult {
    pub task_id: String,
    /// `true` cuando el pipeline terminó: última fase superada (Done) o
    /// reintentos agotados (Failed).
    pub completed: bool,
    /// Fase actual tras el paso: la siguiente (avance), la misma (reintento)
    /// o `None` si el pipeline terminó.
    pub current: Option<PhaseId>,
    /// Evidencia generada por este paso.
    pub evidence: Vec<Evidence>,
}

/// Runner del pipeline. Sin ejecución real de agentes: dado un `Task` y el
/// resultado de su compuerta, decide avance / reintento / fallo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub phases: Vec<Phase>,
}

impl Default for Pipeline {
    fn default() -> Self {
        Self { phases: Phases::default().0 }
    }
}

impl Pipeline {
    pub fn new(phases: Vec<Phase>) -> Self {
        Self { phases }
    }

    /// Fase por id (None si el pipeline no la incluye).
    pub fn phase_for(&self, id: PhaseId) -> Option<&Phase> {
        self.phases.iter().find(|p| p.id == id)
    }

    /// Fase siguiente en el orden de `PhaseId::ALL` (None si `id` es la última).
    pub fn next_after(&self, id: PhaseId) -> Option<PhaseId> {
        PhaseId::ALL
            .iter()
            .position(|p| *p == id)
            .and_then(|i| PhaseId::ALL.get(i + 1).copied())
    }

    /// Avanza el `Task` un paso por el pipeline.
    ///
    /// - Gate verde: anexa `Evidence` (passed=true), mueve `task.phase` a la
    ///   siguiente fase; si era la última, el pipeline termina y el `Task` pasa
    ///   a `Done`.
    /// - Gate roja: anexa `Evidence` (passed=false). Si quedan reintentos
    ///   (`retries` del `Phase`), la fase no avanza y se puede reintentar;
    ///   si se agotaron, el `Task` pasa a `Failed`.
    ///
    /// El estado se promueve a `InProgress` cuando el `Task` llega `Pending`/
    /// `Ready`, de modo que las transiciones terminales sean válidas según el
    /// modelo de `TaskStatus::can_transition_to`.
    pub fn run_pipeline_step(&self, task: &mut Task, gate_pass: bool, now: u64) -> PipelineResult {
        self.ensure_in_progress(task, now);
        let phase_id = task.phase;
        let Some(phase) = self.phase_for(phase_id) else {
            // Fase fuera del pipeline: sin avance, sin evidencia.
            return PipelineResult {
                task_id: task.id.clone(),
                completed: false,
                current: Some(phase_id),
                evidence: Vec::new(),
            };
        };

        let exit = if gate_pass { phase.gate.expected_exit } else { 1 };
        let head = if gate_pass { "ok" } else { "gate failed" };
        let ev = Evidence::command_output(&phase.gate.full_command(), exit, head, gate_pass);
        task.evidence.push(ev.clone());
        task.updated = now;

        if gate_pass {
            match self.next_after(phase_id) {
                Some(next) => {
                    task.phase = next;
                    PipelineResult {
                        task_id: task.id.clone(),
                        completed: false,
                        current: Some(next),
                        evidence: vec![ev],
                    }
                }
                None => {
                    self.set_terminal(task, TaskStatus::Done, now);
                    PipelineResult {
                        task_id: task.id.clone(),
                        completed: true,
                        current: None,
                        evidence: vec![ev],
                    }
                }
            }
        } else {
            // La racha de fallos consecutivos al final de la evidencia siempre
            // pertenece a la fase actual (un avance la corta).
            let prior_failures = self.trailing_failures(task) - 1;
            if (prior_failures as u8) < phase.retries {
                PipelineResult {
                    task_id: task.id.clone(),
                    completed: false,
                    current: Some(phase_id),
                    evidence: vec![ev],
                }
            } else {
                self.set_terminal(task, TaskStatus::Failed, now);
                PipelineResult {
                    task_id: task.id.clone(),
                    completed: true,
                    current: None,
                    evidence: vec![ev],
                }
            }
        }
    }

    /// Promueve `Pending → Ready → InProgress` para que el runner opere sobre
    /// una tarea "en ejecución" (contrato: el Task llega `Ready`).
    fn ensure_in_progress(&self, task: &mut Task, now: u64) {
        if task.status == TaskStatus::Pending {
            task.status = TaskStatus::Ready;
        }
        if task.status == TaskStatus::Ready {
            task.status = TaskStatus::InProgress;
        }
        task.updated = now;
    }

    /// Transición terminal validada (`InProgress → Done/Failed`).
    fn set_terminal(&self, task: &mut Task, target: TaskStatus, now: u64) {
        if task.status.can_transition_to(&target) {
            task.status = target;
            task.updated = now;
        }
    }

    /// Fallos consecutivos al final de la evidencia del `Task`.
    fn trailing_failures(&self, task: &Task) -> usize {
        task.evidence.iter().rev().take_while(|e| !e.passed).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alx_core::types::Evidence;
    use alx_task::graph::TaskGraph;

    fn task(id: &str, phase: PhaseId, now: u64) -> Task {
        let mut t = Task::new(id.into(), format!("tarea {}", id), phase, 1000, now);
        t.status = TaskStatus::Ready;
        t
    }

    #[test]
    fn phases_default_has_8_in_order() {
        let phases = Phases::default();
        assert_eq!(phases.0.len(), 8);
        let ids: Vec<PhaseId> = phases.0.iter().map(|p| p.id).collect();
        assert_eq!(ids, PhaseId::ALL.to_vec());
        assert_eq!(phases.0.last().unwrap().id, PhaseId::Ship);
        // Compuerta placeholder de ejemplo.
        assert_eq!(phases.0[0].gate.full_command(), "echo ingest ok");
        assert_eq!(phases.0[0].retries, 2);
    }

    #[test]
    fn gate_spec_parses_simple_command() {
        let g = GateSpec::from_command("echo phase ok");
        assert_eq!(g.command, "echo");
        assert_eq!(g.args, vec!["phase", "ok"]);
        assert_eq!(g.expected_exit, 0);
    }

    #[test]
    fn run_step_advances_phase_and_adds_evidence() {
        let p = Pipeline::default();
        let mut t = task("t1", PhaseId::Ingest, 1);
        let res = p.run_pipeline_step(&mut t, true, 2);

        assert_eq!(t.phase, PhaseId::Spec);
        assert_eq!(t.status, TaskStatus::InProgress);
        assert!(!res.completed);
        assert_eq!(res.current, Some(PhaseId::Spec));
        assert_eq!(res.task_id, "t1");

        assert_eq!(t.evidence.len(), 1);
        assert!(t.evidence[0].passed);
        assert_eq!(t.evidence[0].command, "echo ingest ok");
        assert_eq!(res.evidence.len(), 1);
    }

    #[test]
    fn run_step_fails_after_retries_exhausted() {
        let p = Pipeline::default(); // retries = 2
        let mut t = task("t1", PhaseId::Ingest, 1);

        let r1 = p.run_pipeline_step(&mut t, false, 2);
        assert!(!r1.completed);
        assert_eq!(r1.current, Some(PhaseId::Ingest));
        assert_eq!(t.phase, PhaseId::Ingest);
        assert_eq!(t.status, TaskStatus::InProgress);

        let r2 = p.run_pipeline_step(&mut t, false, 3);
        assert!(!r2.completed);
        assert_eq!(t.status, TaskStatus::InProgress);

        let r3 = p.run_pipeline_step(&mut t, false, 4);
        assert!(r3.completed);
        assert_eq!(r3.current, None);
        assert_eq!(t.status, TaskStatus::Failed);

        assert_eq!(t.evidence.len(), 3);
        assert!(t.evidence.iter().all(|e| !e.passed));
    }

    #[test]
    fn no_retries_fails_immediately() {
        let p = Pipeline::new(vec![Phase::new(
            PhaseId::Build,
            GateSpec::from_command("cargo build"),
            0,
        )]);
        let mut t = task("t1", PhaseId::Build, 1);
        let res = p.run_pipeline_step(&mut t, false, 2);
        assert!(res.completed);
        assert_eq!(t.status, TaskStatus::Failed);
        assert_eq!(t.evidence.len(), 1);
    }

    #[test]
    fn last_phase_completes_pipeline() {
        let p = Pipeline::default();
        let mut t = task("t1", PhaseId::Ship, 1);
        let res = p.run_pipeline_step(&mut t, true, 2);
        assert!(res.completed);
        assert_eq!(res.current, None);
        assert_eq!(t.status, TaskStatus::Done);
        assert_eq!(t.phase, PhaseId::Ship);
    }

    #[test]
    fn next_after_last_is_none() {
        let p = Pipeline::default();
        assert_eq!(p.next_after(PhaseId::Ship), None);
        assert_eq!(p.next_after(PhaseId::Build), Some(PhaseId::Test));
        assert_eq!(p.next_after(PhaseId::Ingest), Some(PhaseId::Spec));
    }

    #[test]
    fn full_pipeline_over_taskgraph_reaches_done() {
        let mut g = TaskGraph::new();
        g.add(task("t1", PhaseId::Ingest, 1));
        let p = Pipeline::default();
        for _ in 0..8 {
            p.run_pipeline_step(g.by_id_mut("t1").unwrap(), true, 2);
        }
        assert!(g.is_done("t1"));
        assert_eq!(g.by_id("t1").unwrap().evidence.len(), 8);
    }

    #[test]
    fn pipeline_serde_roundtrip() {
        let p = Pipeline::default();
        let json = serde_json::to_string(&p).unwrap();
        let back: Pipeline = serde_json::from_str(&json).unwrap();
        assert_eq!(back.phases.len(), 8);
        assert_eq!(
            back.phase_for(PhaseId::Build).unwrap().gate.full_command(),
            "echo build ok"
        );
        // El Task con evidencia también sobrevive al roundtrip.
        let mut t = task("t1", PhaseId::Spec, 1);
        p.run_pipeline_step(&mut t, true, 2);
        let tj = serde_json::to_string(&t).unwrap();
        let _back: Task = serde_json::from_str(&tj).unwrap();
        let _: &Evidence = &t.evidence[0];
    }
}
