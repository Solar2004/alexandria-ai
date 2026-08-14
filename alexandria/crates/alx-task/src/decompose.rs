//! Decomposition engine (plan 15 §5) — tareas grandes → micro-tareas atómicas.
//!
//! Cada paso `(step, assert)` se convierte en una tarea hija `{parent}-m<i>`
//! con `depends_on = [parent] + [hermana anterior]` (cadena), fase heredada y
//! presupuesto repartido (`total / n`, mínimo 100). El assert se recupera por
//! id con [`micro_task_assert`] — el gate corre por micro-tarea, no por fase.

use alx_core::types::{now_ms, AlxId, Task};
use serde::{Deserialize, Serialize};

/// Id canónico de micro-tarea: `{parent}-m<1-based index>`.
pub fn micro_task_id(parent: &str, i: usize) -> String {
    format!("{}-m{}", parent, i)
}

/// Índice 1-based de una micro-tarea desde su id (sufijo `-m<N>`).
fn micro_task_index(task_id: &str) -> Option<usize> {
    let (_, suffix) = task_id.rsplit_once("-m")?;
    let n: usize = suffix.parse().ok()?;
    (n > 0).then_some(n)
}

/// Micro-tarea: un paso atómico verificable dentro de su padre.
///
/// `done_when` es el comando/criterio que prueba el paso; en la
/// descomposición simple coincide con `assert`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MicroTask {
    pub id: AlxId,
    pub parent: AlxId,
    /// Un paso atómico ("renombrar variable X en archivo Y").
    pub step: String,
    /// Cómo se verifica ESTE paso ("grep X ya no existe en Y").
    pub assert: String,
    /// Comando que prueba el paso (gate por micro-tarea).
    pub done_when: String,
}

impl MicroTask {
    /// Construye las micro-tareas de un padre. `done_when` se inicializa
    /// al mismo criterio que `assert` (la API de `decompose` no separa ambos).
    pub fn from_steps(parent: &Task, steps: Vec<(String, String)>) -> Vec<MicroTask> {
        steps
            .into_iter()
            .enumerate()
            .map(|(i, (step, assert))| {
                let done_when = assert.clone();
                MicroTask {
                    id: micro_task_id(&parent.id, i + 1),
                    parent: parent.id.clone(),
                    step,
                    assert,
                    done_when,
                }
            })
            .collect()
    }
}

/// Descompone `parent` en una micro-tarea por paso de `steps: (step, assert)`.
///
/// Cada hija: id `{parent}-m{i+1}`, título = step, fase heredada, presupuesto
/// `parent.budget.total / steps.len()` (mínimo 100), `depends_on = [parent]`
/// más la hermana anterior cuando i > 0, estado `Pending`, `created/updated = now`.
pub fn decompose(parent: &Task, steps: Vec<(String, String)>) -> Vec<Task> {
    decompose_at(parent, steps, now_ms())
}

/// Versión con `now` inyectado para testear.
fn decompose_at(parent: &Task, steps: Vec<(String, String)>, now: u64) -> Vec<Task> {
    let n = steps.len();
    if n == 0 {
        return Vec::new();
    }
    let per = (parent.budget.total / n as u32).max(100);
    let mut out = Vec::with_capacity(n);
    for (i, (step, _assert)) in steps.into_iter().enumerate() {
        let id = micro_task_id(&parent.id, i + 1);
        let mut t = Task::new(id, step, parent.phase, per, now);
        t.depends_on.push(parent.id.clone());
        if i > 0 {
            t.depends_on.push(micro_task_id(&parent.id, i));
        }
        out.push(t);
    }
    out
}

/// Recupera el `assert` de una micro-tarea por su id (formato `{parent}-m<N>`)
/// usando la misma lista de pasos `(step, assert)` con que se descompuso.
pub fn micro_task_assert<'a>(task_id: &str, steps: &'a [(String, String)]) -> Option<&'a str> {
    let idx = micro_task_index(task_id)?;
    steps.get(idx - 1).map(|(_, assert)| assert.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alx_core::types::{PhaseId, TaskStatus};

    fn parent(id: &str, budget: u32) -> Task {
        Task::new(id.into(), "tarea grande".into(), PhaseId::Build, budget, 1)
    }

    fn steps() -> Vec<(String, String)> {
        vec![
            ("crear schema users".into(), "migración aplica".into()),
            ("endpoint POST /login".into(), "cargo build pasa".into()),
            ("validar input".into(), "tests de validación verdes".into()),
            ("auth con < correcto".into(), "test cubre token expirado".into()),
        ]
    }

    #[test]
    fn decompose_creates_chained_children_with_split_budget() {
        let children = decompose(&parent("feat", 1000), steps());

        assert_eq!(children.len(), 4);
        assert_eq!(children[0].id, "feat-m1");
        assert_eq!(children[3].id, "feat-m4");

        // deps encadenadas: m1 -> [feat]; m2 -> [feat, feat-m1]; m3 -> [feat, feat-m2]
        assert_eq!(children[0].depends_on, vec!["feat".to_string()]);
        assert_eq!(children[1].depends_on, vec!["feat".to_string(), "feat-m1".to_string()]);
        assert_eq!(children[2].depends_on, vec!["feat".to_string(), "feat-m2".to_string()]);

        // presupuesto repartido 1000/4 = 250 por hija
        for c in &children {
            assert_eq!(c.budget.total, 250);
            assert_eq!(c.budget.spent, 0);
        }
        // fase heredada, Pending, created == updated
        for c in &children {
            assert_eq!(c.phase, PhaseId::Build);
            assert_eq!(c.status, TaskStatus::Pending);
            assert_eq!(c.created, c.updated);
            assert!(c.created > 0);
        }
    }

    #[test]
    fn decompose_budget_floor_100() {
        let children = decompose(&parent("p", 300), steps());
        assert_eq!(children.len(), 4);
        // 300/4 = 75 -> mínimo 100
        for c in &children {
            assert_eq!(c.budget.total, 100);
        }
    }

    #[test]
    fn decompose_empty_steps_returns_empty() {
        assert!(decompose(&parent("p", 100), Vec::new()).is_empty());
    }

    #[test]
    fn micro_task_assert_recovers_by_id() {
        let steps = steps();
        assert_eq!(micro_task_assert("feat-m1", &steps), Some("migración aplica"));
        assert_eq!(micro_task_assert("feat-m4", &steps), Some("test cubre token expirado"));
        // ids malformados o fuera de rango
        assert_eq!(micro_task_assert("feat-m5", &steps), None);
        assert_eq!(micro_task_assert("feat", &steps), None);
        assert_eq!(micro_task_assert("feat-mx", &steps), None);
        // ids de padre que contienen "-m" no confunden: se mira el último sufijo
        let nested = format!("{}-m1", micro_task_id("a-b-m2", 1));
        assert_eq!(micro_task_assert(&nested, &steps), Some("migración aplica"));
    }

    #[test]
    fn micro_tasks_carry_assert_and_done_when() {
        let micros = MicroTask::from_steps(&parent("feat", 500), steps());
        assert_eq!(micros.len(), 4);
        assert_eq!(micros[0].id, "feat-m1");
        assert_eq!(micros[0].parent, "feat");
        assert_eq!(micros[0].step, "crear schema users");
        assert_eq!(micros[0].assert, "migración aplica");
        assert_eq!(micros[0].done_when, "migración aplica");
        assert_eq!(micros[3].id, "feat-m4");

        // serde roundtrip (justifica serde_json)
        let json = serde_json::to_string(&micros[0]).unwrap();
        let back: MicroTask = serde_json::from_str(&json).unwrap();
        assert_eq!(back, micros[0]);
    }
}
