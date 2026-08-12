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
}
