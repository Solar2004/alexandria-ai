//! Tipos fundamentales del sistema.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// ID con prefijo por entidad: `t-<uuid>` tarea, `a-<slug>` agente, `h-<slug>` hook.
pub type AlxId = String;

/// Fases del pipeline (harness).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhaseId {
    Ingest,
    Spec,
    Plan,
    Build,
    Test,
    Review,
    Docs,
    Ship,
}

impl PhaseId {
    pub const ALL: [PhaseId; 8] = [
        PhaseId::Ingest,
        PhaseId::Spec,
        PhaseId::Plan,
        PhaseId::Build,
        PhaseId::Test,
        PhaseId::Review,
        PhaseId::Docs,
        PhaseId::Ship,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            PhaseId::Ingest => "Ingest",
            PhaseId::Spec => "Spec",
            PhaseId::Plan => "Plan",
            PhaseId::Build => "Build",
            PhaseId::Test => "Test",
            PhaseId::Review => "Review",
            PhaseId::Docs => "Docs",
            PhaseId::Ship => "Ship",
        }
    }
}

/// Tier de modelo. Hoy el provider real es uno (routatic→deepseek-v4-flash);
/// el tier controla presupuesto, effort y nivel de compresión, no el modelo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelTier {
    T1Cheap,
    T2Medium,
    T3Premium,
}

/// Estado de una tarea del DAG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Ready,
    InProgress,
    Blocked,
    Done,
    Failed,
    Skipped,
}

impl TaskStatus {
    /// Transiciones válidas del estado. `Done/Failed/Skipped` son terminales.
    pub fn can_transition_to(&self, next: &TaskStatus) -> bool {
        use TaskStatus::*;
        match self {
            Pending => matches!(next, Ready | Blocked | Skipped),
            Ready => matches!(next, InProgress | Blocked | Skipped),
            InProgress => matches!(next, Done | Ready | Failed | Blocked),
            Blocked => matches!(next, Ready | Skipped),
            Done | Failed | Skipped => false,
        }
    }
}

/// Presupuesto de tokens por tarea.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudget {
    pub total: u32,
    pub spent: u32,
    pub warn_at_pct: u8,
    pub hard_cap_pct: u8,
}

impl TokenBudget {
    pub fn new(total: u32) -> Self {
        Self { total, spent: 0, warn_at_pct: 80, hard_cap_pct: 100 }
    }

    pub fn spend(&mut self, n: u32) {
        self.spent = self.spent.saturating_add(n).min(self.total);
    }

    pub fn pct_used(&self) -> f32 {
        if self.total == 0 {
            return 1.0;
        }
        self.spent as f32 / self.total as f32
    }

    pub fn is_warning(&self) -> bool {
        self.pct_used() * 100.0 >= self.warn_at_pct as f32
    }

    pub fn is_over(&self) -> bool {
        self.pct_used() * 100.0 >= self.hard_cap_pct as f32
    }
}

/// Tarea del DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: AlxId,
    pub title: String,
    pub status: TaskStatus,
    pub phase: PhaseId,
    pub depends_on: Vec<AlxId>,
    pub budget: TokenBudget,
    pub evidence: Vec<Evidence>,
    pub model_tier: ModelTier,
    /// Epoch millis.
    pub created: u64,
    /// Epoch millis.
    pub updated: u64,
}

impl Task {
    pub fn new(id: AlxId, title: String, phase: PhaseId, budget_total: u32, now: u64) -> Self {
        Self {
            id,
            title,
            status: TaskStatus::Pending,
            phase,
            depends_on: Vec::new(),
            budget: TokenBudget::new(budget_total),
            evidence: Vec::new(),
            model_tier: ModelTier::T2Medium,
            created: now,
            updated: now,
        }
    }
}

/// Tipo de evidencia de verificación.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceKind {
    BuildOutput,
    TestSummary,
    LintReport,
    BenchReport,
    CommandOutput,
}

/// Evidencia capturada de un comando real — la moneda de la verificación.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub kind: EvidenceKind,
    pub command: String,
    pub exit_code: i32,
    pub stdout_head: String,
    pub passed: bool,
    pub metrics: HashMap<String, f64>,
}

impl Evidence {
    pub fn command_output(command: &str, exit_code: i32, stdout_head: &str, passed: bool) -> Self {
        Self {
            kind: EvidenceKind::CommandOutput,
            command: command.to_string(),
            exit_code,
            stdout_head: stdout_head.to_string(),
            passed,
            metrics: HashMap::new(),
        }
    }
}

/// Origen de un recuerdo de memoria.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecallSource {
    Session,
    Tool,
    Project,
    User,
}

/// Recuerdo de memoria (auto-recall): texto comprimido (caveman) que se re-inyecta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recall {
    pub id: AlxId,
    pub text: String,
    pub source: RecallSource,
    pub tags: Vec<String>,
    pub weight: u32,
    pub created: u64,
}

/// Evento del bus central.
#[derive(Debug, Clone)]
pub enum Event {
    SessionStart,
    SessionStop,
    UserPromptSubmit(String),
    ToolPre(String),
    ToolPost(String),
    PhaseEntered(PhaseId),
    PhasePassed(PhaseId),
    PhaseFailed(PhaseId, String),
    ModelChosen(AlxId, ModelTier),
    TokenSpent(AlxId, u32),
    RecallInjected(usize),
    NightTick,
}

/// Epoch millis actual.
pub fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_status_transitions_valid() {
        assert!(TaskStatus::Pending.can_transition_to(&TaskStatus::Ready));
        assert!(TaskStatus::InProgress.can_transition_to(&TaskStatus::Done));
        assert!(TaskStatus::Blocked.can_transition_to(&TaskStatus::Ready));
    }

    #[test]
    fn task_status_terminal_is_locked() {
        assert!(!TaskStatus::Done.can_transition_to(&TaskStatus::InProgress));
        assert!(!TaskStatus::Failed.can_transition_to(&TaskStatus::Ready));
        assert!(!TaskStatus::Skipped.can_transition_to(&TaskStatus::Pending));
    }

    #[test]
    fn budget_warning_and_cap() {
        let mut b = TokenBudget::new(100);
        b.spend(85);
        assert!(b.is_warning());
        assert!(!b.is_over());
        b.spend(20); // 105 > 100
        assert!(b.is_over());
    }

    #[test]
    fn budget_saturating_spend() {
        let mut b = TokenBudget::new(10);
        b.spend(1000);
        assert_eq!(b.spent, 10);
        assert!(b.is_over());
    }

    #[test]
    fn phase_str() {
        assert_eq!(PhaseId::Build.as_str(), "Build");
        assert_eq!(PhaseId::ALL.len(), 8);
    }
}
