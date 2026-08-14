//! alx-hooks — Engine de eventos: ciclo Pre/Async/Post, timeout, lock.
//!
//! Cada evento del bus central (`alx_core::types::Event`) dispara una cadena de
//! hooks. Los hooks capturan el conocimiento que el dev "siempre repite" para que
//! la AI se auto-recuerde sola (auto-memoria, gobernanza, compuertas).
//!
//! # Fase 1 (este crate)
//!
//! No ejecuta procesos reales. `HookRegistry` almacena la config de hooks y
//! `Dispatcher::dispatch` resuelve qué hooks corren para un evento, en orden de
//! prioridad:
//!
//! 1. `HookPriority::Pre` — bloqueante; si falla y tiene `lock`, aborta el pipeline.
//! 2. `HookPriority::Async` — best-effort, en paralelo, con timeout.
//! 3. `HookPriority::Post` — registro, memoria, métricas.
//!
//! El resultado es un [`DispatchPlan`] con los hooks a ejecutar y si algún hook
//! `Pre` con `lock` aborta la cadena (`blocked`).
//!
//! # Modelo
//!
//! Un `Hook` es dato puro (serde-serializable) que vive en config
//! (`phalanx/hooks/*.toml`), no hardcodeado. Cada hook declara:
//!
//! - `event`: nombre del evento que lo dispara (string, ej. `"PhasePassed"`).
//! - `priority`: cuándo corre en la cadena.
//! - `command`: binario + args o descripción de la acción.
//! - `timeout_ms`: presupuesto de tiempo (default 5000).
//! - `lock`: true = si falla, aborta el pipeline (solo importa en `Pre`).
//! - `retry`: reintentos best-effort en fallo.
//! - `enabled`: los hooks deshabilitados no corren.

use alx_core::types::Event;
use serde::{Deserialize, Serialize};

/// Momento en el que corre un hook dentro de la cadena de un evento.
///
/// El orden de declaración define el orden de ejecución: `Pre` antes que `Async`
/// antes que `Post` (deriva `Ord`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HookPriority {
    /// Bloqueante: corre primero; si falla y tiene `lock`, aborta el pipeline.
    Pre,
    /// Best-effort: corre en paralelo con timeout; un fallo no tumba la sesión.
    Async,
    /// Registro: memoria, métricas, observabilidad. Nunca aborta.
    Post,
}

/// Config de un hook: dato puro, serde-serializable, cargable desde TOML/JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hook {
    /// ID único del hook, ej. `"h-phalanx-mission"`.
    pub id: String,
    /// Nombre del evento que lo dispara, ej. `"PhasePassed"` (ver [`event_name`]).
    pub event: String,
    /// Prioridad dentro de la cadena: Pre | Async | Post.
    pub priority: HookPriority,
    /// Binario + args, o descripción de la acción si es nativo.
    pub command: String,
    /// Presupuesto de tiempo en ms (default 5000).
    pub timeout_ms: u64,
    /// Si falla y es `Pre`, aborta el pipeline.
    pub lock: bool,
    /// Reintentos best-effort en fallo.
    pub retry: u8,
    /// Si false, el hook nunca corre (ni se incluye en el plan de dispatch).
    pub enabled: bool,
    /// Qué resuelve → documentación viva del hook.
    pub description: String,
}

impl Hook {
    /// Constructor mínimo con defaults razonables: timeout 5000ms, sin lock,
    /// sin retries, habilitado.
    pub fn new(id: impl Into<String>, event: impl Into<String>, priority: HookPriority) -> Self {
        Self {
            id: id.into(),
            event: event.into(),
            priority,
            command: String::new(),
            timeout_ms: 5000,
            lock: false,
            retry: 0,
            enabled: true,
            description: String::new(),
        }
    }

    /// Marca el hook como lock (aborta el pipeline si falla). Útil al construir.
    pub fn with_lock(mut self) -> Self {
        self.lock = true;
        self
    }

    /// Pone la descripción del hook. Útil al construir.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }
}

/// Registro de hooks: almacena la config y resuelve qué hooks corren por evento.
#[derive(Debug, Default)]
pub struct HookRegistry {
    hooks: Vec<Hook>,
}

impl HookRegistry {
    /// Registro vacío.
    pub fn new() -> Self {
        Self::default()
    }

    /// Añade un hook al registro. El mismo ID puede re-registrarse (se duplica);
    /// los que comparten evento+prioridad corren en orden de inserción.
    pub fn add(&mut self, hook: Hook) {
        self.hooks.push(hook);
    }

    /// Todos los hooks registrados, en orden de inserción (incluye deshabilitados).
    pub fn list(&self) -> &[Hook] {
        &self.hooks
    }

    /// Hooks habilitados que corren para `event`, ordenados Pre → Async → Post
    /// (orden estable dentro de la misma prioridad).
    pub fn by_event(&self, event: &str) -> Vec<&Hook> {
        let mut matched: Vec<&Hook> = self
            .hooks
            .iter()
            .filter(|h| h.enabled && h.event == event)
            .collect();
        matched.sort_by_key(|h| h.priority);
        matched
    }

    /// Habilita el hook con `id`. Devuelve false si no existe.
    pub fn enable(&mut self, id: &str) -> bool {
        match self.hooks.iter_mut().find(|h| h.id == id) {
            Some(h) => {
                h.enabled = true;
                true
            }
            None => false,
        }
    }

    /// Deshabilita el hook con `id`. Devuelve false si no existe.
    pub fn disable(&mut self, id: &str) -> bool {
        match self.hooks.iter_mut().find(|h| h.id == id) {
            Some(h) => {
                h.enabled = false;
                true
            }
            None => false,
        }
    }
}

/// Plan de ejecución para un evento: los hooks que corren (en orden de
/// prioridad) y si algún hook `Pre` con `lock` aborta la cadena.
#[derive(Debug)]
pub struct DispatchPlan<'a> {
    /// Nombre del evento que disparó el plan (ej. `"PhasePassed"`).
    pub event: String,
    /// Hooks habilitados a ejecutar, en orden Pre → Async → Post.
    pub hooks: Vec<&'a Hook>,
    /// True si al menos un hook `Pre` tiene `lock`: un fallo abortaría el pipeline.
    pub blocked: bool,
    /// El primer hook `Pre` con `lock` (si existe) — identifica quién aborta.
    pub blocking_hook: Option<&'a Hook>,
}

/// Resultado de despachar un evento: envuelve el [`DispatchPlan`].
#[derive(Debug)]
pub struct DispatchResult<'a> {
    /// El plan resuelto.
    pub plan: DispatchPlan<'a>,
}

impl<'a> DispatchResult<'a> {
    fn new(plan: DispatchPlan<'a>) -> Self {
        Self { plan }
    }

    /// Acceso directo a la lista ordenada de hooks del plan.
    pub fn hooks(&self) -> &[&'a Hook] {
        &self.plan.hooks
    }
}

/// Despacha eventos contra un [`HookRegistry`]: resuelve la cadena de hooks que
/// corre para cada evento. Fase 1: no ejecuta procesos, solo resuelve el plan.
#[derive(Debug)]
pub struct Dispatcher<'r> {
    registry: &'r HookRegistry,
}

impl<'r> Dispatcher<'r> {
    /// Crea un dispatcher sobre un registro (no lo posee).
    pub fn new(registry: &'r HookRegistry) -> Self {
        Self { registry }
    }

    /// Dado un evento, resuelve qué hooks habilitados corren (Pre → Async → Post)
    /// y si algún hook `Pre` con `lock` bloquea la cadena.
    pub fn dispatch(&self, event: &Event) -> DispatchResult<'r> {
        let name = event_name(event);
        let hooks = self.registry.by_event(name);
        let blocking_hook = hooks.iter().copied().find(|h| h.priority == HookPriority::Pre && h.lock);
        DispatchResult::new(DispatchPlan {
            event: name.to_string(),
            blocked: blocking_hook.is_some(),
            blocking_hook,
            hooks,
        })
    }
}

/// Nombre del evento como string — el contrato de matching evento→hook.
///
/// Los hooks declaran su `event` con este nombre exacto (ej. `"PhasePassed"`).
pub fn event_name(event: &Event) -> &str {
    match event {
        Event::SessionStart => "SessionStart",
        Event::SessionStop => "SessionStop",
        Event::UserPromptSubmit(_) => "UserPromptSubmit",
        Event::ToolPre(_) => "ToolPre",
        Event::ToolPost(_) => "ToolPost",
        Event::PhaseEntered(_) => "PhaseEntered",
        Event::PhasePassed(_) => "PhasePassed",
        Event::PhaseFailed(..) => "PhaseFailed",
        Event::ModelChosen(..) => "ModelChosen",
        Event::TokenSpent(..) => "TokenSpent",
        Event::RecallInjected(_) => "RecallInjected",
        Event::NightTick => "NightTick",
        Event::IterateRequest(..) => "IterateRequest",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alx_core::types::PhaseId;

    fn hook(id: &str, event: &str, priority: HookPriority) -> Hook {
        Hook::new(id, event, priority)
    }

    /// Registro con los 3 hooks del catálogo para `PhasePassed`.
    fn phase_passed_registry() -> HookRegistry {
        let mut r = HookRegistry::new();
        r.add(hook("gate.verify", "PhasePassed", HookPriority::Pre).with_lock());
        r.add(hook("memory.capture", "PhasePassed", HookPriority::Post));
        r.add(hook("bench.sample", "PhasePassed", HookPriority::Async));
        r
    }

    #[test]
    fn registro_add_y_list() {
        let mut r = HookRegistry::new();
        r.add(hook("h-1", "SessionStart", HookPriority::Pre));
        r.add(hook("h-2", "SessionStart", HookPriority::Async));
        assert_eq!(r.list().len(), 2);
        assert_eq!(r.list()[0].id, "h-1");
        assert_eq!(r.list()[1].priority, HookPriority::Async);
    }

    #[test]
    fn dispatch_solo_hooks_del_evento() {
        let mut r = HookRegistry::new();
        r.add(hook("mission", "UserPromptSubmit", HookPriority::Pre));
        r.add(hook("gate.verify", "PhasePassed", HookPriority::Pre));
        r.add(hook("memory.capture", "ToolPost", HookPriority::Post));
        let d = Dispatcher::new(&r);

        // Solo los hooks de PhasePassed corren en ese evento.
        let plan = d.dispatch(&Event::PhasePassed(PhaseId::Build)).plan;
        assert_eq!(plan.event, "PhasePassed");
        let ids: Vec<&str> = plan.hooks.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, vec!["gate.verify"]);
    }

    #[test]
    fn orden_prioridad_pre_async_post() {
        let r = phase_passed_registry();
        let d = Dispatcher::new(&r);
        let plan = d.dispatch(&Event::PhasePassed(PhaseId::Build)).plan;
        let prios: Vec<HookPriority> = plan.hooks.iter().map(|h| h.priority).collect();
        assert_eq!(
            prios,
            vec![HookPriority::Pre, HookPriority::Async, HookPriority::Post]
        );
        // Y el orden de IDs refleja el catálogo.
        let ids: Vec<&str> = plan.hooks.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, vec!["gate.verify", "bench.sample", "memory.capture"]);
    }

    #[test]
    fn orden_estable_dentro_de_la_misma_prioridad() {
        let mut r = HookRegistry::new();
        r.add(hook("a", "NightTick", HookPriority::Post));
        r.add(hook("b", "NightTick", HookPriority::Async));
        r.add(hook("c", "NightTick", HookPriority::Post));
        let d = Dispatcher::new(&r);
        let ids: Vec<&str> = d
            .dispatch(&Event::NightTick)
            .plan
            .hooks
            .iter()
            .map(|h| h.id.as_str())
            .collect();
        // Async entre los dos Post, y los Post conservan orden de inserción.
        assert_eq!(ids, vec!["b", "a", "c"]);
    }

    #[test]
    fn lock_en_pre_bloquea_la_cadena() {
        // gate.verify es Pre con lock.
        let r = phase_passed_registry();
        let d = Dispatcher::new(&r);
        let res = d.dispatch(&Event::PhasePassed(PhaseId::Ship));
        assert!(res.plan.blocked);
        assert_eq!(res.plan.blocking_hook.unwrap().id, "gate.verify");
    }

    #[test]
    fn sin_lock_en_pre_no_bloquea() {
        let mut r = HookRegistry::new();
        r.add(hook("memory.inject", "SessionStart", HookPriority::Pre)); // sin lock
        r.add(hook("governor.load", "SessionStart", HookPriority::Pre)); // sin lock
        let d = Dispatcher::new(&r);
        let res = d.dispatch(&Event::SessionStart);
        assert!(!res.plan.blocked);
        assert!(res.plan.blocking_hook.is_none());
    }

    #[test]
    fn hook_disabled_no_corre() {
        let mut r = phase_passed_registry();
        assert!(r.disable("bench.sample"));

        // Deshabilitado: no aparece en el plan de dispatch. El bloque suelta el
        // borrow del registry antes de mutarlo.
        let ids: Vec<&str> = {
            let d = Dispatcher::new(&r);
            d.dispatch(&Event::PhasePassed(PhaseId::Build))
                .plan
                .hooks
                .iter()
                .map(|h| h.id.as_str())
                .collect()
        };
        // bench.sample (Async) ya no está; Pre y Post siguen.
        assert_eq!(ids, vec!["gate.verify", "memory.capture"]);

        // Re-habilitar lo devuelve al plan.
        assert!(r.enable("bench.sample"));
        let d2 = Dispatcher::new(&r);
        let ids2: Vec<&str> = d2
            .dispatch(&Event::PhasePassed(PhaseId::Build))
            .plan
            .hooks
            .iter()
            .map(|h| h.id.as_str())
            .collect();
        assert_eq!(ids2, vec!["gate.verify", "bench.sample", "memory.capture"]);
    }

    #[test]
    fn enable_disable_de_id_inexistente_devuelve_false() {
        let mut r = HookRegistry::new();
        assert!(!r.enable("no-existe"));
        assert!(!r.disable("no-existe"));
    }

    #[test]
    fn by_event_ignora_deshabilitados_y_filtra_por_nombre() {
        let mut r = phase_passed_registry();
        r.add(hook("otro", "ToolPost", HookPriority::Post));
        r.disable("memory.capture");

        let matched = r.by_event("PhasePassed");
        let ids: Vec<&str> = matched.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, vec!["gate.verify", "bench.sample"]);

        // El registro completo sigue teniendo todo (deshabilitado incluido).
        assert_eq!(r.list().len(), 4);
    }

    #[test]
    fn event_name_cubre_todas_las_variantes() {
        use alx_core::types::{ModelTier, PhaseId};
        assert_eq!(event_name(&Event::SessionStart), "SessionStart");
        assert_eq!(event_name(&Event::UserPromptSubmit("x".into())), "UserPromptSubmit");
        assert_eq!(event_name(&Event::ToolPre("Edit".into())), "ToolPre");
        assert_eq!(event_name(&Event::ToolPost("Write".into())), "ToolPost");
        assert_eq!(event_name(&Event::PhaseEntered(PhaseId::Ingest)), "PhaseEntered");
        assert_eq!(event_name(&Event::PhasePassed(PhaseId::Build)), "PhasePassed");
        assert_eq!(event_name(&Event::PhaseFailed(PhaseId::Build, "boom".into())), "PhaseFailed");
        assert_eq!(event_name(&Event::ModelChosen("a-1".into(), ModelTier::T1Cheap)), "ModelChosen");
        assert_eq!(event_name(&Event::TokenSpent("a-1".into(), 10)), "TokenSpent");
        assert_eq!(event_name(&Event::RecallInjected(3)), "RecallInjected");
        assert_eq!(event_name(&Event::NightTick), "NightTick");
        assert_eq!(event_name(&Event::IterateRequest(1, vec!["fb".into()])), "IterateRequest");
    }

    #[test]
    fn hook_serializa_a_json_y_vuelve() {
        let h = Hook::new("h-mission", "UserPromptSubmit", HookPriority::Pre)
            .with_lock()
            .with_description("re-inyecta MISSION.md");
        let json = serde_json::to_string(&h).unwrap();
        let back: Hook = serde_json::from_str(&json).unwrap();
        assert_eq!(back, h);
        assert!(back.lock);
        assert_eq!(back.timeout_ms, 5000);
    }

    #[test]
    fn dispatch_para_evento_sin_hooks_devuelve_plan_vacio() {
        let r = phase_passed_registry();
        let d = Dispatcher::new(&r);
        let res = d.dispatch(&Event::NightTick);
        assert!(res.plan.hooks.is_empty());
        assert!(!res.plan.blocked);
        assert!(res.plan.blocking_hook.is_none());
        assert_eq!(res.plan.event, "NightTick");
    }
}
