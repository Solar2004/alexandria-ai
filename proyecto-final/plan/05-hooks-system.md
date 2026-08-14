# 05 · Hooks System — `alx-hooks`

> El sistema nervioso. **Cada evento dispara una cadena de hooks.** Los hooks capturan el conocimiento que el dev "siempre repite" para que la AI se auto-recuerde sola. Nada de comandos manuales.

## 1. Modelo de hook

```rust
struct Hook {
    id: AlxId,                    // "h-<slug>"
    event: EventKind,             // qué evento lo dispara
    priority: HookPriority,       // Pre (bloqueante) | Async | Post
    command: CommandSpec,         // binario + args (Rust nativo o script heredado)
    timeout_ms: u64,              // 5000 default
    lock: bool,                   // true = aborta el pipeline si falla
    retry: u8,                    // reintentos best-effort
    enabled: bool,
    description: String,          // qué resuelve → documentación viva
}

enum EventKind {
    SessionStart, SessionStop,
    UserPromptSubmit, PreCompact,
    PreToolUse, PostToolUse,
    Stop, NightTick,
    PhaseEntered, PhasePassed, PhaseFailed,
    RecallNeeded,
}
```

## 2. Mapeo Claude Code ↔ eventos internos

| Hook Claude Code | Evento alx | Hooks que corren |
|---|---|---|
| `SessionStart` | SessionStart | memory.inject (auto-recalls), governor.load, skill-catalog.load |
| `UserPromptSubmit` | UserPromptSubmit | memory.hint, governor.classify, phalanx.mission (re-inyecta MISSION.md), task.materialize |
| `PreToolUse` | ToolPre | gate.sandbox-check, error-prevention, token.guard |
| `PostToolUse` | ToolPost | memory.capture (aprendizaje), task.progress, bench.sample |
| `Stop` | SessionStop | memory.commit, docs.autoupdate, night.maybe-schedule |
| `PreCompact` | PreCompact | memory.summary (resumen de contexto para no perder nada) |
| `NightTick` | NightTick | harness.run-next, night.commit, night.report |

## 3. Catálogo de hooks (el "auto-recordarse")

> Regla de oro: **cada cosa que el dev repite a la AI es un hook que falta.**

| Hook | Evento | Qué hace | Lock |
|---|---|---|---|
| `phalanx.mission` | UserPromptSubmit | Re-inyecta `MISSION.md` + reglas del proyecto (caveman, calidad, evidencia) | sí |
| `memory.inject` | SessionStart | Inyecta recalls del proyecto (auto-memoria) | no |
| `memory.capture` | PostToolUse | Si la tool dejó aprendizaje (error, fix, patrón), lo comprime y guarda como Recall | no |
| `memory.summary` | PreCompact | Escribe resumen de contexto pre-compactación (`.remember/tmp`) | no |
| `memory.commit` | SessionStop | Convierte la sesión en `today-*.md`, actualiza `now.md` | no |
| `governor.classify` | UserPromptSubmit | Clasifica dificultad → tier de modelo → ruta | no |
| `governor.budget-check` | PostToolUse | Decrementa presupuesto; >80% warn; >100% aborta fase | sí |
| `skill.activation` | UserPromptSubmit | Detecta skill aplicable y lo anuncia (automatiza el "usa la skill") | no |
| `gate.verify` | PhasePassed | Corre compuerta de la fase, captura evidencia | sí |
| `error.prevention` | PreToolUse | Si tool = Edit/Write, pre-valida (no editar sin leer, no tocar fuera del repo) | sí |
| `docs.autoupdate` | Stop | Actualiza dev-docs / changelog con lo hecho | no |
| `night.maybe-schedule` | Stop | Si hay tareas pendientes y es noche, agenda `alx-night` | no |
| `bench.sample` | PostToolUse | Mide métricas de la operación (tiempo, tokens, exit) | no |
| `iterate.trigger` | Stop / TaskDone | Fuerza iteración: lee/actualiza IterationState, emite IterateRequest con feedback acumulado | no |
| `atg.compat` | SessionStart | Detecta wrapper `atg` y embebe sus modos de red | no |

## 4. Ciclo de vida de un evento

```
Evento llega al bus
  → alx-hooks resuelve cadena (evento + prioridad, orden estable)
  → corre hooks Pre: cualquiera puede abortar (lock) con razón
  → corre hooks Async: best-effort, en paralelo, con timeout
  → corre hooks Post: registro, memoria, métricas
  → si un hook crashea: log + retry (si retry>0) + seguir (si !lock)
  → evento queda en event.log
```

## 5. Auto-memoria en profundidad (R6)

El flujo que elimina la repetición:

1. **Captura**: `memory.capture` en PostToolUse. Patrón: cuando una tool devuelve error o éxito inesperado, se extrae una frase caveman del aprendizaje.
   - Ejemplo: la AI arregla un bug de auth → Recall: *"auth token expiry check usa `<` no `<=`"*.
2. **Compresión**: `alx-memory` comprime (reglas caveman) y deduplica por similitud. Almacena con `weight`.
3. **Inyección**: `memory.inject` en SessionStart + `phalanx.mission` en cada prompt. Los recalls con `weight` alto se inyectan primero. Máx N tokens por inyección (budget de memoria).
4. **Refuerzo**: si un Recall ayuda a evitar un error, `weight++`. Si nunca se usa, caduca.

Resultado: el conocimiento que antes se repetía en cada sesión **fluye solo**.

## 6. Decisiones

- **Hooks como datos**: los hooks viven en `phalanx/hooks/*.toml` (config), no hardcodeados. ALEXANDRIA los carga y ejecuta.
- **Compat con hooks heredados**: los hooks bash actuales de `.claude/hooks/` se envuelven como `CommandSpec` externo; no se reescriben en la fase 1.
- **Lock solo donde importa**: mission, gate, error-prevention, budget. El resto best-effort — un hook de memoria no debe poder tumbar una sesión.
- **Todo observable**: cada hook escribe en `event.log`; el informe nocturno lista qué hook corrió, cuánto tardó, qué costó.
