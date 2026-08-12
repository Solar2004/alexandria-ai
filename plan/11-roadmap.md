# 11 · Roadmap — construcción paso a paso

> De cero a dogfood. Cada fase = tareas verificables. Orden pensado para que el sistema se use desde el primer día (dogfood temprano) y se refine con el tiempo.

## Principio de orden

1. **Primero lo que el sistema necesita para pensar** (core, cli, hooks, memory).
2. **Luego lo que lo hace barato** (governor).
3. **Luego lo que lo hace completo** (task, harness, gate, bench).
4. **Luego lo que lo conecta** (mcp, night, PHALANX).
5. **Luego lo que lo integra y empaqueta** (atg, install, CI, docs).

## FASE 0 — Plan maestro ✅ (2026-08-12)

- [x] MISSION.md (memoria permanente)
- [x] 00-vision, 01-context, 02-architecture (mermaid x20)
- [x] Specs de subsistemas (03–10)
- [x] Este roadmap
- Verificación: todos los archivos en `plan/` listos para leerse en orden.

## FASE 1 — Workspace Rust + `alx-core`

**Objetivo**: que `cargo build` produzca un workspace que compila y cuyos tests pasan.

| # | Tarea | Subtareas | Verificación |
|---|---|---|---|
| 1.1 | Workspace | Cargo.toml raíz (workspace), 12 crates skeleton, `.gitignore` targets | `cargo build` verde |
| 1.2 | Tipos core | `Event`, `Task`, `TaskStatus`, `PhaseId`, `ModelTier`, `Recall`, `Evidence`, `TokenBudget` en `alx-core` | `cargo test -p alx-core` |
| 1.3 | Event bus | canal `tokio::broadcast`, dispatch por prioridad | test: emisor→receptor |
| 1.4 | Store JSONL | append de tasks/events/recalls/budget, replay al arranque | test: write→replay idéntico |
| 1.5 | IDs + reloj | `AlxId` con prefijo, `Instant` helpers | test: unicidad |

## FASE 2 — CLI skeleton + TUI

| # | Tarea | Subtareas | Verificación |
|---|---|---|---|
| 2.1 | Comandos | `clap`: `run`, `task`, `harness`, `hook`, `memory`, `governor`, `gate`, `bench`, `mcp`, `night`, `doctor`, `eval` | `alx --help` listo |
| 2.2 | `alx run` | arranca sesión: carga state, inyecta memoria, espera prompt | `alx run` arranca |
| 2.3 | TUI estado | ratatui: DAG, presupuesto, fase actual | `alx` abre TUI |
| 2.4 | Merge atg | modos free/raw/bare/clean del wrapper en `alx run` | `alx run --free` funciona |

## FASE 3 — `alx-hooks` (sistema nervioso)

| # | Tarea | Subtareas | Verificación |
|---|---|---|---|
| 3.1 | Hook struct | `Hook`, `CommandSpec`, carga desde TOML | test parse |
| 3.2 | Dispatcher | evento→cadena, orden Pre/Async/Post | test: orden exacto |
| 3.3 | Timeout/lock/retry | matar hook colgado, lock aborta, retry best-effort | test: hook lento no bloquea |
| 3.4 | Migrar hooks heredados | envolver `.claude/hooks/*.sh` como CommandSpec | test: dispara el script |
| 3.5 | Catálogo | 14 hooks de 05: mission, memory.*, governor.*, gate.*, etc. | `alx hook list` muestra 14 |
| 3.6 | event.log | todo evento registrado con metadata | test: log crece |

## FASE 4 — `alx-memory` (auto-recordarse)

| # | Tarea | Subtareas | Verificación |
|---|---|---|---|
| 4.1 | Recall store | `recalls.jsonl`, tags, weight | test roundtrip |
| 4.2 | Compresión caveman | reglas: quitar artículos/filler/hedging, fragmentos | test: 100 tokens → <40 |
| 4.3 | Dedup + weight | similitud → merge; acierto → weight++; caducidad | test dedup |
| 4.4 | Inyección | recalls top-N en envelope, budget de memoria | test: inyección ≤ N tokens |
| 4.5 | Hooks memory.* | capture/inject/summary/commit | test flujo completo |
| 4.6 | Integrar `.remember/` | leer now.md/today-*.md como recalls fuente | test: parse |

## FASE 5 — `alx-governor` (barato y rápido)

| # | Tarea | Subtareas | Verificación |
|---|---|---|---|
| 5.1 | Clasificador | señales de 09 → score → tier | test corpus etiquetado |
| 5.2 | Routing | `/readyz` proxies, fallback headroom→routatic→omniroute | test: proxy caído → fallback |
| 5.3 | Presupuestos | por tarea, warn 80%, cap 100% | test límites |
| 5.4 | Ledger | `budget.ledger.jsonl` | test: gasto registrado |
| 5.5 | Cost-report | agregación por tarea/fase/sesión | test: números cuadran |
| 5.6 | Goal engine | objetivo de sesión → presupuestos por tarea | test: suma ≤ objetivo |

## FASE 6 — `alx-task` (DAG)

| # | Tarea | Subtareas | Verificación |
|---|---|---|---|
| 6.1 | DAG + estados | transiciones 08 §1, guardas | test: transición inválida error |
| 6.2 | Persistencia | `tasks.jsonl`, `dag.dot`, replay | test proptest: replay idéntico |
| 6.3 | CLI task | create/list/show/deps/update/retry/skip/plan-from | comandos reales |
| 6.4 | planning-with-files | `plan-from goal.md` → `task_plan.md`; `show` → `progress.md` | test: archivos escritos |
| 6.5 | Decomposition engine | micro-tareas atómicas con `assert` + `done_when`; tarea grande → DAG de micro-tareas | test: tarea con 2 asserts → 2 micro-tareas |

## FASE 7 — `alx-harness` (pipeline)

| # | Tarea | Subtareas | Verificación |
|---|---|---|---|
| 7.1 | Fases + contratos | 8 fases, `Phase`, `Artifact`, `GateSpec` | test: contrato parse |
| 7.2 | Runner | pick→ejecutar agente→gate→evidencia→avanzar | test: fase completa con fixture |
| 7.3 | Reanudable | estado por fase, skip verificado | test: crash → resume |
| 7.4 | Loop de mejora | fallo repetido → Recall emitido | test: N fallos → recall |

## FASE 8 — `alx-gate` (verificación)

| # | Tarea | Subtareas | Verificación |
|---|---|---|---|
| 8.1 | Runner comandos | sandbox, timeout, captura stdout | test: exit_code capturado |
| 8.2 | Evidencia | `Evidence` anexado al Task | test: no vacío |
| 8.3 | LSP discovery | detectar servidores LSP del proyecto (Rust/TS/Py) | test: discovery en fixture |
| 8.4 | Lint runner | cargo clippy / eslint con política strict | test: warnings → fail |

## FASE 9 — `alx-bench` (performance matemático)

| # | Tarea | Subtareas | Verificación |
|---|---|---|---|
| 9.1 | Métricas | runtime, memoria, complejidad ciclomática | test: métricas reportadas |
| 9.2 | Umbrales | `bench.toml` por fase | test: exceder → fail |
| 9.3 | Diff bench | build time, binario, tests antes/después; regresión >10% bloquea | test: regresión detectada |

## FASE 10 — `alx-mcp` (bus)

| # | Tarea | Subtareas | Verificación |
|---|---|---|---|
| 10.1 | Server stdio | protocolo MCP, 9 namespaces de tools | test: handshake + list_tools |
| 10.2 | Server SSE | modo remoto | test: conexión SSE |
| 10.3 | Client | conectar a codebase-memory, horario, etc. | test: discovery tools |
| 10.4 | Catálogo central | todas las tools registradas | test: sin duplicados |
| 10.5 | Allowlist | `security.toml` por fase | test: tool prohibida → rechazo |

## FASE 11 — `alx-night` (autónomo)

| # | Tarea | Subtareas | Verificación |
|---|---|---|---|
| 11.1 | Cron | `NightTick`, scheduling | test: tick dispara |
| 11.2 | Informe | resumen de lo hecho/coste/pendiente → `plan/night-report.md` | test: informe generado |
| 11.3 | Commit atómico | git2, mensaje claro | test: commit creado en repo fixture |

## FASE 12 — PHALANX (el plugin único)

| # | Tarea | Subtareas | Verificación |
|---|---|---|---|
| 12.1 | Estructura | directorios `phalanx/{skills,hooks,agents,plans,config}` | `alx phalanx status` |
| 12.2 | config.toml | TODO el sistema en un archivo (ver `plugin-phalanx.md`) | parse + validación |
| 12.3 | hooks/*.toml | 14 hooks de 05 materializados | `alx hook list` |
| 12.4 | router.toml | política de agentes por fase | test: route correcta |
| 12.5 | security.toml | allowlists | test: tool denegada |
| 12.6 | bench.toml | umbrales | test: gate respeta |
| 12.7 | skills | montar los 23 de agent-skills | `alx eval run all` verde |
| 12.8 | agents | validar los 420 con `alx doctor agents` | 0 errores críticos |

## FASE 13 — Integración total

| # | Tarea | Subtareas | Verificación |
|---|---|---|---|
| 13.1 | atg → alx | `bin/atg` delega en `alx run` (modos) | `atg --free` usa alx |
| 13.2 | install.sh | instala `alx` + PHALANX + hook SessionStart | re-run idempotente |
| 13.3 | Bridges | headroom/routatic/omniroute probados por governor | test: ruta real |
| 13.4 | CI | `.github/workflows/alx.yml` 4 gates | PR con los 4 verdes |
| 13.5 | alx doctor | valida agents/hooks/skills/config | 0 warnings críticos |

## FASE 14 — Dogfood (prueba de fuego)

| # | Tarea | Subtareas | Verificación |
|---|---|---|---|
| 14.1 | Feature real | alx construye una feature end-to-end del repo | Ship verde |
| 14.2 | Medir coste | cost-report vs baseline manual | ≥60% menos |
| 14.3 | Ajustar governor | calibrar pesos, presupuestos con datos reales | re-bench verde |

## FASE 15 — Docs + packaging

| # | Tarea | Subtareas | Verificación |
|---|---|---|---|
| 15.1 | Docs | README, CLI reference, QUICKSTART | enlaces válidos |
| 15.2 | Release | binario release, instalador final | instala limpio |
| 15.3 | Meta | MISSION.md actualizado, 13-glosario completo | — |

## FASE 16 — `alx-critic` (auto-crítica)

**Objetivo**: la AI se critica sola, itera hasta pulir, aprende de sus errores.

| # | Tarea | Subtareas | Verificación |
|---|---|---|---|
| 16.1 | Critic loop | critic T1 tras cada fase; feedback severidad; re-critic | test: fase rechazada → reabierta |
| 16.2 | Criterios | `critics.toml`: reglas genéricas + por fase + must_check | test: check violado → blocker |
| 16.3 | Reglas deterministas | secrets grep, clippy, cobertura, complejidad | test: secreto detectado |
| 16.4 | critic.learn | error corregido → must_check futuro | test: fallo → check aprendido |
| 16.5 | Escalada | 3 iter critic barato → T3 con historial | test: escalada a la 4ª |
| 16.6 | critic-report | `state/critics/<task>/<phase>.jsonl` | test: report existe |

## FASE 17 — `alx-audit` (ecosistema dedup)

**Objetivo**: indexar todo (skills/agents/plugins/hooks/MCP), dedup, doctor.

| # | Tarea | Subtareas | Verificación |
|---|---|---|---|
| 17.1 | Indexer | skills globales (86) + repo (24+3) → registry | test: total indexado |
| 17.2 | Agent dedup | 842 agentes → registry único por slug+descripción | test: code-review → 1 |
| 17.3 | Skill dedup | night-ops/fable/emil/planning → 1 fuente | test: sin duplicados |
| 17.4 | Hook catalog | cbm-*, HCOM/CENTAURY, .orca, heredados → 20 .toml | `alx hook list` = 20 |
| 17.5 | MCP registry | 10 servers registrados como clientes | test: discovery ok |
| 17.6 | doctor | valida todo; informe de salud | `alx doctor` → 0 críticos |

## FASE 18 — `alx-evolve` (harness evolutivo / self-evolución)

**Objetivo**: la AI crea harnesses en tiempo real. Temporal por defecto, permanente con evidencia, doc-min obligatoria, watcher de objetivos.

| # | Tarea | Subtareas | Verificación |
|---|---|---|---|
| 18.1 | Harness struct | `Harness`, `HarnessKind`, `HarnessState`, `Trigger`, serialización TOML | test: parse/roundtrip |
| 18.2 | Registry | `harnesses/active/*.toml` + `index.toml`; archive | test: register → index |
| 18.3 | evolve.detect | detecta candidatos en PostToolUse (regla/patrón/objetivo) | test: patrón → candidato |
| 18.4 | docmin.verify | doc-min obligatoria en cada Edit/Write; complementa si falta | test: archivo sin doc → recall |
| 18.5 | Watcher | vigila temporales; cumple objetivo → Retired; zombie → retirar | test: objetivo cumplido → retired |
| 18.6 | Promoción | temporal que sirvió N veces → Permanent (con aval del critic) | test: N usos → promoted |
| 18.7 | refine | falso positivo ×3 → ajusta regla del harness | test: 3 rechazos erróneos → refine |

## Criterio de "fase completa"

Cada fase termina con: **tests verdes + comando real ejecutado + evidencia capturada**. Sin eso, la fase NO está completa y se revisa antes de avanzar.

## Dependencias entre fases

```
F1 → F2 → F3 → F4 ─┐
                ├→ F5 → F6 → F7 → F8 → F9 ─┐
F3 ──────────────┘                          ├→ F10 → F11 → F12 → F13 → F14 → F15
                                            │
                                            └→ F16 (critic) → F17 (audit) → F18 (evolve)
```

F3 (hooks) desbloquea F4 (memory) y F10 (mcp). F5 (governor) alimenta F6–F9. F12 (PHALANX) solo se completa cuando F3–F9 existen. F16 (critic) se apoya en F6/F7/F3; F17 (audit) es independiente y puede correr en paralelo con F10. F18 (evolve) se apoya en F16 (critic) y F17 (audit).
