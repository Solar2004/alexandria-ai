# proyecto-final — ALEXANDRIA

> El proyecto definitivo. Motor Rust + documentación viva + harnesses evolutivos.
> La AI se construye a sí misma aquí: código, docs mínimas y harnesses temporales/permanentes.

## Estructura

```
proyecto-final/
├── alexandria/     # workspace Rust — 16 crates (alx-core ... alx-evolve)
├── docs/           # documentación viva generada mientras se construye
└── harnesses/      # harnesses evolutivos (active/*.toml + archive/)
```

## Estado

- **Motor**: 16 crates, 19 tests verdes (`cargo build && cargo test`).
- **Plan maestro**: `plan/` en la raíz del repo (00-vision → 16-evolve, 57 iteraciones de arquitectura).
- **Misión**: `plan/MISSION.md` (memoria maestra, auto-releída cada sesión).

## Cómo correr

```bash
cd proyecto-final/alexandria
cargo build
cargo test
```

## Comandos `alx`

| Comando | Qué hace |
|---|---|
| `alx status` | Estado del motor (tareas, hooks, recalls, agentes) |
| `alx network` | Verifica la red real (headroom→mask→routatic, fallback omniroute) |
| `alx build` | Dogfood: verifica su propio build (cargo build) |
| `alx run "título"` | Pipeline demo (task→DAG→decompose→harness, gates echo) |
| `alx run "título" --real` | Pipeline REAL: cadena LLM + critic real + must_checks + evolve + ledger |
| `alx night` | Informe nocturno desde el DAG |
| `alx mcp` | Server MCP JSON-RPC por stdio |
| `alx phalanx` | Estado del plugin PHALANX (config + hooks) |
| `alx feature "X" [--real]` | Dogfood: ejecuta pipeline y escribe artefacto en docs/features/ |
| `alx evolve` | Ciclo watcher de harnesses con persistencia |
| `alx doctor` | Indexa y valida el ecosistema (crates, hooks, harnesses) |
| `alx cost` | Cost-report desde el ledger persistido + telemetría por día |
| `alx agents` | Agentes del registry + envelope de spawn |
| `alx spawn <a> <t>` | Ejecuta un agente real contra la cadena (text directo) |
| `alx agents-run "<t>"` | 3 agentes en paralelo sobre una tarea |
| `alx tui` | Dashboard ANSI del motor (estado, red, coste, métricas) |
| `alx report` | Reporte markdown completo |
| `alx metrics` | Líneas de código por crate |
| `alx weekly` | Resumen semanal (coste, harnesses, métricas) |

Instalar: `./proyecto-final/install.sh` → `~/.local/bin/alx`.
Autónomo nocturno: `proyecto-final/scripts/night-run.sh` (cron/systemd 02:00).
