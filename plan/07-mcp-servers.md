# 07 · MCP Servers — `alx-mcp`

> El bus de integración. ALEXANDRIA ES un servidor MCP (expone tools a cualquier host) y ES un cliente MCP (consume servidores existentes). Cero integraciones punto a punto.

## 1. Modos

| Modo | Transporte | Para qué |
|---|---|---|
| `alx mcp server --stdio` | stdio | Claude Code / Codex / cualquier host lo monta como herramienta |
| `alx mcp server --sse` | HTTP SSE | hosts remotos, dashboards, `alx-night` headless |
| `alx mcp client` | stdio/SSE out | consume servidores existentes |

## 2. Superficie de tools que expone alx (server)

> Todo lo que el motor sabe hacer, expuesto como herramienta MCP. Los hooks deciden cuándo llamarlas; los hosts las ven.

| Namespace | Tools |
|---|---|
| `task.*` | `task.create`, `task.list`, `task.status`, `task.depends`, `task.update` |
| `harness.*` | `harness.run`, `harness.phase`, `harness.resume`, `harness.gate` |
| `agent.*` | `agent.spawn`, `agent.list`, `agent.route`, `agent.validate` |
| `hook.*` | `hook.list`, `hook.trigger`, `hook.toggle`, `hook.logs` |
| `memory.*` | `memory.recall`, `memory.inject`, `memory.capture`, `memory.forget` |
| `governor.*` | `governor.classify`, `governor.budget`, `governor.cost-report` |
| `gate.*` | `gate.run`, `gate.evidence`, `gate.lsp-discover` |
| `bench.*` | `bench.run`, `bench.thresholds`, `bench.report` |
| `phalanx.*` | `phalanx.status`, `phalanx.skills`, `phalanx.mission`, `phalanx.config` |

## 3. Cliente a servidores existentes

| Servidor | Cómo lo usa alx |
|---|---|
| `codebase-memory` | Ingest/Build: conocimiento estructural del repo (call graphs, arquitectura) |
| `code-graph-rag` | Ingest: preguntas sobre el código, indexar |
| `horario` | Night/autonomía: respeta horario y castigos (no romper rutinas) |
| `media` | Docs/Review: analizar video/audio si aplica |
| `notebooklm` | Docs/Research: síntesis sobre fuentes |
| `perplexity` | Research: búsqueda web con fuentes |
| `playwright` | Test/Review: E2E, verificación visual en navegador |
| `figma` | Frontend: extraer specs de diseño |

El cliente MCP descubre tools de cada servidor, las registra en el catálogo de alx, y el governor controla qué se permite según fase y presupuesto.

## 4. Seguridad y gobernanza

- **Allowlist de tools por fase**: `phalanx/security.toml` define qué tools puede usar cada fase. Ej: `[phase.Review] allow = ["gate.*","agent.*","memory.*"]`.
- **Coste por tool**: cada tool MCP registra coste estimado (tokens+latencia). El governor lo suma al presupuesto de la tarea.
- **Sandbox**: `gate.*` y `task.*` que ejecutan comandos corren con sandbox (no red, no HOME) salvo override explícito.
- **Auditoría**: toda llamada MCP → `event.log` con quién la pidió, cuándo, qué costó.

## 5. Decisiones

- **Un crate, dos modos**: `alx-mcp` implementa protocolo una vez; server y client comparten el core de protocolo.
- **El host no sabe de servidores internos**: Claude Code solo ve `alx` como un servidor más. La orquestación de codebase-memory/figma/etc es interna del motor.
- **Catálogo central**: todas las tools de todos los orígenes (propias + clientes) se registran en un catálogo único; es la fuente para el router y el validador.
- **MCP como futuro**: cualquier host nuevo (Cursor, Windsurf, Gemini CLI) se integra montando el server `alx` — cero código nuevo por host.
