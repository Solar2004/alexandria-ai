# PHALANX — Spec del mega-plugin (EL ÚNICO plugin)

> PHALANX = la falange. Un solo plugin que hace todo: skills, hooks, agentes, planes, configuración. El usuario instala UN plugin; ALEXANDRIA (motor Rust) lo ejecuta.

## 1. Estructura de directorios

```
phalanx/
├── config.toml          # TODO el sistema, un archivo
├── hooks/               # catálogo de hooks (un .toml por hook)
│   ├── mission.toml
│   ├── memory-capture.toml
│   ├── governor-classify.toml
│   └── ... (14 total, ver 05 §3)
├── skills/              # 23 skills de agent-skills + night-ops + emil
│   └── <skill>/SKILL.md
├── agents/              # symlinks/labels al registry de agents/
│   └── router.toml      # política: qué agente en qué fase
├── plans/               # planing-with-files: task_plan.md, progress.md, findings.md
├── security.toml        # allowlist de tools por fase
├── bench.toml           # umbrales de performance (ver 10 §4)
└── manifest.md          # declaración: versión, deps, checksum
```

## 2. `config.toml` — el sistema en un archivo

```toml
[phalanx]
name = "phalanx"
version = "0.1.0"
mission = "plan/MISSION.md"          # memoria maestra inyectada por hook mission
compression = "caveman"              # regla global de habla
caveman_level = "ultra"

[engine]
state_dir = "~/.alexandria/state"
event_log = "~/.alexandria/state/events.log"
max_threads = 6
model_mask = true                    # reusa cc-model-mask

[governor]
default_tier = "T2Medium"
warn_at_pct = 80
hard_cap_pct = 100
routes = [
  # routatic = PROVIDER (deepseek-v4-flash). headroom comprime, mask enmascara el modelo.
  { tier = "T1Cheap",  chain = ["http://127.0.0.1:3456"] },                                        # routatic directo
  { tier = "T2Medium", chain = ["http://127.0.0.1:8788", "http://127.0.0.1:3460", "http://127.0.0.1:3456"] },  # headroom→mask→routatic
  { tier = "T3Premium",chain = ["http://127.0.0.1:8788", "http://127.0.0.1:3460", "http://127.0.0.1:3456"] },
]
fallback = "http://127.0.0.1:20128"   # omniroute: SOLO si routatic cae (no es cadena principal)

[memory]
store = "~/.alexandria/state/recalls.jsonl"
max_inject_tokens = 2000
cadence_days = 30

[task]
store = "~/.alexandria/state/tasks.jsonl"
plans_dir = "phalanx/plans"

[harness]
phases = ["Ingest","Spec","Plan","Build","Test","Review","Docs","Ship"]
retries = 2

[gate]
sandbox = true
policy = "strict"                    # warnings = fail

[night]
enabled = true
report = "plan/night-report.md"
commit = true                        # commit atómico tras cada pasada

[mcp]
server_stdio = true
server_sse = { enabled = true, port = 8770 }

[mcp.clients]
# Default = los 5 necesarios (arrancan siempre). El resto opcional (se activan por fase).
default = ["codebase-memory", "code-graph-rag", "notebooklm", "mcp-search", "chrome-devtools"]
optional = ["perplexity", "playwright", "figma", "media", "horario"]
```

**Este archivo ES la configuración de PHALANX.** El motor lo lee y se configura solo. Cambiar `config.toml` cambia el comportamiento del sistema — sin recompilar.

## 3. Skills que incluye

- Los 23 de `agent-skills` (spec, plan, build, test, review, ship, docs, perf, security, etc.).
- `night-ops` (protocolo autónomo).
- `emil` (diseño de ingeniería).
- `planning-with-files` (planes en archivos).
- El router los expone como harness por fase (ver `04-harness-pipeline.md` §5).

## 4. Hooks que materializa

Los 14 de `05-hooks-system.md` §3, cada uno un `.toml` en `phalanx/hooks/`. El hook `phalanx.mission` inyecta `MISSION.md`; `memory.*` auto-recuerda; `governor.*` controla coste; `gate.verify` exige evidencia.

## 5. Ciclo de vida de PHALANX

1. **Instalación** (una vez): `install.sh` copia `phalanx/` al proyecto, compila `alx`, añade hook SessionStart.
2. **Activación** (cada sesión): `alx run` arranca → lee `config.toml` → carga hooks/skills/agents → inyecta memoria → espera.
3. **Operación**: todo automático vía hooks y pipeline. El usuario no ejecuta comandos.
4. **Mejora**: `alx eval` + `alx bench` miden; los fallos generan Recalls; el sistema se ajusta solo.

## 6. PHALANX vs el resto

| Componente | Rol |
|---|---|
| ALEXANDER | la visión/nombre |
| ALEXANDRIA | el motor Rust (crates) |
| **PHALANX** | el plugin único que el usuario instala y ve |
| atg, bridges, MCP externos, planning-with-files | internos del motor, invisibles al usuario final |

## 7. Decisión de diseño

**PHALANX no contiene lógica**: contiene configuración y contenido (skills, hooks, agents, planes). La lógica vive en ALEXANDRIA. Así el plugin es portable, versionable y reemplazable — pero solo hay UN plugin, como pidió Alexander.
