# 02 · Arquitectura — ALEXANDRIA

> Arquitectura final (iteración 20 de 20). El log de evolución de las 20 iteraciones está en `iterations/architecture-iterations.md`.

## 1. Principio de capas

```
┌──────────────────────────────────────────────────────────┐
│  USUARIO  (terminal, `alx`, `atg`)                       │
├──────────────────────────────────────────────────────────┤
│  PHALANX  (mega-plugin: skills + hooks + agentes + plan) │
├──────────────────────────────────────────────────────────┤
│  ALEXANDRIA  (motor Rust — crates)                       │
│   hooks · memory · governor · harness · task · gate      │
├──────────────────────────────────────────────────────────┤
│  BUS MCP  (server propio + clientes a servidores ajenos) │
├──────────────────────────────────────────────────────────┤
│  HOSTS  (Claude Code, Codex, headroom, routatic, Omni)   │
│  SERVS  (codebase-memory, figma, playwright, perplexity…)│
└──────────────────────────────────────────────────────────┘
```

Nada salta capas. El motor lo ve todo; el usuario solo ve PHALANX.

## 2. Mermaid — arquitectura final

```mermaid
flowchart TB
    subgraph U["USUARIO"]
        U1["alx (CLI binario)"]
        U2["atg (wrapper legacy)"]
        U3["hooks Claude Code"]
    end

    subgraph P["PHALANX · Mega-plugin (EL UNICO)"]
        direction LR
        P1["skills/ (23+ agent-skills)"]
        P2["hooks/ (catálogo eventos)"]
        P3["agents/ (registro 420+)"]
        P4["plans/ (planning-with-files)"]
        P5["config.toml (PHALANX config)"]
    end

    subgraph A["ALEXANDRIA · Motor Rust (workspace)"]
        direction TB
        C1["alx-core<br/>tipos · estado · event bus · clock"]
        C2["alx-hooks<br/>engine de eventos · dispatch · timeout"]
        C3["alx-memory<br/>auto-recalls · session · proyecto"]
        C4["alx-governor<br/>routing modelos · compresión · budget · objetivos"]
        C5["alx-task<br/>DAG tareas · estados · persistencia"]
        C6["alx-harness<br/>pipeline fases · contratos · compuertas"]
        C7["alx-gate<br/>verificación · LSP auto · lint · test runner"]
        C8["alx-bench<br/>métricas perf · umbrales · diff check"]
        C9["alx-night<br/>scheduler autónomo · cron"]
    end

    subgraph M["BUS MCP"]
        M1["alx-mcp-server<br/>(stdio + SSE)"]
        M2["alx-mcp-client"]
    end

    subgraph H["HOSTS / MODELOS"]
        H1["Claude Code"]
        H2["Codex"]
        H3["headroom :8788 (compresión)"]
        H4["routatic :3456 (deepseek-v4-flash)"]
        H5["OmniRoute :20128 (Anthropic↔OpenAI)"]
    end

    subgraph S["SERVIDORES MCP EXISTENTES"]
        S1["codebase-memory"]
        S2["code-graph-rag"]
        S3["horario"]
        S4["media"]
        S5["notebooklm"]
        S6["perplexity"]
        S7["playwright"]
        S8["figma"]
    end

    U1 --> A
    U2 --> A
    U3 --> A
    P --> A
    A --> M
    M --> H
    M --> S
    C6 --> C1
    C6 --> C7
    C2 --> C1
    C4 --> C2
    C3 --> C1
    C5 --> C1
    C8 --> C7
    C9 --> C6
```

## 3. Workspace de crates

| Crate | Responsabilidad | Deps clave |
|---|---|---|
| `alx-core` | Tipos, estado global, event bus, reloj, IDs | serde, thiserror |
| `alx-hooks` | Ciclo de vida de hooks: registrar, disparar, timeout, lock, retry | tokio, serde_json |
| `alx-memory` | Auto-recalls: capturar aprendizajes, comprimir (caveman), inyectar en prompt | alx-core |
| `alx-governor` | Router de modelos (dificultad→tier), compresión, presupuesto de tokens, goal engine | alx-core, reqwest |
| `alx-task` | DAG de tareas: estados, dependencias, persistencia JSONL/RocksDB | alx-core |
| `alx-harness` | Pipeline de fases: contratos entrada/salida, compuertas, reanudable | alx-core, alx-task |
| `alx-gate` | Runner de verificación: build/test/lint, LSP discovery, captura de evidencia | tokio |
| `alx-bench` | Métricas de perf, umbrales, comparación de diffs, informe | criterion, toml |
| `alx-night` | Scheduler autónomo, cron, informe nocturno, commit atómico | cron, git2 |
| `alx-mcp` | Server (stdio/SSE) + client para servidores existentes | rmcp/tokio, serde_json |
| `alx-cli` | Binario `alx`: subcomandos, TUI de estado, merge con atg | clap, ratatui |
| `alx-lib` | Fachada pública: todo lo que PHALANX expone | — |

**Regla de dependencias**: apuntan hacia abajo (core es hoja). `alx-cli` y `alx-lib` son los únicos crates de entrada.

## 4. Flujo de datos canónico (una sesión)

1. `alx` arranca → `alx-core` carga estado + `alx-memory` inyecta recalls en el prompt.
2. Hook `UserPromptSubmit` → `alx-hooks` dispara cadena de hooks (memoria, governor, skill-activation).
3. `alx-governor` clasifica la petición: dificultad → tier de modelo → ruta (headroom/routatic/omniroute).
4. `alx-task` materializa o actualiza el DAG de tareas para el objetivo.
5. `alx-harness` recorre fases; cada fase llama a `alx-gate` para verificar (build/test/lint/LSP).
6. `alx-bench` mide perf y compara contra umbrales antes de merge.
7. `alx-memory` captura aprendizajes de la sesión → comprimidos → store.
8. Hook `Stop` → `alx-night` (si es noche) programa siguiente pasada / actualiza memoria.

## 5. Mapa a requisitos

| Requisito | Crate |
|---|---|
| R2 motor Rust | todo el workspace |
| R3 un plugin | alx-lib → PHALANX |
| R4 nada de comandos | alx-hooks (catálogo completo) |
| R5 harness por fase | alx-harness |
| R6 auto-memoria | alx-memory |
| R7 optimización hablar | alx-governor |
| R8 objetivos automáticos | alx-governor (goal engine) |
| R9 LSP/lint/tests auto | alx-gate |
| R10 perf matemático | alx-bench |
| R11 conectar skills+workflow | alx-mcp + alx-harness |
| R14 autónomo | alx-night + night-ops protocol |

## 6. Decisión clave

**PHALANX es config + skills + hooks; ALEXANDRIA es el cerebro.** PHALANX sin ALEXANDRIA = carpeta de markdown. ALEXANDRIA sin PHALANX = binario huérfano. Solo juntos forman la falange.
