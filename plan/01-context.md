# 01 · Contexto — Auditoría de lo existente

> Qué hay hoy, qué se reutiliza, qué falta. Base para no reinventar.

## 1. aicli-ultimate (`~/Projectos/aicli-ultimate/`)

**Qué es**: instalador/configurador multi-CLI (Codex, Claude Code, OpenCode, OMP, Antigravity). TUI ratatui en Rust para el instalador. Confirmación del usuario: *"solo es un instalador"*.

**Inventario**:
- `install.sh` (65KB) — lógica de instalación por host, idempotente, backups, update.
- `tui/` — Cargo.toml + src: TUI ratatui del instalador.
- `plugins/`: caveman, ponytail, centaury-workflow, orquestrator, apollo-rust-best-practices, github-lsp.
- `adapters/`: antigravity, claude, opencode — config por host.
- `config/ultimate.config.toml` — modelo: `[agents]`, `[features]`, `[mcp_servers]`, `[tui]`.
- `agents/`: ultimate_planner, ultimate_researcher, ultimate_reviewer (toml).
- `git-hooks/`: pre-commit, pre-push (CentauryAI protected branch).
- `docs/`: orquestrator-agent-setup, github-ruleset.

**Reutilizable**: patrón de config TOML multi-host, catálogo de plugins (caveman, ponytail, orquestrator), estructura de adapters, concepto de "ultimate_*" agents. **No reutilizable como motor**: no hay pipeline, no hay hooks engine, no hay gobernador, no hay memoria.

## 2. AlexanderTheGreat (`~/Projectos/AlexanderTheGreat/` — repo actual)

**Qué es**: harness disperso de Claude Code. Wrapper `atg` (headroom + temas + patch), 265 agentes, plugins, hooks propios, night-ops.

**Inventario por zona**:

| Zona | Contenido | Estado |
|---|---|---|
| `bin/atg` | Wrapper bash: headroom wrap (compresión, :8788), modos free/raw/bare/clean, auto-init CLAUDE.md, tema sol/luna, repatch | ✅ funciona, manual |
| `install.sh` | Instalador idempotente (statusline, merge settings, wrapper, logo, skills, MCP, OmniRoute) | ✅ funciona, manual |
| `agents/` | 265 agentes `.md` (academic, engineering, design, sales, finance...) | ✅ registro masivo, sin validar/ordenar |
| `agents-volt/` | ~156 agentes VoltAgents | ✅ registro, sin integrar |
| `plugins/agent-skills` | 23 skills de ingeniería (spec, plan, build, test, review, ship, webperf...) + evals + hooks propios + scripts validación | ✅ la mejor base pedagógica |
| `plugins/planning-with-files` | Planner en archivos: planes, ledger, attest, session-catchup, scripts + tests Python | ✅ base de gestión de planes |
| `.claude/hooks/` | skill-activation-prompt, skill-verification-guard, post-tool-use-tracker, session-doc-updater, error-handling-reminder | ✅ hooks vivos |
| `.claude/agents/` | 8 agentes (code-reviewer, refactor-planner, documentation-architect, plan-reviewer...) | ✅ |
| `.claude/commands/` | dev-docs, verify-setup, route-research | ✅ |
| `skills/` | emil, fable, night-ops, manifest.md | ✅ |
| `scripts/` | cc-openai-bridge (Responses API→OpenAI), cc-model-mask (oculta modelo), night-run.sh, patch-logo.sh | ✅ bridges |
| `systemd/` | cloudcli, headroom, omniroute services | ✅ infra |
| `statusline/` | ccstatus sol/luna con datos | ✅ |
| `.remember/` | claude-mem: now.md, today-*.md, logs | ✅ memoria pasiva |

**Infra de red existente** (reutilizable):
- `headroom` :8788 — proxy de compresión (wrap claude).
- `routatic` :3456 — router opencode-go (deepseek-v4-flash).
- `omniroute` :20128 — traduce Anthropic↔OpenAI, fallback multi-provider.
- `cc-openai-bridge` — bridge Responses API→OpenAI local.
- `cc-model-mask` — la AI ve `claude-opus-4-6[1m]`, compacta al 92% de 1M.

## 3. Gaps (lo que NO existe)

| Gap | Impacto |
|---|---|
| No hay motor Rust | todo es bash/python disperso, lento, frágil |
| No hay engine de hooks centralizado | hooks sueltos por repo, sin catálogo, sin timeout/lock/retry |
| No hay pipeline de fases (harness) | el workflow depende de que la AI recuerde usar skills |
| No hay auto-memoria funcional | el dev repite instrucciones cada sesión |
| No hay gobernador de coste | sin routing por dificultad, sin presupuesto, sin compresión entre agentes |
| No hay verificación automática | "debería funcionar" no se detecta |
| No hay registro de agentes con validación | 265+156 agentes sin orden, sin schema, sin router |
| No hay tareas como DAG persistido | planning-with-files son planes en archivos, no estados maquina |
| No hay MCP server propio | alx no expone tools; consume servidores ajenos sin orquestar |
| No hay bench de performance | "optimizado" sin métricas ni umbrales |

## 4. Activos a REUTILIZAR (no reescribir)

1. Wrapper `atg` + headroom → base de `alx run` (modos de red).
2. Bridges (cc-openai-bridge, cc-model-mask, omniroute) → routing del gobernador.
3. `agents/` + `agents-volt/` → registry (validar con schema, no recrear).
4. `plugins/agent-skills/skills/*` → PHALANX skills (los 23, ya con evals).
5. `plugins/agent-skills/evals/*` → suite de testing del sistema.
6. `plugins/planning-with-files` → backend de `alx-task` (planes en archivos ya existen).
7. `.claude/hooks/*` → migrar a `alx-hooks` (mismo contrato, centralizado).
8. Servidores MCP existentes (codebase-memory, code-graph-rag, horario, media, notebooklm, perplexity, playwright, figma) → clientes MCP de alx.
9. `night-ops` skill → protocolo de `alx-night`.
10. `.remember/` (claude-mem) → base de `alx-memory`.

## 5. Decisión de arquitectura

**ALEXANDRIA** no reemplaza a los componentes: los **centraliza** con un motor Rust y los expone como UN plugin (PHALANX). El ordenador sabe de la falange, no de cada lanza.
