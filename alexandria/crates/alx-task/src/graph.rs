//! TaskGraph — el DAG de tareas (plan 08 §1).
//!
//! Máquina de estados con `TaskStatus::can_transition_to`, dependencias
//! resueltas por id, y consultas de planificación: tareas listas (todas las
//! deps Done) y tareas bloqueadas (alguna dep Failed/Blocked).

use alx_core::types::{Task, TaskStatus};
use serde::{Deserialize, Serialize};

/// Grafo acíclico dirigido de tareas, en orden de inserción.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskGraph {
    tasks: Vec<Task>,
}

impl TaskGraph {
    /// Grafo vacío.
    pub fn new() -> Self {
        Self::default()
    }

    /// Añade una tarea al grafo (en orden de inserción).
    pub fn add(&mut self, task: Task) {
        self.tasks.push(task);
    }

    /// Todas las tareas, en orden de inserción.
    pub fn all(&self) -> Vec<&Task> {
        self.tasks.iter().collect()
    }

    /// Tarea por id.
    pub fn by_id(&self, id: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// Tarea por id, con mutabilidad.
    pub fn by_id_mut(&mut self, id: &str) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    /// Transición de estado validada con `can_transition_to`.
    ///
    /// Error si la tarea no existe o si la transición no es válida.
    /// En éxito, `updated` se actualiza a `now`.
    pub fn transition(
        &mut self,
        task_id: &str,
        new_status: TaskStatus,
        now: u64,
    ) -> Result<(), String> {
        let task = self
            .by_id_mut(task_id)
            .ok_or_else(|| format!("tarea '{}' no existe en el grafo", task_id))?;
        if !task.status.can_transition_to(&new_status) {
            return Err(format!(
                "transición inválida: {:?} -> {:?}",
                task.status, new_status
            ));
        }
        task.status = new_status;
        task.updated = now;
        Ok(())
    }

    /// Tareas `Pending` cuyas dependencias están **todas** `Done`.
    ///
    /// Una tarea sin dependencias es lista por vacuidad. `now` no se usa
    /// (el promotor a `Ready` corre en otro punto del ciclo); se mantiene
    /// en la firma para no romper la API.
    pub fn ready_tasks(&self, now: u64) -> Vec<&Task> {
        let _ = now;
        self.tasks
            .iter()
            .filter(|t| {
                t.status == TaskStatus::Pending
                    && t.depends_on
                        .iter()
                        .all(|dep| self.by_id(dep).map(|d| d.status) == Some(TaskStatus::Done))
            })
            .collect()
    }

    /// Tareas activas (no terminales) con alguna dependencia en
    /// `Failed` o `Blocked` — necesitan atención o skip manual.
    pub fn blocked_tasks(&self) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|t| {
                !matches!(t.status, TaskStatus::Done | TaskStatus::Failed | TaskStatus::Skipped)
                    && t.depends_on.iter().any(|dep| {
                        matches!(
                            self.by_id(dep).map(|d| d.status),
                            Some(TaskStatus::Failed) | Some(TaskStatus::Blocked)
                        )
                    })
            })
            .collect()
    }

    /// ¿La tarea existe y está `Done`?
    pub fn is_done(&self, task_id: &str) -> bool {
        matches!(
            self.by_id(task_id).map(|t| t.status),
            Some(TaskStatus::Done)
        )
    }

    /// Resuelve `depends_on` a referencias de tarea (los ids huérfanos se omiten).
    pub fn dependencies_of(&self, task_id: &str) -> Vec<&Task> {
        match self.by_id(task_id) {
            None => Vec::new(),
            Some(task) => task
                .depends_on
                .iter()
                .filter_map(|dep| self.by_id(dep))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alx_core::types::PhaseId;

    fn task(id: &str, now: u64) -> Task {
        Task::new(id.into(), format!("tarea {}", id), PhaseId::Build, 1000, now)
    }

    #[test]
    fn valid_lifecycle_pending_ready_inprogress_done() {
        let mut g = TaskGraph::new();
        g.add(task("a", 1));
        g.transition("a", TaskStatus::Ready, 2).unwrap();
        g.transition("a", TaskStatus::InProgress, 3).unwrap();
        g.transition("a", TaskStatus::Done, 4).unwrap();
        assert!(g.is_done("a"));
        assert_eq!(g.by_id("a").unwrap().updated, 4);
    }

    #[test]
    fn invalid_transition_returns_error() {
        // Pending -> Done no es válido.
        let mut g = TaskGraph::new();
        g.add(task("a", 1));
        assert!(g.transition("a", TaskStatus::Done, 2).is_err());
        // Done -> InProgress no es válido (terminales bloqueados).
        let mut g2 = TaskGraph::new();
        g2.add(task("b", 1));
        for st in [TaskStatus::Ready, TaskStatus::InProgress, TaskStatus::Done] {
            g2.transition("b", st, 2).unwrap();
        }
        assert!(g2.transition("b", TaskStatus::InProgress, 3).is_err());
        // Tarea inexistente también es error.
        assert!(g2.transition("nope", TaskStatus::Ready, 1).is_err());
    }

    #[test]
    fn ready_tasks_require_all_deps_done() {
        let mut g = TaskGraph::new();
        g.add(task("a", 1));
        g.add(task("b", 1));
        let mut c = task("c", 1);
        c.depends_on = vec!["a".to_string(), "b".to_string()];
        g.add(c);

        // a y b sin deps -> listas; c espera a ambas.
        assert_eq!(g.ready_tasks(1).len(), 2);

        g.transition("a", TaskStatus::Ready, 2).unwrap();
        g.transition("a", TaskStatus::InProgress, 3).unwrap();
        g.transition("a", TaskStatus::Done, 4).unwrap();
        let ready: Vec<&str> = g.ready_tasks(5).iter().map(|t| t.id.as_str()).collect();
        assert!(!ready.contains(&"c"));

        g.transition("b", TaskStatus::Ready, 6).unwrap();
        g.transition("b", TaskStatus::InProgress, 7).unwrap();
        g.transition("b", TaskStatus::Done, 8).unwrap();
        let ready: Vec<&str> = g.ready_tasks(9).iter().map(|t| t.id.as_str()).collect();
        assert!(ready.contains(&"c"));
    }

    #[test]
    fn blocked_tasks_with_failed_dep() {
        let mut g = TaskGraph::new();
        g.add(task("a", 1));
        let mut b = task("b", 1);
        b.depends_on = vec!["a".to_string()];
        g.add(b);
        let mut d = task("d", 1);
        d.depends_on = vec!["e".to_string()];
        g.add(d);
        // e se bloquea a sí misma (sin dep rota): no debe aparecer.
        let e = task("e", 1);
        g.add(e);

        g.transition("a", TaskStatus::Ready, 2).unwrap();
        g.transition("a", TaskStatus::InProgress, 3).unwrap();
        g.transition("a", TaskStatus::Failed, 4).unwrap();
        g.transition("e", TaskStatus::Ready, 5).unwrap();
        g.transition("e", TaskStatus::Blocked, 6).unwrap();

        // b (dep a Failed) y d (dep e Blocked) están bloqueadas; e no tiene dep rota.
        let mut blocked: Vec<&str> = g.blocked_tasks().iter().map(|t| t.id.as_str()).collect();
        blocked.sort_unstable();
        assert_eq!(blocked, vec!["b", "d"]);
    }

    #[test]
    fn dependencies_of_resolves_refs_skips_orphans() {
        let mut g = TaskGraph::new();
        g.add(task("a", 1));
        g.add(task("b", 1));
        let mut c = task("c", 1);
        c.depends_on = vec!["a".to_string(), "missing".to_string()];
        g.add(c);

        let deps = g.dependencies_of("c");
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].id, "a");
        assert!(g.dependencies_of("nope").is_empty());
    }

    #[test]
    fn graph_serde_roundtrip() {
        let mut g = TaskGraph::new();
        g.add(task("a", 1));
        let json = serde_json::to_string(&g).unwrap();
        let back: TaskGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(back.all().len(), 1);
        assert!(!back.is_done("a"));
    }
}
