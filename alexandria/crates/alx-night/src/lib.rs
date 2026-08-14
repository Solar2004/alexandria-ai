//! alx-night — Scheduler autónomo (plan 11, fase 11.1-11.2).
//!
//! Agenda la pasada nocturna (`NightSchedule`, default 02:00), construye el
//! informe nocturno (`NightReport`) desde el DAG de tareas (plan 08 §5: el
//! informe apunta a `progress.md` como fuente de verdad para el humano) y lo
//! renderiza a markdown legible. El commit atómico (11.3) vive en una fase
//! posterior.
//!
//! - [`NightSchedule`]: cuándo corre (hora exacta + ventana de gracia).
//! - [`NightReport`]: hechas/pendientes/coste del DAG en una fecha.
//! - [`run_cycle`]: dispara el informe solo cuando toca.

use alx_core::types::TaskStatus;
use alx_task::graph::TaskGraph;
use serde::{Deserialize, Serialize};

/// Agenda de la pasada nocturna. `default()` = habilitada, 02:00.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NightSchedule {
    pub enabled: bool,
    pub hour: u8,
    pub minute: u8,
}

impl Default for NightSchedule {
    fn default() -> Self {
        Self { enabled: true, hour: 2, minute: 0 }
    }
}

impl NightSchedule {
    /// ¿Toca ahora? `enabled` y hora exacta (`now_hour`, `now_minute`).
    pub fn should_run(&self, now_hour: u8, now_minute: u8) -> bool {
        self.enabled && now_hour == self.hour && now_minute == self.minute
    }

    /// ¿Toca dentro de la ventana de gracia? Misma hora y minutos en
    /// `[self.minute, self.minute + grace_min]` (límite saturado a u8).
    pub fn should_run_grace(&self, now_hour: u8, now_minute: u8, grace_min: u8) -> bool {
        let end = self.minute.saturating_add(grace_min);
        self.enabled && now_hour == self.hour && now_minute >= self.minute && now_minute <= end
    }
}

/// Informe nocturno: resumen de lo hecho y lo pendiente del DAG.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NightReport {
    pub date: String,
    pub tasks_done: Vec<String>,
    pub tasks_pending: Vec<String>,
    pub cost_estimate_usd: f64,
    pub summary: String,
}

/// Construye el informe desde el grafo. `tasks_done` = títulos `Done`;
/// `tasks_pending` = títulos `Ready`/`Pending`/`Blocked`. El coste es un
/// placeholder (0.0) hasta que el governor alimente el ledger de gasto.
pub fn build_report(graph: &TaskGraph, date: &str) -> NightReport {
    let mut tasks_done = Vec::new();
    let mut tasks_pending = Vec::new();
    for t in graph.all() {
        match t.status {
            TaskStatus::Done => tasks_done.push(t.title.clone()),
            TaskStatus::Ready | TaskStatus::Pending | TaskStatus::Blocked => {
                tasks_pending.push(t.title.clone())
            }
            TaskStatus::InProgress | TaskStatus::Failed | TaskStatus::Skipped => {}
        }
    }
    let summary = format!(
        "{} hechas, {} pendientes",
        tasks_done.len(),
        tasks_pending.len()
    );
    NightReport {
        date: date.to_string(),
        tasks_done,
        tasks_pending,
        cost_estimate_usd: 0.0,
        summary,
    }
}

/// Renderiza el informe como markdown legible.
pub fn render(report: &NightReport) -> String {
    let mut out = String::new();
    out.push_str("## Informe nocturno\n\n");
    out.push_str(&format!("Fecha: {}\n\n", report.date));
    out.push_str(&format!("Resumen: {}\n\n", report.summary));

    out.push_str(&format!("### Hechas ({})\n", report.tasks_done.len()));
    if report.tasks_done.is_empty() {
        out.push_str("- (ninguna)\n");
    } else {
        for t in &report.tasks_done {
            out.push_str(&format!("- {}\n", t));
        }
    }
    out.push('\n');

    out.push_str(&format!("### Pendientes ({})\n", report.tasks_pending.len()));
    if report.tasks_pending.is_empty() {
        out.push_str("- (ninguna)\n");
    } else {
        for t in &report.tasks_pending {
            out.push_str(&format!("- {}\n", t));
        }
    }
    out.push('\n');

    out.push_str(&format!(
        "Coste estimado: {:.2} USD\n",
        report.cost_estimate_usd
    ));
    out
}

/// Ciclo nocturno: si el schedule dice que toca, construye el informe (fecha
/// = día civil UTC de hoy); si no, `None`.
pub fn run_cycle(
    schedule: &NightSchedule,
    graph: &mut TaskGraph,
    now_hour: u8,
    now_minute: u8,
) -> Option<NightReport> {
    if schedule.should_run(now_hour, now_minute) {
        Some(build_report(graph, &today()))
    } else {
        None
    }
}

/// Día civil UTC de hoy en `YYYY-MM-DD` (algoritmo civil_from_days; sin
/// chrono para no ampliar deps).
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alx_core::types::{PhaseId, Task};

    fn task(id: &str, now: u64) -> Task {
        Task::new(id.into(), format!("tarea {}", id), PhaseId::Build, 1000, now)
    }

    #[test]
    fn should_run_exact_hour() {
        let s = NightSchedule::default(); // 02:00
        assert!(s.should_run(2, 0));
        assert!(!s.should_run(2, 1));
        assert!(!s.should_run(3, 0));
    }

    #[test]
    fn should_run_when_disabled() {
        let s = NightSchedule { enabled: false, hour: 2, minute: 0 };
        assert!(!s.should_run(2, 0));
        assert!(!s.should_run_grace(2, 10, 15));
    }

    #[test]
    fn grace_window_includes_minutes() {
        let s = NightSchedule::default(); // 02:00
        assert!(s.should_run_grace(2, 0, 15));
        assert!(s.should_run_grace(2, 10, 15));
        assert!(s.should_run_grace(2, 15, 15));
        assert!(!s.should_run_grace(2, 16, 15));
        assert!(!s.should_run_grace(3, 5, 15));
    }

    #[test]
    fn build_report_separates_done_pending() {
        let mut g = TaskGraph::new();
        g.add(task("a", 1)); // Done
        g.add(task("b", 1)); // Ready
        g.add(task("c", 1)); // Pending
        g.add(task("d", 1)); // Blocked
        g.add(task("e", 1)); // Skipped (no cuenta)
        g.transition("a", TaskStatus::Ready, 2).unwrap();
        g.transition("a", TaskStatus::InProgress, 3).unwrap();
        g.transition("a", TaskStatus::Done, 4).unwrap();
        g.transition("b", TaskStatus::Ready, 2).unwrap();
        g.transition("d", TaskStatus::Blocked, 2).unwrap();
        g.transition("e", TaskStatus::Skipped, 2).unwrap();

        let report = build_report(&g, "2026-08-12");
        assert_eq!(report.tasks_done, vec!["tarea a"]);
        assert_eq!(report.tasks_pending, vec!["tarea b", "tarea c", "tarea d"]);
        assert_eq!(report.cost_estimate_usd, 0.0);
        assert_eq!(report.summary, "1 hechas, 3 pendientes");
    }

    #[test]
    fn render_contains_summary() {
        let report = NightReport {
            date: "2026-08-12".into(),
            tasks_done: vec!["tarea a".into()],
            tasks_pending: vec!["tarea b".into()],
            cost_estimate_usd: 0.0,
            summary: "1 hechas, 1 pendientes".into(),
        };
        let out = render(&report);
        assert!(out.contains("## Informe nocturno"));
        assert!(out.contains("Resumen: 1 hechas, 1 pendientes"));
        assert!(out.contains("- tarea a"));
        assert!(out.contains("- tarea b"));
        assert!(out.contains("Coste estimado"));
    }

    #[test]
    fn run_cycle_some_only_when_due() {
        let sched = NightSchedule::default(); // 02:00
        let mut g = TaskGraph::new();
        g.add(task("a", 1));

        assert!(run_cycle(&sched, &mut g, 3, 0).is_none());
        assert!(run_cycle(&sched, &mut g, 2, 1).is_none());

        let report = run_cycle(&sched, &mut g, 2, 0).unwrap();
        assert_eq!(report.summary, "0 hechas, 1 pendientes");
        assert_eq!(report.tasks_pending, vec!["tarea a"]);
    }
}
