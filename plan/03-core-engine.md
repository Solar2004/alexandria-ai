# 03 · Core Engine — `alx-core`

> El hueso de ALEXANDRIA: tipos, estado, event bus, reloj. Todo crate depende de aquí; aquí no depende de nadie.

## 1. Tipos fundamentales

```rust
// Identidad
type AlxId = String;            // prefijo por entidad: "t-<uuid>" tarea, "a-<slug>" agente, "h-<uuid>" hook

// Eventos — todo el sistema es reactivo
enum Event {
    Prompt(UserPrompt),          // el usuario (o night) puso un objetivo
    ToolPre(ToolInvocation),     // antes de una herramienta
    ToolPost(ToolResult),        // después de una herramienta
    PhaseEntered(PhaseId),
    PhasePassed(PhaseId, Evidence),
    PhaseFailed(PhaseId, Reason),
    TestRun(TaskId, TestOutcome),
    ModelChosen(TaskId, ModelTier),
    TokenSpent(TaskId, Tokens),
    SessionStart(SessionMeta),
    SessionStop(SessionMeta),
    RecallInjected(Vec<Recall>),
    NightTick(Instant),
}

// Tareas — DAG
struct Task {
    id: AlxId,
    title: String,
    status: TaskStatus,          // Pending | Ready | InProgress | Blocked | Done | Failed | Skipped
    depends_on: Vec<AlxId>,      // aristas del DAG
    phase: PhaseId,
    budget: TokenBudget,
    evidence: Vec<Evidence>,     // outputs verificados
    model_tier: ModelTier,       // elegido por governor
    created: Instant,
    updated: Instant,
}

// Fases del pipeline
enum PhaseId { Ingest, Spec, Plan, Build, Test, Review, Docs, Ship }

// Tier de modelo — la moneda del gobernador
enum ModelTier { T1Cheap, T2Medium, T3Premium }   // haiku/deepseek · sonnet · opus

// Recuerdo de memoria (auto-recall)
struct Recall {
    id: AlxId,
    text: String,                // comprimido en caveman
    source: RecallSource,        // Session | Tool | Project | User
    tags: Vec<String>,
    weight: u32,                 // frecuencia de acierto → prioridad de inyección
    created: Instant,
}

// Evidencia — la moneda de la verificación
struct Evidence {
    kind: EvidenceKind,          // BuildOutput | TestSummary | LintReport | BenchReport | CommandOutput
    command: String,
    exit_code: i32,
    stdout_head: String,         // primer N bytes (no inflar memoria)
    passed: bool,
    metrics: HashMap<String, f64>,
}
```

## 2. Event bus

- Un canal `tokio::broadcast` central. Cualquier crate emite; `alx-hooks` recibe y despacha a la cadena de hooks registrada para ese evento.
- **Orden garantizado por prioridad**: hook `Pre` (bloqueante, puede abortar) → hook `Async` (best-effort, no bloquea) → hook `Post`.
- **Timeout y lock**: cada hook tiene timeout configurable; un hook que se cuelga se mata y el evento sigue.
- **Log de eventos**: todo evento va al `event.log` (JSONL) — fuente de verdad para informes y auditoría.

## 3. Estado global (store)

- Estado vivo en memoria (Rust structs) + **snapshot JSONL** en disco: `~/.alexandria/state/`.
  - `tasks.jsonl`, `events.log`, `recalls.jsonl`, `budget.ledger.jsonl`, `phases/<phase>.jsonl`.
- Arranque: carga snapshots, reconstruye DAG de tareas, inyecta recalls pendientes.
- **Crash-safe**: escrituras append-only; un crash deja el estado en el último evento completo.

## 4. Reloj y scheduling

- Reloj monotónico (`Instant`) para timeouts y medición.
- `alx-night` usa `cron` para disparar `NightTick` y arrancar sesiones autónomas.

## 5. Presupuestos (tokens)

```rust
struct TokenBudget {
    total: u32,          // techo absoluto para la tarea
    spent: u32,          // acumulado (lo reporta governor)
    warn_at_pct: u8,     // 80
    hard_cap_pct: u8,    // 100 → aborta la fase
}
```

El presupuesto lo asigna `alx-governor` al crear la tarea y se decrementa en cada `TokenSpent`.

## 6. Decisiones

- **JSONL primero, SQLite después**: para la fase 1 basta JSONL (append-only, portable, grepeable). Migrar a SQLite solo si el volumen de tareas lo exige.
- **IDs con prefijo**: grep y debugging inmediatos (`t-`, `a-`, `h-`).
- **Sin dependencia de UI en core**: core es headless; `alx-cli` es quien pinta.
