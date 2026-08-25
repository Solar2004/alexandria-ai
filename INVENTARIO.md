# 📦 INVENTARIO — AlexanderTheGreat (build completa)

> Estado: 2026-08-12 · Repo: /home/artorias/Projectos/AlexanderTheGreat
> Cada pieza está verificada por invocación real (no "instalada y a ver").

---

## 1. Stack de modelos (cadena de ejecución)

```
cc / claude → atg → headroom :8788 → routa-gateway :3460 → routatic :3456 → opencode-go → modelo real del config (`routa show`; 1M visible)
```

| Capa | Puerto/Servicio | Función |
|---|---|---|
| `atg` (wrapper) | — | auto_init, tema sol/luna, re-brand ☀, env 1M |
| headroom | :8788 | wrap de sesión (compresión, headroom) |
| routa-gateway | :3460 (+:3461 OpenAI) | máscara [1m] + suelo max_tokens + probes cortocircuitados + gobernador de entropía + failover de modelos; sustituye a cc-model-mask y cc-openai-bridge |
| routatic-proxy | :3456 | routing por escenario, fallbacks, rota claves k1/k2, visión mimo-v2.5 |
| cc-openai-bridge | :3461 | OpenAI/Responses API → Anthropic (para code-graph-rag Cypher gen) |
| omniroute | :20128 | fallback free tiers (`cc --free`) |
| opencode.ai | — | proveedor real (modelo deepseek-v4-flash, 1M context) |

⚠️ Reglas: NO tocar `/model` en CC (rompe el disfraz → 200k). Coste mostrado inflado (CC lo calcula como Opus) — real es deepseek.

## 2. Servicios systemd (usuario, autostart)

| Servicio | Puerto | Estado |
|---|---|---|
| headroom.service | :8788 | enabled |
| oc-go-cc.service | :3456 | enabled (rota k1/k2) |
| routa-gateway.service | :3460/:3461 | enabled |
| cc-openai-bridge.service | :3461 | enabled |
| omniroute.service | :20128 | enabled |
| cloudcli.service | :3002 | enabled (control remoto, cc.centaury.net) |
| night-ops.timer | — | enabled (02:00 diario) |
| night-ops.service | — | oneshot (procesa cola nocturna) |

## 3. MCPs (9, todos globales user-scope, `✔ Connected`)

perplexity · playwright · horario · media · notebooklm · code-graph-rag · figma · claude-mem (mcp-search) · chrome-devtools (ecc)
Blindado: `enabledMcpjsonServers: ["*"]` disabled, `~/.mcp.json` eliminado, uv MCPs con `env -u PYTHONPATH`.

## 4. Agentes (421 subagentes + 8 showcase)

| Fuente | Cantidad | Notas |
|---|---|---|
| agency-agents | 265 | especialistas por dominio (marketing, engineering, security…) |
| VoltAgent | 156 | awesome-claude-code-subagents (react, mlops, data…) |
| Showcase diet103 | 8 | code-architecture-reviewer, refactor-master, etc. |
| Plugin ecc | 68 | prefijo `ecc:` |
| agent-skills / feature-dev / code-simplifier | 8 | prefijos de plugin |

Selección: **plugin propio `alexander-harness`** (dispatcher hook) + selector nativo CC.
Índice: `plugins/alexander-harness/agent-index.json` (410 con desc, 139 categorías) — regenerar con `scripts/build-agent-index.py`.
Descripciones podadas a ≤18 palabras (5.4k tokens total, límite 15k) — cuerpos intactos.

## 5. Skills (~85 globales + plugins)

- Globales: fable-mode (5), emilkowalski (10 UI/animación), night-ops, showcase (backend/frontend-dev-guidelines, skill-developer, error-tracking), agent-dispatch (propia), superpowers, ecc, caveman + resto
- Plugin agent-skills (addyosmani): 24 skills ingeniería + 8 comandos (/spec /plan /build /test /review /ship)
- Auto-activación: hooks showcase en el proyecto (skill-rules.json, regex offline, 8/8 verify) — las skills obligatorias se cargan antes de editar

## 6. Plugins / Marketplaces instalados

caveman · claude-plugins-official · ecc · ponytail · superpowers-marketplace · thedotmack · addys-agent-skills · planning-with-files · **alexander-harness (PROPIO, v0.1.0)**

## 7. Hooks

- Proyecto `.claude/settings.json`: skill-activation-prompt (UserPromptSubmit), skill-verification-guard (PreToolUse Edit/Write), post-tool-use-tracker, skill-activation-tracker, session-doc-updater (Stop)
- Plugin alexander-harness: **agent-dispatch** (UserPromptSubmit) — sugiere el subagente correcto por tarea (ES/EN, score por palabras clave + nombre)

## 8. Comandos slash

`/agents` (buscar agente) · `/spawn` (generar Task) — propios · `/verify-setup` `/dev-docs` `/dev-docs-update` `/route-research-for-testing` (showcase) · 8 de agent-skills

## 9. Herramientas propias (`~/.local/bin` + repo scripts/)

ccmodel · routa (CLI de modelos) · routa-gateway.py · oc-go-cc-wrapper v2 (clave sticky) · hermes-perplexity-mcp · night-run.sh · build-agent-index.py · patch-logo.sh · backup.sh · install.sh · statusline ccstatusline (☀/☾, powerline)

## 10. Memoria y orquestación

- claude-mem: memoria persistente por proyecto (2ª sesión en adelante)
- Agent Teams: `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` — agentes paralelos que se hablan
- planning-with-files: planes persistentes en disco (task_plan/findings/progress)
- code-graph-rag: grafo del repo (Memgraph :7687, Qdrant :6333) — Cypher gen con deepseek local
- Harness de calidad: `~/CLAUDE.md` (7 reglas obligatorias: spec → verificar → tests → diff → errores=datos → agentes → evidencia)
- night-ops: cola `night-ops/queue.md` → timer 02:00 → informe `night-ops/report/YYYY-MM-DD.md` → cron 08:00

## 11. Branding

patch-cc (AlexanderTheGreat · atg-build · ☀) — re-aplicado automáticamente por `atg ensure_patched()` tras cada update. Backups: `~/.local/share/patch-cc/backups/2.1.228.orig`

## 12. Portabilidad (reset de PC)

1. Copiar carpeta (o `backups/*.tar.gz`) → 2. node/uv/docker → 3. `./backup.sh --restore` → 4. `backups/stack.txt` → 5. `./install.sh`
Backup último: `backups/atg-20260812-164811.tar.gz` (224 MB)
`install.sh` replica TODO (3a–3h): MCPs, agentes, skills, plugins, servicios, night-ops, plugin propio.

## 13. Puertos en uso

8788 headroom · 3460 mask · 3456 routatic · 3461 bridge · 20128 omniroute · 3002 cloudcli · 8791 ocq-web · 7687 Memgraph · 6333 Qdrant · 9178 hindsight · 9119 hermes dashboard · 8787 HermesWebUI (¡NO TOCAR!)
