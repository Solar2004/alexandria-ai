# ALEXANDRIA — Índice de crates

> Los 16 crates del motor. Doc-min: cada crate tiene doc de cabecera en `src/lib.rs`.

| Crate | Qué hace | Estado |
|---|---|---|
| alx-core | Tipos, event bus, store JSONL, estado | ✅ |
| alx-hooks | Hook, HookPriority, Registry, Dispatcher, DispatchPlan | ✅ |
| alx-memory | compress caveman, RecallStore (dedup/reinforce/inject/prune) | ✅ |
| alx-governor | classify→tier, router chains, budget, ledger de coste | ✅ |
| alx-task | TaskGraph (transitions/ready/blocked), decompose micro-tareas | ✅ |
| alx-harness | Phases (8), GateSpec, Pipeline runner con retries | ✅ |
| alx-gate | run_command real (timeout+kill), verify build/test/lint, LSP discovery | ✅ |
| alx-bench | Thresholds, Metrics, check, regression | ✅ |
| alx-critic | IterationState, critic real (cadena), parse_verdict + fallback, escalate T3, learn | ✅ |
| alx-audit | AuditIndex dedup, Doctor, doctor_report | ✅ |
| alx-night | NightSchedule, NightReport, run_cycle | ✅ |
| alx-mcp | ToolCatalog, server JSON-RPC stdio, client registry | ✅ |
| alx-agents | AgentSpec frontmatter, Registry, Router, build_envelope | ✅ |
| alx-cli | Binario `alx`: 16 subcomandos (TUI, report, spawn, run --real...) | ✅ |
| alx-lib | Fachada Alexandria + re-exports | ✅ |
| alx-evolve | Harness evolutivo: detect, add_candidate, save/load, watcher_cycle | ✅ |

**Regla doc-min**: todo archivo de código lleva doc de cabecera. Verificado por el doctor.
