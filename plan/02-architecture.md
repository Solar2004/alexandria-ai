# 02 · Arquitectura — ALEXANDRIA

> Arquitectura final (iteración 40 de 40). Evolución completa: `iterations/architecture-iterations.md`. Base de diseño: auditoría exhaustiva (`14-auditoria.md`) y auto-crítica (`15-critic.md`).

## 1. Principio de capas

```
┌──────────────────────────────────────────────────────────────┐
│  USUARIO  (terminal, `alx`, `atg`)                           │
├──────────────────────────────────────────────────────────────┤
│  PHALANX  (EL ÚNICO plugin: config + skills + hooks + agents)│
├──────────────────────────────────────────────────────────────┤
│  ALEXANDRIA  (motor Rust — 15 crates)                        │
│   core · hooks · memory · governor · task(+decomp) · harness │
│   gate · bench · critic · audit · night · mcp · agents       │
├──────────────────────────────────────────────────────────────┤
│  BUS MCP  (server propio + clientes a 10 servers existentes) │
├──────────────────────────────────────────────────────────────┤
│  RED  (headroom → mask → routatic → deepseek; omniroute, br) │
└──────────────────────────────────────────────────────────────┘
```

Nada salta capas. El motor lo ve todo; el usuario solo ve PHALANX.

## 2. Mermaid MEGA — el sistema completo (iteración 40)

```mermaid
flowchart TB
    subgraph U["USUARIO"]
        U1["alx (CLI)"]
        U2["atg (legacy)"]
        U3["hooks Claude Code"]
    end

    subgraph P["PHALANX · EL UNICO PLUGIN"]
        direction TB
        P1["config.toml"]
        P2["skills/ (24 agent-skills + night-ops + fable + emil, dedup)"]
        P3["hooks/ (20 .toml)"]
        P4["agents/ (registry 842, dedup)"]
        P5["plans/ (planning-with-files)"]
        P6["critics.toml (auto-critica)"]
        P7["bench.toml (umbrales)"]
        P8["security.toml (allowlists)"]
    end

    subgraph A["ALEXANDRIA · Motor Rust (workspace 15 crates)"]
        direction TB
        C1["alx-core<br/>tipos · estado · event bus · reloj"]
        C2["alx-hooks<br/>eventos → cadena Pre/Async/Post · timeout · lock"]
        C3["alx-memory<br/>auto-recalls · compresion caveman · inyeccion"]
        C4["alx-governor<br/>routing dificultad→tier · budget · ledger"]
        C5["alx-task<br/>DAG tareas · decomposition engine (micro-tareas)"]
        C6["alx-harness<br/>pipeline 8 fases · contratos · compuertas"]
        C7["alx-gate<br/>verificacion · LSP auto · lint · evidencia"]
        C8["alx-bench<br/>metricas perf · umbrales · diff check"]
        C9["alx-critic<br/>auto-critica · feedback loop · escalada T3"]
        C10["alx-audit<br/>registry dedup · doctor · valida ecosistema"]
        C11["alx-night<br/>cron autonomo · informe · commit atomico"]
        C12["alx-mcp<br/>server stdio/SSE · client"]
        C13["alx-agents<br/>registry · router · spawn · headless sessions"]
    end

    subgraph M["BUS MCP"]
        M1["alx-mcp-server (stdio + SSE)"]
        M2["alx-mcp-client"]
    end

    subgraph S["SERVIDORES MCP (5 default)"]
        S1["codebase-memory"]
        S2["code-graph-rag"]
        S3["notebooklm"]
        S4["mcp-search (claude-mem)"]
        S5["chrome-devtools"]
    end

    subgraph NET["RED (corregida iter 41: routatic=PROVIDER)"]
        direction LR
        N1["headroom :8788 compresion"]
        N2["cc-model-mask :3460"]
        N3["routatic :3456 PROVIDER"]
        N4["deepseek-v4-flash"]
        N5["omniroute :20128 fallback gateway (solo si routatic cae)"]
        N6["cc-openai-bridge :3461"]
    end

    subgraph HOST["HOSTS"]
        H1["Claude Code"]
        H2["Codex (futuro)"]
    end

    U1 --> A
    U2 --> A
    U3 --> A
    P --> A
    A --> M
    M --> S
    M --> H
    C4 --> N1
    N1 --> N2 --> N3 --> N4
    N5 -.-> N3
    N6 -.-> N3
    C6 --> C1
    C6 --> C7
    C6 --> C9
    C5 --> C1
    C9 --> C3
    C10 --> C13
    C13 --> C12
```

## 3. Mermaid del PIPELINE con decisiones (el "cuándo pasa, ifs")

```mermaid
flowchart LR
    START[Objetivo] --> ING[Ingest]
    ING --> SP[Spec]
    SP --> PL[Plan]
    PL --> DQ{Descomponer?}
    DQ -->|grande| MICRO[Micro-tareas con assert]
    DQ -->|pequeña| BL[Build]
    MICRO --> BL
    BL --> GT{Gate verde?}
    GT -->|no| RT{Retry < 2?}
    RT -->|si| BL
    RT -->|no| FAIL[Failed + diagnostico]
    GT -->|si| CR{Critic aprueba?}
    CR -->|rechaza| IT{Iter < 3?}
    IT -->|si| BL
    IT -->|no| ESC[Escalar T3]
    ESC --> BL
    CR -->|si| TS[Test]
    TS --> RV[Review + Security]
    RV --> DOC[Docs]
    DOC --> SH[Ship: commit + PR]
    SH --> EVD[Evidence + informe]
```

## 4. Mermaid de RED (cadena canónica)

```mermaid
flowchart LR
    CC[Claude Code] --> H[headroom :8788 compresion]
    H --> MASK[cc-model-mask :3460]
    MASK --> R[routatic :3456 PROVIDER]
    R --> DS[deepseek-v4-flash]
    OMNI[omniroute :20128 fallback gateway] -.->|solo si routatic cae| R
    BR[cc-openai-bridge :3461 OpenAI↔Anthropic] -.-> R
    CC2[Codex futuro] --> H
```

## 5. Workspace de crates (15)

| Crate | Responsabilidad | Deps clave |
|---|---|---|
| `alx-core` | Tipos, estado, event bus, reloj, IDs | serde, thiserror |
| `alx-hooks` | Ciclo de vida de hooks: registrar, disparar, timeout, lock, retry | tokio, serde_json |
| `alx-memory` | Auto-recalls: capturar, comprimir (caveman), inyectar | alx-core |
| `alx-governor` | Routing (dificultad→tier→ruta real), compresión, presupuesto, ledger | reqwest |
| `alx-task` | DAG de tareas + decomposition engine (micro-tareas con assert) | alx-core |
| `alx-harness` | Pipeline de fases: contratos, compuertas, reanudable | alx-core, alx-task |
| `alx-gate` | Runner de verificación: build/test/lint, LSP discovery, evidencia | tokio |
| `alx-bench` | Métricas de perf, umbrales, diff check | criterion, toml |
| `alx-critic` | **Auto-crítica**: feedback loop, escalada, critic.learn | alx-memory, alx-gate |
| `alx-audit` | **Registry dedup**: indexa skills/agents/plugins/hooks del ecosistema, doctor | serde |
| `alx-night` | Scheduler autónomo, cron, informe, commit atómico | cron, git2 |
| `alx-mcp` | Server (stdio/SSE) + client a 10 servidores existentes | rmcp, tokio |
| `alx-agents` | Registry, router, spawn, headless sessions (summoning) | alx-core |
| `alx-cli` | Binario `alx`: subcomandos, TUI estado, merge atg | clap, ratatui |
| `alx-lib` | Fachada pública: todo lo que PHALANX expone | — |

**Regla de dependencias**: apuntan hacia abajo (core es hoja). `alx-cli` y `alx-lib` son las únicas entradas.

## 6. Flujo de datos canónico (una sesión, con crítica)

1. `alx` arranca → `alx-core` carga estado + `alx-memory` inyecta recalls + `alx-audit` valida ecosistema.
2. Hook `UserPromptSubmit` → `alx-hooks` dispara cadena (mission, governor.classify, skill.activation).
3. `alx-governor` clasifica dificultad → tier → **ruta real** (headroom→mask→routatic).
4. `alx-task` descompone el objetivo en micro-tareas (assert por paso).
5. `alx-harness` recorre fases; `alx-gate` verifica cada micro-tarea; `alx-critic` pule hasta aprobar.
6. `alx-bench` mide contra umbrales antes de merge.
7. `alx-critic` aprende: errores corregidos → `must_check` futuros.
8. `alx-memory` captura aprendizajes → store. Hook `Stop` → night agenda siguiente pasada.

## 7. Mapa a requisitos (R1–R19)

| Requisito | Componente |
|---|---|
| R1 sistema autónomo definitivo | ALEXANDER (todo) |
| R2 motor Rust | workspace 15 crates |
| R3 un solo plugin | alx-lib → PHALANX |
| R4 nada de comandos, todo hooks | alx-hooks + catálogo 20 hooks |
| R5 harness por fase | alx-harness (8 fases) |
| R6 auto-memoria | alx-memory |
| R7 optimización de hablar | alx-governor (compresión caveman) |
| R8 objetivos automáticos | alx-task (goal engine) |
| R9 LSP/lint/tests auto | alx-gate |
| R10 perf matemático | alx-bench |
| R11 conectar skills+workflow | alx-mcp + alx-harness |
| R12 mermaid ≥20 iteraciones | iterations/ (40 hechas) |
| R13 guardar memoria | MISSION.md + alx-memory |
| R14 autónomo | alx-night + night-ops |
| R15 auditar ecosistema, sin duplicados | alx-audit (86 skills, 842 agents, 26 plugins) |
| R16 mermaid grande con ifs | este doc §2–4 |
| R17 descomponer tareas (imposible fallar) | alx-task decomposition engine |
| R18 auto-crítica sin aprobación | alx-critic |
| R19 sesiones headless simples | alx-agents headless + summoning |

## 8. Decisión clave

**PHALANX es config + contenido; ALEXANDRIA es el cerebro; el critic es la conciencia.** Sin critic, el sistema ejecuta; con critic, el sistema mejora. La falange conquista porque aprende de cada batalla.
