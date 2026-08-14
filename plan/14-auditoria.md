# 14 · Auditoría Exhaustiva del Ecosistema

> Inventario completo de TODO lo que existe (global + repo + MCP + red). Base para conectar todo, no perder nada, y eliminar duplicados. Auditoría ejecutada 2026-08-12 con agentes especializados.

## 1. Global (`~/.claude/`)

| Componente | Cantidad | Detalle |
|---|---|---|
| skills/ | 86 | Animación (17), UI/Design (28), Figma (6), Workflow/IA (35) |
| agents/ | 421 | academic(6), design(11), engineering(63), finance(5), gis(13), healthcare(6), marketing(33), paid-media(7), product(5), project-mgmt(6), sales(11), security(14), specialized(14), support(5), testing(8), workflow(4), lenguajes + misc |
| plugins/ | 26 (23 activos) | claude-plugins-official, caveman, ponytail(disabled), superpowers-marketplace, claude-mem, ecc, agent-skills(local), planning-with-files(local), alexander-harness(local) |
| commands/ | 2 | exit-orchestrator, orquestrator-hcom |
| hooks/ | 3 dir + HCOM/CENTAURY | cbm-code-discovery-gate(PreToolUse Grep/Glob), cbm-session-reminder(SessionStart), cbm-subagent-reminder(SubagentStart) + HCOM/CENTAURY hooks en TODOS los eventos + .orca/agent-hooks |
| MCP | 7+3 | ver §4 |
| Perfiles paralelos | 7 | .claude-subscription2..9, .claude-ultimate, .claude-modal-1 (cada uno con su .mcp.json) |

**Env global**: `ANTHROPIC_BASE_URL=http://127.0.0.1:3460` (routatic vía cc-model-mask), modelo `claude-opus-4-6[1m]`, statusLine `ccstatusline` (10s), theme `custom:sol`, `permissions.defaultMode=acceptEdits`, `teammateMode=tmux`.

## 2. Repo AlexanderTheGreat

| Zona | Contenido |
|---|---|
| plugins/agent-skills | 24 skills, 8 commands, 4 agents, 24 evals + fixtures, hooks (session-start, sdd-cache, simplify-ignore) |
| plugins/planning-with-files | 6 skills (multi-idioma), 13 commands, ~25 scripts, 6 templates, reempaquetado multi-tool |
| plugins/alexander-harness | agent-index.json (410), hooks/agent-dispatch.sh, commands/agents+spawn, skills/agent-dispatch |
| skills/ | emil (10 sub-skills), fable (fable-mode + guardrails + opus/sonnet/haiku), night-ops, manifest.md |
| agents/ | 265 (engineering-58, marketing-36, specialized-15, gis-13, security-12, sales-11, design-10, testing-9, project-7, paid-7, support-6, healthcare-6, academic-6, product-5, finance-5, workflow-4, xr-3, legal-3 + ~40 singletons) |
| agents-volt/ | 156 (plano: powershell-5, data-4, ai/api/security/project/performance/mobile/error/dotnet/devops/database/content/design/backend 2 c/u + resto 1) |
| .claude/hooks/ | Conectados: skill-activation-prompt, skill-verification-guard, post-tool-use-tracker, skill-activation-tracker, session-doc-updater. Sueltos: error-handling-reminder, stop-build-check-enhanced, trigger-build-resolver, tsc-check. lib/ (7 .ts: embeddings, gemini-client, metrics, session-parser, session-state, types, vector-store), providers/ (ai/anthropic/gemini/ollama/openai) |
| .claude/agents/ | 8 (auto-error-resolver, code-architecture-reviewer, code-refactor-master, documentation-architect, frontend-error-fixer, plan-reviewer, refactor-planner, web-research-specialist) |
| .claude/commands/ | dev-docs, dev-docs-update, route-research-for-testing, verify-setup |
| scripts/ | ccmodel, cc-model-mask.py, cc-openai-bridge.py, night-run.sh, oc-go-cc-wrapper, patch-logo.sh, build-agent-index.py |
| statusline/ | statusline.sh (☀/☾, modelo, contexto%, coste, branch, RAM), ccstatus-sol/luna.json |
| systemd/ | cloudcli(:3002), headroom(:8788), omniroute(:20128) |
| themes/ | sol.json, luna.json |

## 3. Infraestructura de red (cadena real verificada)

```
CC → headroom:8788 (compresión) → cc-model-mask:3460 (enmascara modelo) → routatic:3456 (router deepseek) → deepseek-v4-flash
```

| Puerto | Proceso | Rol |
|---|---|---|
| 3456 | routatic-proxy | Router opencode-go → deepseek (responde, sin /readyz) |
| 8788 | headroom | Compresión de contexto, upstream 3460 |
| 3460 | cc-model-mask.py | Enmascara `claude-opus-4-6[1m]` ↔ `deepseek-v4-flash` (es el ANTHROPIC_BASE_URL) |
| 3461 | cc-openai-bridge.py | Bridge OpenAI chat/completions ↔ Anthropic messages |
| 20128 | omniroute | Gateway multi-proveedor fallback |
| 3002 | cloudcli | UI remota web/móvil |
| 3000 | Hermes dashboard | Gateway/observabilidad |

User services externos: oc-go-cc, cc-openai-bridge, cloudflared-tunnel, ocq-tunnel, familia hermes-*.

## 4. Servidores MCP

| Servidor | Comando | Notas |
|---|---|---|
| perplexity | hermes-perplexity-mcp | búsqueda web |
| playwright | npx @playwright/mcp | E2E/navegador |
| horario | uv run mcp-horario | horario/castigos |
| media | uv run mcp-media | análisis video/audio |
| notebooklm | uv run mcp-notebooklm | síntesis fuentes |
| code-graph-rag | code-graph-rag mcp-server | grafo de código (Cypher → :3461) |
| codebase-memory-mcp | codebase-memory-mcp | memoria de código |
| figma (plugin) | HTTP mcp.figma.com | diseño |
| mcp-search (claude-mem) | node scripts/mcp-server.cjs | memoria claude-mem |
| chrome-devtools (ecc) | npx chrome-devtools-mcp | devtools |
| **Disabled**: scrapling, duckduckgo, kindly, heor-agent, aria-clinical-research, deeplook, web-search, blender-mcp, blender-vse | | |

## 5. Duplicados y solapamientos (los que hay que resolver)

| Concepto | Dónde aparece | Resolución |
|---|---|---|
| night-ops | repo/skills + global/skills + dir raíz night-ops/ | 1 fuente → PHALANX skills |
| fable | repo/skills + global | 1 fuente |
| emil | repo/skills/emil (10) + global (emil-design-eng) | 1 fuente |
| planning-with-files | plugin + reempaquetado .codex/.cursor/.gemini... | mantener plugin, ignorar reempaquetados |
| caveman | plugin global + skills caveman-compress/stats | unificar en PHALANX (compresión) |
| code-review | ≥6 sitios: engineering-code-reviewer, agents-volt/code-reviewer, agent-skills/agents/code-reviewer, global code-reviewer, ecc:code-reviewer, skill code-review-and-quality | 1 agente + 1 skill (router) |
| spec/plan/test/review/ship | comandos agent-skills + ecc + planning-with-files + plan-reviewer | 1 comando por fase (harness) |
| agent-index | alexander-harness (410) = agents(265)+volt(156)+global(421) solapados | registry único dedup |
| anime-js≈animejs-animation | global | dedup al integrar |
| spline-3d≈spline-3d-integration | global | dedup |
| grilling≈grill-with-docs | global | dedup |
| premium≈elevated≈frontend-design≈taste | global | dedup |
| smooth-scroll≈lenis-scroll | global | dedup |

## 6. Mapa de integración (componente → dónde vive en ALEXANDRIA/PHALANX)

| Componente actual | Destino |
|---|---|
| atg, headroom, cc-model-mask, cc-openai-bridge, omniroute, routatic | gobernador + alx-cli (modos de red) |
| skills/ (emil, fable, night-ops) + 86 globales | PHALANX skills (registry dedup) |
| 421+421 agentes | alx-agents registry (validar, dedup, rutear) |
| planning-with-files | alx-task (capa legible) |
| hooks sueltos (.claude/hooks, cbm-*, HCOM/CENTAURY, .orca) | alx-hooks (catálogo unificado) |
| MCP servers (10 activos) | alx-mcp client |
| statusline/themes | alx-cli TUI |
| systemd services | infra (alx-night + alx-governor los usan) |
| .remember / claude-mem / mcp-search | alx-memory |
| alexander-harness (agent-dispatch) | alx-agents (spawn) |
| night-run.sh | alx-night |
| dev-docs commands | alx-harness Docs phase |

## 7. Conclusión

El ecosistema real es ~10x más grande que lo mapeado en 01. **Nada se pierde**: todo se indexa en el registry de PHALANX. **Nada se duplica**: el router/registry dedup. La falange une todas las lanzas existentes.
