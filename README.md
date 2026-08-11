# ⚔ 亞歷山大之構 — AlexanderTheGreat

Build de **Claude Code de última generación**, personalizada para Alexander.
Mínima, con datos, con alma: ☀ sol y ☾ luna.

> 構者，萬法之所歸；此構立，則重置無憂。

## ¿Qué es?

Un repo autocontenido que instala y reconstruye todo tu Claude Code ideal tras un reset de PC:

| Capa | Qué hace |
|---|---|
| **Wrapper `atg`** | Un solo comando con modos: `cc` (headroom+compresión), `--raw` (directo routatic), `--free` (OmniRoute), `--bare` (minimal) |
| **Statusline con datos** | Barra en vivo: ☀/☾ por hora, reloj, modelo, contexto %, coste $, git, RAM — colores sol (día) / luna (noche) |
| **Skills élite** | hallmark (anti-slop), mattpocock (ingeniería), addyosmani (24 skills prod), archify (diagramas), cangjie (distilar libros/videos), reverse-skill (pentesting), diagram-design (27 tipos), anthropics oficial |
| **Cadena de modelos** | `claude → headroom:8788 → routatic:3456 → opencode.ai` (deepseek-v4-flash, keys k1/k2 rotables, visión mimo-v2.5) |
| **OmniRoute (opcional)** | Red de seguridad verificada: claude → :20128 → fallback a 90+ free tiers (keyless) cuando opencode-go agote quota |
| **Launcher/welcome eliminado** | El panel de tips/"Welcome back" no sale: `atg` auto-genera `CLAUDE.md` al primer arranque (la pantalla de bienvenida desaparece con memoria de proyecto; queda mini-cabecera de 3 líneas — no hay switch oficial, issue abierto) |
| **Temas sol/luna en la UI** | `themes/sol.json` (ámbar/crema, día) y `themes/luna.json` (índigo/plata, noche) — `atg` los aplica solo según la hora, hot-reload en vivo |
| **Auto-init** | Primer `atg` en un proyecto: detecta stack (node/python/rust/go/make/docker/git) y genera `CLAUDE.md` con la estructura real — sin escribir `/init` nunca |
| **Backup/Restore** | `./backup.sh` — todo tu `.claude` en un tar, restaurable en 30 segundos |

## Instalación (PC nueva tras reset)

```bash
# 1. clona el repo (o cópialo desde el backup)
git clone <tu-remote> ~/Projectos/AlexanderTheGreat

# 2. prerrequisitos
HOME=/home/artorias uv tool install "headroom-ai[proxy,code]"
# + tu routatic-proxy / oc-go-cc (ver skill claude-code-ops)
# + los servicios ya vienen en systemd/ y install.sh los instala
#   (headroom :8788 → routatic :3456; omniroute :20128)

# 3. instala todo (skills + statusline + config + alias)
cd ~/Projectos/AlexanderTheGreat && ./install.sh

# 4. opcionales
./install.sh --mcp        # code-review-graph (MCP)
./install.sh --omniroute  # gateway fallback gratuito

# 5. recarga el shell y listo
source ~/.zshrc && cc
```

## Uso diario

```bash
cc                  # modo estrella: claude + headroom (compresión) + statusline + skills
cc --raw            # claude directo a routatic (sin headroom)
cc --free           # claude vía OmniRoute (fallback gratuito multi-provider)
cc --bare           # claude minimal (sin plugins/themes/skills) — modo limpio
atg --help          # modos
```

## Statusline (los datos que ves)

```
☀ 14:32 │ deepseek-v4-flash │ ▓▓▓▓▓░░░░░ 47% │ $0.012 │ ⎇ main+2 │ 6G
```

- **☀ / ☾** — paleta solar (06–18h: ámbar/oro/crema) o lunar (18–06h: índigo/plata)
- **ctx** — uso de ventana de contexto (barra 10 segmentos)
- **coste $** — coste de sesión cuando > $0.001
- **git** — rama + archivos sucios
- Actualiza cada 10 s (`refreshInterval`); no consume tokens.

## Skills

| Skill | Origen | Para qué |
|---|---|---|
| hallmark | Nutlope/hallmark | Diseño UI/UX que NO parece generado por IA (anti-slop) |
| mattpocock/skills | mattpocock | Skills de ingeniería real (typescript, testing, arquitectura) |
| agent-skills | addyosmani | 24 skills production-grade (code review, TDD, interview-me…) |
| archify | tt-a1i | Diagramas de arquitectura bonitos y verificables (HTML) |
| cangjie-skill | kangarooking | Distilar libros / vídeos largos / podcasts → skills ejecutables |
| reverse-skill | zhaoxuya520 | Router de ingeniería inversa / pentesting autorizado |
| diagram-design | cathrynlavery | 27 tipos de diagramas editoriales |
| anthropics/skills | Anthropic oficial | Set selectivo (artifacts, canvas, docs, playwright…) |

Instalación vía `npx skills add <owner>/<repo>` o clone directo a `~/.claude/skills/`.

## Backup / Restore

```bash
./backup.sh            # → backups/atg-<fecha>.tar.gz (settings + skills + statusline + headroom env)
./backup.sh --restore  # restaura el más reciente
```

## Estructura

```
AlexanderTheGreat/
├── install.sh        # instalador idempotente
├── backup.sh         # backup/restore
├── bin/atg           # wrapper (modos cc/raw/free/bare)
├── statusline/       # statusline.sh (datos + sol/luna)
├── themes/           # paletas
├── skills/           # manifest de skills
├── config/           # settings base
└── backups/          # tars de backup
```

## Troubleshooting

- **headroom no responde** → `systemctl --user restart headroom.service`
- **quota opencode-go agotada** → `cc --free` (OmniRoute) o esperar la ventana
- **sin statusline** → `jq -e '.statusLine' ~/.claude/settings.json`; reinstalar con `./install.sh --no-skills`
- **banner/mascota de Claude** → issue upstream #2254 (sin toggle oficial); el statusline lo compensa
