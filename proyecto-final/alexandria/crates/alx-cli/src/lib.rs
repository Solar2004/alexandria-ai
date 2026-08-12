//! alx-cli — lógica reutilizable del binario `alx`.
//!
//! `AppState` guarda las tareas en memoria (Fase 2: skeleton funcional, sin
//! persistencia a disco) y `render_status` produce el texto de estado que el
//! binario pinta. `run_pipeline` integra el pipeline end-to-end: task → DAG →
//! descomposición → harness (gate simulado), y `render_run` pinta el informe.

use alx_core::types::{now_ms, PhaseId, Task, TaskStatus};
use alx_harness::{Phases, Pipeline};
use alx_task::decompose::decompose;
use alx_task::graph::TaskGraph;

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
}

/// Ejecuta el pipeline end-to-end sobre una tarea demo.
///
/// Flujo: crea `Task("t-demo", Plan)` → la añade al `TaskGraph` → la
/// descompone en 2 micro-tareas encadenadas → las corre por el `Pipeline`
/// con el gate simulado en `true` (la integración aquí es de flujo; el gate
/// real de comandos se conecta en otra fase). Cada micro-tarea avanza por el
/// pipeline hasta que termina (`Done` o `Failed`).
pub fn run_pipeline(title: &str) -> RunResult {
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
    for child in &children {
        let task = graph.by_id_mut(&child.id).expect("micro-tarea en el grafo");
        loop {
            let result = pipeline.run_pipeline_step(task, true, now_ms());
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
    }
}

/// Informe legible del resultado de una ejecución del pipeline.
pub fn render_run(result: &RunResult) -> String {
    format!(
        "## Pipeline run\nTítulo: {}\nMicro-tareas: {}, hechas: {}, fallidas: {}\nEvidencia: {}",
        result.task_title, result.micro_tasks, result.done, result.failed, result.evidence_count
    )
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
        assert!(r.evidence_count > 0);
    }

    #[test]
    fn render_run_contains_title() {
        let r = run_pipeline("feature demo");
        let text = render_run(&r);
        assert!(text.contains("feature demo"));
        assert!(text.contains("## Pipeline run"));
        assert!(text.contains("hechas: 2"));
    }

    #[test]
    fn status_via_facade_mentions_alexandria() {
        let alex = Alexandria::new();
        assert!(alex.status().contains("ALEXANDRIA"));
    }
}
