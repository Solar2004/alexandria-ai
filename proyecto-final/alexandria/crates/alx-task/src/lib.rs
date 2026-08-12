//! alx-task — DAG de tareas + decomposition engine.
//!
//! El DAG (grafo acíclico dirigido) modela tareas con estados y dependencias
//! (plan 08 §1-§2); el decomposition engine rompe tareas grandes en micro-tareas
//! atómicas, cada una con su assert verificable (plan 15 §5 — "fallar barato").
//!
//! - [`graph::TaskGraph`]: estados, transiciones, tareas listas / bloqueadas.
//! - [`decompose`]: micro-tareas con `depends_on` encadenado y presupuesto repartido.

pub mod decompose;
pub mod graph;
