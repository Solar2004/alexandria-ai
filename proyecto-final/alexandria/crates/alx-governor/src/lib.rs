//! alx-governor — Gobernador de coste del motor ALEXANDRIA.
//!
//! Decide QUÉ modelo/tier, CON QUÉ ruta (cadena de proxies), CON QUÉ
//! presupuesto, y mide todo (plan 09, token economics). Objetivo: ≥60%
//! menos tokens que una sesión manual equivalente.
//!
//! Módulos:
//! - [`classify`]: dificultad (señales/prompt) → score 0..1 → `ModelTier`.
//! - [`router`]: tier → cadena de proxies; fallback omniroute.
//! - [`budget`]: presupuesto por tarea (T1=2k, T2=15k, T3=60k).

pub mod budget;
pub mod classify;
pub mod router;

pub use budget::BudgetManager;
pub use classify::{classify, classify_prompt_text, tier_for_score, ClassificationInput};
pub use router::{Route, Router};
