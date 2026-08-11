# Manifest de skills — AlexanderTheGreat

> 技多不壓身；此冊錄百工之源。

| Skill | Origen | Instalador | Estado |
|---|---|---|---|
| hallmark | https://github.com/Nutlope/hallmark | `npx skills add nutlope/hallmark -g` | auto (install.sh) |
| mattpocock/skills | https://github.com/mattpocock/skills | `npx skills add mattpocock/skills -g` | auto |
| archify | https://github.com/tt-a1i/archify | `npx skills add tt-a1i/archify -g` | auto |
| cangjie-skill | https://github.com/kangarooking/cangjie-skill | `npx skills add kangarooking/cangjie-skill -g` | auto |
| reverse-skill | https://github.com/zhaoxuya520/reverse-skill | git clone → ~/.claude/skills/ | auto |
| diagram-design | https://github.com/cathrynlavery/diagram-design | git clone → ~/.claude/skills/ | auto |
| anthropics/skills (selectivo) | https://github.com/anthropics/skills | git clone → ~/.claude/skills/ | auto |
| agent-skills (addyosmani) | https://github.com/addyosmani/agent-skills | `npx skills add addyosmani/agent-skills` (manual, 24) | pendiente decidir |
| google/skills | https://github.com/google/skills | `npx skills add google/skills` (manual) | opcional |
| code-review-graph (MCP) | https://github.com/tirth8205/code-review-graph | `uv tool install code-review-graph` + `install --platform claude` | `--mcp` |
| code-graph-rag | https://github.com/vitali87/code-graph-rag | pip con extras treesitter-full+semantic | opcional (pesado) |
| TencentDB-Agent-Memory | https://github.com/Tencent/TencentDB-Agent-Memory | git clone + deploy | opcional (redundante con headroom --memory) |

## Stack de modelos (cadena)

```
cc  →  headroom :8788  →  routatic-proxy :3456  →  opencode.ai/zen/go/v1
                    (compresión)      (traducción+scenarios+vision)   (deepseek-v4-flash, k1/k2)
cc --free  →  OmniRoute :20128  →  fallback 4 niveles (subscription→API→cheap→free)
cc --raw   →  routatic directo
```
