# Perfil de test ALEXANDRIA

> Perfil nuevo de Claude Code que prueba el motor ALEXANDRIA desde cero,
> conectado a la cadena real (headroom→mask→routatic→deepseek).

## Cómo usarlo

```bash
CLAUDE_CONFIG_DIR=~/.claude-alexandria claude
```

## Qué trae

| Componente | Estado |
|---|---|
| Conexión a routatic (ANTHROPIC_BASE_URL=:3460) | ✓ |
| Modelo enmascarado (claude-opus-4-6[1m] ↔ deepseek) | ✓ |
| **Statusline POWERLINE** (`alx-statusline`) | ✓ segmentos: ALEXANDRIA · tasks · net · cost · iter |
| MCP: `alexandria` (alx mcp) + codebase-memory | ✓ |
| Hook iterate (R24) | ✓ |
| CLAUDE.md: caveman alto para pensar, normal para datos finales | ✓ |
| Auto-compact agresivo (16k) | ✓ |

## Statusline (verificado)

```
 ALEXANDRIA  tasks:0  net:4/4  cost:$0.000160  iter:0/20
```
(separadores powerline)

## Archivos

- `~/.claude-alexandria/settings.json` — env + statusLine + hooks
- `~/.claude-alexandria/.mcp.json` — alx mcp + codebase-memory
- `~/.claude-alexandria/CLAUDE.md` — modo de trabajo caveman + reglas
- `~/.claude-alexandria/hooks/iterate.sh` — recordatorio R24
- `~/.local/bin/alx-statusline` — statusline powerline
