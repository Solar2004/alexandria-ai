# ALEXANDRIA — Estado real del motor

> Estado verificado (no "debería funcionar"): tests, comandos e integraciones reales.

## Ciclo 11 — harness de benchmark con detección de estancamiento + documentación completa (2026-08-26)

- **R28 en el harness de benchmark**: si el MISMO test falla 2x seguidas, el
  modelo DESCARTa el enfoque y reescribe desde cero con algoritmo distinto
  (antes "corrige" repetía el mismo error). Intentos 4→6 en las 3 familias.
- **Medido en vivo (N=8, deepseek-v4-flash, unittest REAL)**:
  directa 2/8 (25%) vs **harness 7/8 (87.5%) = 3.5x** — recupera 5/6 fallos
  de la directa. Evidencia: `harnesses/bench/results/bigcode8-ciclo11-stall-detect.md`.
  N=20 completo pendiente para el ratio oficial.
- **README documentado de verdad**: tabla de 38 comandos (antes 9), features
  de los ciclos 9-10 (init/research/polish/skills-fetch/harness-new), env vars,
  resultado N=8 en la tabla de benchmarks. Quickstart con sección de benchmarks.
- Benchmark chart regenerado con datos verificados; badge de 212 tests.
- 212 tests · clippy 0 warnings.

## Ciclo 10 — experto por proyecto + investigación profunda (2026-08-25, sesión continua 2)

- **`.alexandria/` por proyecto**: `alx init` crea registry/rúbricas/skills/
  lecciones/config propios; el motor resuelve automático proyecto-vs-global.
  El harness se ADAPTA a cada proyecto y aprende con él.
- **Investigación profunda (plan/17)**: `alx research <pregunta>` abre el
  protocolo de 7 pasos (mecanismo → iceberg → simulaciones guardadas → frenos
  → evidencia → síntesis). `alx research-check` es compuerta dura (exit 1 si
  superficial) y `research-guard.sh` BLOQUEA cerrar la sesión con research
  a medias. La IA ya no puede quedarse en lo obvio.
- **Pulido dosificado**: `alx polish` evalúa con critic real contra rúbrica,
  mejora con LLM, y DECIDE cuántas rondas viendo la mejora (meseta → parada).
  Verificado en vivo: util.js 0.70→1.00 APROBADO en 2 rondas.
- **Skills del experto**: `alx skills-fetch --search` busca en GitHub POR
  ESTRELLAS; instalar = un comando. Catálogo curado incluido.
- **Recurrencia → harnesses**: `alx patterns [--apply]` mina métricas de hooks.
- **Cero modelos hardcodeados**: modelo_real_activo() lee en vivo el config de
  routatic (cambiar modelo = cambia TODO el motor). Env ALX_MODEL = override.
- **Fix crítico de benchmarks**: run_command trunca stdout a 4000 chars ->
  respuestas largas cortadas -> 0/6 falso. Volcado a fichero; harness vuelve
  a ganar (N=4: 1/4 vs 3/4). Muestra N=20 en curso.
- 212 tests · clippy 0 · tsc hooks 0.

## Ciclo 9 — auditoría autónoma: sistemas desconectados reparados (2026-08-25, sesión continua)

- **Failover de modelos**: si el modelo activo 500s arriba (pasó con muse y con
  ox-alpha-free), el gateway prueba candidatos (`ROUTA_FALLBACK_MODELS`) y
  routatic usa sus fallbacks; `routa auto` encuentra un modelo vivo y lo activa.
  Telemetría: failovers/último_modelo en `/stats`.
- **Sistema de skills REPARADO**: lib/ y providers/ nunca existieron — 3 hooks
  registrados morían en silencio. Reconstruidos: clasificación IA real vía la
  cadena local (verificado: "refactor+migración" → safe-refactor + migration),
  embeddings offline por hashing, vector-store sqlite, degradación elegante.
- **Alexandria se instala sus hooks**: harnesses/hooks es la fuente canónica
  (con lib/providers) y `alx setup` sincroniza + npm install automático.
- **Auto-mejora completa (R20-R23)**: faltaba el paso CREAR → nuevos comandos
  `alx harness-new/harness-list/harness-use`; SessionStart cicla `alx evolve`
  automáticamente cada sesión.
- **TUI**: `alx tui` = dashboard ratatui vivo (red+gobernador+harnesses+bucle).
- Rutas muertas proyecto-final/ arregladas (night-run, bench-all, auto-continue).
- ccmodel del statusline lee el config de routatic (ya no "modelo?").
- **Registry de agentes conectado**: agents_show/render_agents leían FUERA
  del repo ('../../../../'): registry vacío siempre. Ruta corregida + búsqueda
  por slug (421 agentes reales ahora accesibles con `alx agents-show <nombre>`).
- **alx spawn verificado en vivo**: agente real contra la cadena responde.
- Informe nocturno incluye salud de la cadena (routa status + doctor).
- 212 tests OK · clippy 0 · tsc hooks 0 errores.

## Ciclo 8 — routa: cadena v2 sin muse-stack + gobernador de entropía (2026-08-25)

- **Cadena nueva**: `CC → headroom :8788 → routa-gateway :3460 → routatic :3456 → opencode-go`.
  `cc-model-mask` y `cc-openai-bridge` (muse-stack) RETIRADOS del sistema; el
  gateway los sustituye con un solo servicio (`scripts/routa/`, `./install.sh`).
- **Máscara [1m] viva**: CC ve `claude-opus-4-6[1m]`; el modelo REAL se lee EN
  VIVO de `~/.config/routatic-proxy/config.json` — cambiar modelo no toca el gateway.
- **CLI `routa`** (~/.local/bin/routa): `show|models|use|status|doctor|restart|key|logs`.
  `routa use <model>` cambia slots+fallbacks+aliases atómicamente y reinicia routatic.
- **Gobernador de entropía** (en gateway y en `alx-governor::entropy`):
  techo global de concurrencia (3), cola con timeout, backoff exponencial
  JITTERIZADO, circuit-breaker y cooldown compartido por fichero. Cura del
  "demasiadas conexiones tumban la red" (502 al segundo mensaje).
- **Probes GET**: `alx network` ya no gasta generaciones (era ~308 tokens/ping).
- **Claves sticky**: `oc-go-cc-wrapper` v2 NO rota en cada restart; rotación
  consciente con `routa key next`. Clave 2 comentada (sin créditos).
- **Hallazgo upstream**: `muse-spark-1.2-contributor` devolvía HTTP 500 desde
  opencode-go (verificado directo); default movido a `deepseek-v4-flash`
  (funciona). Para volver a muse cuando Meta lo arregle: `routa use muse-spark-1.2-contributor`.
- Verificado en vivo: multi-turno OK, streaming SSE OK, OpenAI-compat :3461 OK,
  `routa doctor` 5/5 + generación real 200.
- Archivo de muse-stack: `~/.local/share/atg-archive/muse-stack-removed-2026-08-25.tar.gz`.

## Motor

- **16/16 crates** con lógica (alx-core ... alx-evolve).
- **207 tests verdes, 0 fallos** (`cargo test`) · clippy 0 warnings.
- **20 subcomandos** (status, network, build, run [--real], night, mcp, phalanx, feature, evolve, doctor, cost, agents, spawn, agents-run, agents-show, tui, report, metrics, weekly, iterate).
- Binario `alx` instalado en `~/.local/bin/alx` (PATH), vía `proyecto-final/install.sh`.

## Comandos (verificados en vivo)

| Comando | Estado |
|---|---|
| `alx status` | ✓ estado del motor |
| `alx network` | ✓ red real con probes GET sin coste (gateway/headroom/routatic/omniroute) |
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
- Red real: headroom→gateway→routatic (cadena verificada, modelo real en config de routatic).
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

### Benchmark ciclo 10 (2026-08-25, verificado en vivo)

- BigCodeBench sample N=20, unittest REAL, cadena actual
  (deepseek-v4-flash con failover automático a minimax durante la carrera):
  - DIRECTA: 3/20 = 15%
  - HARNESS (plan-then-code + feedback): 9/20 = 45%
  - Multiplicador: **3.0x** — consistente con el histórico (~4x) pese al
    cambio de modelo. El techo absoluto es el modelo; el multiplicador es del
    sistema.
- Fix previo imprescindible: stdout truncado a 4000 chars cortaba respuestas
  largas -> 0/20 falso. Volcado a fichero lo arregló (commit ea8b4bc).
- Durante la carrera el failover actuó solo (deepseek caído → minimax sirvió):
  el gobernador sostuvo 40+ minutos de generación continua sin saturar.

## Diagrama de estado (mermaid)

```mermaid
flowchart LR
    ALX[alx CLI 18 subcomandos] --> ENGINE[Motor Rust 16 crates · 212 tests]
    ENGINE --> RED[headroom→gateway→routatic + entropia]
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
