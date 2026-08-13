# ALEXANDRIA — Estado real del motor

> Estado verificado (no "debería funcionar"): tests, comandos e integraciones reales.

## Motor

- **16/16 crates** con lógica (alx-core ... alx-evolve).
- **207 tests verdes, 0 fallos** (`cargo test`) · clippy 0 warnings.
- **20 subcomandos** (status, network, build, run [--real], night, mcp, phalanx, feature, evolve, doctor, cost, agents, spawn, agents-run, agents-show, tui, report, metrics, weekly, iterate).
- Binario `alx` instalado en `~/.local/bin/alx` (PATH), vía `proyecto-final/install.sh`.

## Comandos (verificados en vivo)

| Comando | Estado |
|---|---|
| `alx status` | ✓ estado del motor |
| `alx network` | ✓ red real (headroom 200, mask 502, routatic/omniroute 404) |
| `alx build` | ✓ dogfood, build OK |
| `alx run "X" --real` | ✓ pipeline real: cadena + critic real + must_checks + evolve + ledger |
| `alx night` | ✓ informe nocturno + systemd timer 02:00 activo |
| `alx mcp` | ✓ server JSON-RPC (registrado en ~/.claude.json) |
| `alx phalanx` | ✓ config (13 secciones) + 10 hooks |
| `alx feature "X"` | ✓ dogfood end-to-end (artefacto + verificación build) |
| `alx evolve` | ✓ watcher con persistencia (harnesses/active) |
| `alx doctor` | ✓ indexa 16 crates + 10 hooks + harnesses (27 items) |
| `alx cost` | ✓ cost-report acumulado del ledger persistido |
| `alx agents` | ✓ registry + envelope |
| `alx spawn <a> <t>` | ✓ agente real contra la cadena |
| `alx agents-run "<t>"` | ✓ 3 agentes en paralelo (ciclo 2) |
| `alx tui` | ✓ dashboard ANSI con telemetría (ciclo 2) |
| `alx report` | ✓ reporte markdown completo (ciclo 2) |
| `alx metrics` | ✓ líneas por crate, 7833 total (ciclo 2) |

## Integraciones con el sistema real

- Hook SessionStart → `alx status`.
- Hook Stop → `alx doctor`.
- Hook auto-continue + iterate.trigger → iteración automática (R24, con awaiting_user + target_iter).
- atg → banner ALEXANDRIA + `atg --alx <subcomando>`.
- `alx mcp` registrado como servidor MCP del sistema.
- systemd user `alx-night.timer` (02:00).
- Red real: headroom→mask→routatic→deepseek (cadena verificada).
- Ledger persistido en `state/ledger.jsonl` (coste real por llamada).

## Ciclo 7 — Benchmark campeón + generalidad (verificado en vivo)

- **Config campeona: PLAN-THEN-CODE** — el modelo describe el algoritmo antes de codificar. Harness = describir→codificar→ejecutar→corregir.
- **BigCodeBench sample (N=60)**: directa 9/60 (15%) vs **harness 43/60 (72%) = 4.78x**.
- **HELD-OUT (N=30, problemas nunca usados)**: directa 2/30 (7%) vs **harness 22/30 (73%) = 11x** → robustez confirmada, no es suerte del subset.
- **HumanEval (N=164, familia 2)**: directa 147/164 (89.6%) vs **harness 156/164 (95.1%)** → generalidad confirmada (2 familias).
- **Comandos**: `alx bench-bigcode` (sample o `ALX_BENCH_FILE`), `alx bench-humaneval`, `ALX_BENCH_MAX`, `ALX_BENCH_MODEL`, `ALX_BENCH_URL`.
- **Ruta a Claude real**: `cc-model-mask:3460` (`claude-opus-4-6[1m]`, lento; headroom 502; routatic reescribe a deepseek). fable 5 ausente.
- **Dashboard visual**: `docs/benchmark-viz.html` (5 gráficas Chart.js, tema oscuro, verificado con playwright — screenshot `docs/viz-check.png`).
- **Dashboard detallado**: `docs/benchmark-report.html` (10 secciones, SVGs, referencia publicada + advertencia comparabilidad + métrica agregada).

## Ciclo 6 — Benchmarks (verificado en vivo)

- **Benchmark REAL**: BigCodeBench (ICLR'25), 60 problemas profesionales con unittest reales, en `harnesses/bench/bigcodebench-sample.jsonl` (descargados de HF).
- **Comando**: `alx bench-bigcode` (`ALX_BENCH_MAX` limita runtime).
- **Resultado**: directa 9/60 (15%) vs **harness 34/60 (57%) = ~4x estable** (N=30: 4.25x).
- Harness supera a GPT-4o (~40%) en este subconjunto ejecutable, mismo modelo base.
- 3 experimentos controlados: feedback simple=34/60 (mejor), feedback rich=29/60 (revertido), ensamble pass@k=33/60 (revertido).
- **Veredicto**: techo ~55% es del modelo deepseek-v4-flash; 5x inalcanzable en este benchmark con este modelo. Spec ensamble: `docs/ensamble-spec.md`.

## Diagrama de estado (mermaid)

```mermaid
flowchart LR
    ALX[alx CLI 18 subcomandos] --> ENGINE[Motor Rust 16 crates · 207 tests]
    ENGINE --> RED[headroom→mask→routatic→deepseek]
    ENGINE --> CRITIC[critic real + escalada T3]
    ENGINE --> MEM[memory + must_checks]
    ENGINE --> EVOLVE[harnesses evolutivos + watcher]
    ENGINE --> LEDGER[ledger coste + telemetria por dia]
    HOOKS[Hooks sistema] --> ALX
    ATG[atg --alx] --> ALX
    MCP[alx mcp en ~/.claude.json] --> ALX
    SYSTEMD[alx-night.timer 02:00] --> ALX
```

## Filosofía

- **Evidencia real en cada fase** (tests, comandos, outputs).
- **Iteración es la norma** (R24): el sistema se pule solo.
- **Harnesses evolutivos**: temporales por defecto, permanentes con evidencia, doc-min obligatoria.
- **Barato y rápido**: compresión caveman, tier por dificultad, ledger de coste.
