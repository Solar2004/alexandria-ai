//! alx-lib — Fachada publica de ALEXANDRIA: todo lo que PHALANX expone.
//!
//! Monta las piezas internas ([`EventBus`], [`TaskGraph`], [`HookRegistry`],
//! [`RecallStore`], [`Router`], [`AgentRegistry`]) bajo la struct [`Alexandria`]
//! y re-exporta los tipos clave de cada crate, de modo que un consumidor
//! externo use la fachada sin conocer el layout interno del workspace.
//!
//! ```no_run
//! use alx_lib::{Alexandria, Event};
//! let alex = Alexandria::new();
//! alex.emit(Event::SessionStart);
//! ```
//!
//! # Re-exports
//!
//! Tipos de `alx-core`, `alx-task`, `alx-hooks`, `alx-memory`,
//! `alx-governor` y `alx-agents` expuestos en la raíz.

// ——— Re-exports: la superficie pública ———
pub use alx_agents::{AgentRegistry, AgentSpec, build_envelope};
pub use alx_core::bus::EventBus;
pub use alx_core::types::{Event, Evidence, ModelTier, PhaseId, Task, TaskStatus};
pub use alx_governor::{BudgetManager, Router, classify};
pub use alx_hooks::{Hook, HookPriority, HookRegistry};
pub use alx_memory::{RecallStore, compress};
pub use alx_task::graph::TaskGraph;

/// La API de alto nivel de ALEXANDRIA.
///
/// Ensambla las piezas internas y ofrece los puntos de entrada que PHALANX
/// usa: montaje, resumen de estado y publicación de eventos en el bus central.
pub struct Alexandria {
    /// Bus central de eventos (broadcast a suscriptores).
    pub bus: EventBus,
    /// DAG de tareas.
    pub tasks: TaskGraph,
    /// Registro de hooks del engine de eventos.
    pub hooks: HookRegistry,
    /// Almacén de auto-recalls.
    pub memories: RecallStore,
    /// Rutas de tier → cadena de proxies + fallback.
    pub router: Router,
    /// Registro de specs de agentes.
    pub agents: AgentRegistry,
}

impl Default for Alexandria {
    fn default() -> Self {
        Self::new()
    }
}

impl Alexandria {
    /// Monta las piezas del sistema con sus estados iniciales.
    pub fn new() -> Self {
        Self {
            bus: EventBus::new(),
            tasks: TaskGraph::new(),
            hooks: HookRegistry::new(),
            memories: RecallStore::new(),
            router: Router::default_routes(),
            agents: AgentRegistry::new(),
        }
    }

    /// Resumen legible del estado del sistema: nº de tareas, hooks, recalls
    /// y agentes.
    pub fn status(&self) -> String {
        format!(
            "ALEXANDRIA — {} tareas, {} hooks, {} recalls, {} agentes",
            self.tasks.all().len(),
            self.hooks.list().len(),
            self.memories.len(),
            self.agents.all().len(),
        )
    }

    /// Publica un evento en el bus central (todos los suscriptores lo reciben).
    pub fn emit(&self, event: Event) {
        self.bus.publish(event);
    }
}

/// Versión de la fachada pública.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn new_mounts_all_components() {
        let alex = Alexandria::new();
        assert_eq!(alex.bus.subscriber_count(), 0);
        assert!(alex.tasks.all().is_empty());
        assert!(alex.hooks.list().is_empty());
        assert!(alex.memories.is_empty());
        assert!(alex.agents.all().is_empty());
        // Router montado con rutas por defecto: T1 → routatic directo.
        assert!(alex.router.route_for(&ModelTier::T1Cheap).is_some());
        assert!(!alex.router.fallback_url().is_empty());
    }

    #[test]
    fn status_mentions_alexandria_and_zero_counts_at_start() {
        let alex = Alexandria::new();
        let s = alex.status();
        assert!(s.contains("ALEXANDRIA"));
        assert!(s.contains("0 tareas"));
        assert!(s.contains("0 hooks"));
        assert!(s.contains("0 recalls"));
        assert!(s.contains("0 agentes"));
    }

    #[test]
    fn status_reflects_growth_after_adding_items() {
        let mut alex = Alexandria::new();
        alex.tasks
            .add(Task::new("t-1".into(), "hacer algo".into(), PhaseId::Build, 1000, 0));
        alex.hooks.add(Hook::new("h-1", "SessionStart", HookPriority::Pre));
        alex.memories.add(alx_memory::Recall {
            id: "r-1".into(),
            text: "auth < check".into(),
            source: alx_memory::RecallSource::Session,
            tags: Vec::new(),
            weight: 1,
            created: 0,
        });
        alex.agents.add(AgentSpec { name: "a-1".into(), ..Default::default() });

        let s = alex.status();
        assert!(s.contains("1 tareas"));
        assert!(s.contains("1 hooks"));
        assert!(s.contains("1 recalls"));
        assert!(s.contains("1 agentes"));
    }

    #[test]
    fn emit_publishes_event_to_subscribers() {
        let alex = Alexandria::new();
        let rx = alex.bus.subscribe();
        alex.emit(Event::NightTick);
        let got = rx.recv_timeout(Duration::from_millis(200));
        assert!(matches!(got, Ok(Event::NightTick)));
    }

    #[test]
    fn emit_reaches_all_subscribers() {
        let alex = Alexandria::new();
        let rx1 = alex.bus.subscribe();
        let rx2 = alex.bus.subscribe();
        alex.emit(Event::SessionStop);
        assert!(matches!(
            rx1.recv_timeout(Duration::from_millis(200)),
            Ok(Event::SessionStop)
        ));
        assert!(matches!(
            rx2.recv_timeout(Duration::from_millis(200)),
            Ok(Event::SessionStop)
        ));
    }

    #[test]
    fn version_is_0_3_1() {
        assert_eq!(version(), "0.3.2");
    }

    #[test]
    fn reexports_compile() {
        // Core: Task::new y TaskStatus llegan por la fachada.
        let task = Task::new("t-1".into(), "re-export".into(), PhaseId::Build, 1000, 0);
        assert_eq!(task.status, TaskStatus::Pending);
        let _ev: Event = Event::NightTick;
        let _tier: ModelTier = ModelTier::T2Medium;
        let _evidence = Evidence::command_output("cargo test", 0, "ok", true);
        let _bus: EventBus = EventBus::new();
        // Task.
        let _graph: TaskGraph = TaskGraph::new();
        // Hooks.
        let _hook = Hook::new("h-1", "SessionStart", HookPriority::Pre);
        let _reg: HookRegistry = HookRegistry::new();
        // Memory.
        let _store: RecallStore = RecallStore::new();
        let _c = compress("el token auth");
        // Governor.
        let _router: Router = Router::default_routes();
        let _budget = BudgetManager::allocate(&ModelTier::T1Cheap);
        let _score: f64 =
            classify("", &alx_governor::classify::ClassificationInput::new(false, false, false, false, false));
        // Agents.
        let _agents: AgentRegistry = AgentRegistry::new();
        let _spec: AgentSpec = AgentSpec::default();
        let _fn = build_envelope;
    }
}
