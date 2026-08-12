//! ALEXANDRIA core — el hueso del motor.
//!
//! Todo crate depende de aquí; aquí no depende de nadie salvo std + serde.
//! Fase 1: tipos, event bus simple (std mpsc), store JSONL append-only.

pub mod bus;
pub mod store;
pub mod types;
