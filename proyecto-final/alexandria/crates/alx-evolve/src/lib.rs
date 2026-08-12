//! alx-evolve — harness evolutivo / self-evolución.
//!
//! La AI crea harnesses en tiempo real: detecta qué formalizar, los crea con
//! documentación mínima, los aplica, y un watcher de objetivos los retira
//! (temporales cumplidos) o los promueve (demostraron servir). Nada se escapa
//! sin doc-min. Spec: plan/16-evolve.md.

use serde::{Deserialize, Serialize};

/// Vida de un harness: temporal (muere al cumplir objetivo) o permanente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HarnessKind {
    Temporal,
    Permanent,
}

/// Estado del lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HarnessState {
    Active,
    WaitingObjective,
    Retired,
    Promoted,
}

/// Cuándo corre el harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    Event(String),   // "PostToolUse", "PhasePassed", ...
    Phase(String),   // "Build", "Review", ...
    Manual,
}

/// Un harness del sistema evolutivo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Harness {
    pub id: String,          // "hx-<slug>"
    pub name: String,
    pub kind: HarnessKind,
    pub trigger: Trigger,
    pub objective: String,
    /// Documentación mínima obligatoria: qué, por qué, cuándo.
    pub doc: String,
    pub state: HarnessState,
    pub created_by: String,
    pub created: u64,
    pub uses: u32,
}

impl Harness {
    /// Crea un harness nuevo. `doc` vacía = inválido (regla doc-min).
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        kind: HarnessKind,
        trigger: Trigger,
        objective: impl Into<String>,
        doc: impl Into<String>,
        created_by: impl Into<String>,
        created: u64,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            trigger,
            objective: objective.into(),
            doc: doc.into(),
            state: match kind {
                HarnessKind::Temporal => HarnessState::WaitingObjective,
                HarnessKind::Permanent => HarnessState::Active,
            },
            created_by: created_by.into(),
            created,
            uses: 0,
        }
    }

    /// Un harness temporal cumple su objetivo → se retira (autodestrucción).
    pub fn retire_if_goal_met(&mut self) -> bool {
        if self.kind == HarnessKind::Temporal && self.state == HarnessState::WaitingObjective {
            self.state = HarnessState::Retired;
            return true;
        }
        false
    }

    /// Temporal que demostró servir (usos >= umbral) → promueve a permanente.
    pub fn promote(&mut self, min_uses: u32) -> bool {
        if self.kind == HarnessKind::Temporal
            && self.state == HarnessState::WaitingObjective
            && self.uses >= min_uses
        {
            self.kind = HarnessKind::Permanent;
            self.state = HarnessState::Promoted;
            return true;
        }
        false
    }

    /// Registra una aplicación del harness.
    pub fn record_use(&mut self) {
        self.uses = self.uses.saturating_add(1);
    }
}

/// Registry de harnesses. Estado en memoria; el disco vive en
/// `proyecto-final/harnesses/` (active/*.toml + index.toml).
#[derive(Debug, Default, Clone)]
pub struct HarnessRegistry {
    harnesses: Vec<Harness>,
}

impl HarnessRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, h: Harness) {
        self.harnesses.push(h);
    }

    pub fn all(&self) -> &[Harness] {
        &self.harnesses
    }

    pub fn by_id(&self, id: &str) -> Option<&Harness> {
        self.harnesses.iter().find(|h| h.id == id)
    }

    pub fn by_id_mut(&mut self, id: &str) -> Option<&mut Harness> {
        self.harnesses.iter_mut().find(|h| h.id == id)
    }

    /// Watcher de objetivos: retira temporales que cumplieron. Devuelve los
    /// ids retirados.
    pub fn run_watcher(&mut self, goal_met: &dyn Fn(&Harness) -> bool) -> Vec<String> {
        let mut retired = Vec::new();
        for h in self.harnesses.iter_mut() {
            if h.state == HarnessState::WaitingObjective && goal_met(h) {
                h.state = HarnessState::Retired;
                retired.push(h.id.clone());
            }
        }
        retired
    }

    /// Promueve temporales con suficiente uso.
    pub fn promote_used(&mut self, min_uses: u32) -> Vec<String> {
        let mut promoted = Vec::new();
        for h in self.harnesses.iter_mut() {
            if h.promote(min_uses) {
                promoted.push(h.id.clone());
            }
        }
        promoted
    }

    /// Nº de harnesses vivos (no Retired).
    pub fn live_count(&self) -> usize {
        self.harnesses.iter().filter(|h| h.state != HarnessState::Retired).count()
    }
}

/// doc-min: un harness sin documentación no puede registrarse.
pub fn validate_doc_min(h: &Harness) -> Result<(), String> {
    if h.doc.trim().is_empty() {
        return Err(format!("harness {} sin doc-min (obligatoria)", h.id));
    }
    if h.doc.trim().chars().count() < 20 {
        return Err(format!("harness {} doc-min demasiado corta (<20 chars)", h.id));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn harness(id: &str, kind: HarnessKind, now: u64) -> Harness {
        Harness::new(
            id,
            id,
            kind,
            Trigger::Phase("Build".into()),
            "mantener consistencia",
            "Verifica que la salida de la fase cumple las reglas establecidas del proyecto.",
            "alx-evolve",
            now,
        )
    }

    #[test]
    fn temporal_starts_waiting_permanent_active() {
        let t = harness("t1", HarnessKind::Temporal, 1);
        let p = harness("p1", HarnessKind::Permanent, 1);
        assert_eq!(t.state, HarnessState::WaitingObjective);
        assert_eq!(p.state, HarnessState::Active);
    }

    #[test]
    fn retire_when_goal_met() {
        let mut h = harness("t1", HarnessKind::Temporal, 1);
        assert!(h.retire_if_goal_met());
        assert_eq!(h.state, HarnessState::Retired);
        // doble retiro no hace nada
        assert!(!h.retire_if_goal_met());
    }

    #[test]
    fn permanent_never_retires_by_goal() {
        let mut h = harness("p1", HarnessKind::Permanent, 1);
        assert!(!h.retire_if_goal_met());
        assert_eq!(h.state, HarnessState::Active);
    }

    #[test]
    fn promote_with_enough_uses() {
        let mut h = harness("t1", HarnessKind::Temporal, 1);
        h.record_use();
        h.record_use();
        assert!(h.promote(2));
        assert_eq!(h.kind, HarnessKind::Permanent);
        assert_eq!(h.state, HarnessState::Promoted);
        // no se promueve de nuevo
        assert!(!h.promote(2));
    }

    #[test]
    fn watcher_retires_only_matching_temporals() {
        let mut reg = HarnessRegistry::new();
        reg.add(harness("t1", HarnessKind::Temporal, 1));
        reg.add(harness("p1", HarnessKind::Permanent, 1));
        let retired = reg.run_watcher(&|_| true);
        assert_eq!(retired, vec!["t1".to_string()]);
        assert_eq!(reg.live_count(), 1);
    }

    #[test]
    fn doc_min_rejects_empty() {
        let mut h = harness("t1", HarnessKind::Temporal, 1);
        h.doc = String::new();
        assert!(validate_doc_min(&h).is_err());
        h.doc = "corta".into();
        assert!(validate_doc_min(&h).is_err());
        h.doc = "Verifica que la salida de la fase cumple las reglas establecidas.".into();
        assert!(validate_doc_min(&h).is_ok());
    }
}
