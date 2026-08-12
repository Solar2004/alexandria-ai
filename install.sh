#!/bin/bash
# ============================================================
#  AlexanderTheGreat — instalador idempotente
#  Repo: ~/Projectos/AlexanderTheGreat
#  Uso:  ./install.sh [--no-skills] [--mcp] [--omniroute] [--force]
#  Instala: statusline con datos + tema sol/luna, skills élite,
#  merge de settings.json, wrapper atg, alias cc, (MCP, OmniRoute)
# ============================================================
set -euo pipefail
ATGHOME="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export PATH="/home/artorias/.local/bin:$PATH"
SKILLS=1; MCP=0; OMNIROUTE=0; FORCE=0
for arg in "$@"; do
  case "$arg" in
    --no-skills) SKILLS=0 ;;
    --mcp)       MCP=1 ;;
    --omniroute) OMNIROUTE=1 ;;
    --force)     FORCE=1 ;;
  esac
done

ok()   { echo -e "\033[38;2;255;176;32m☀\033[0m $1"; }
warn() { echo -e "\033[38;2;230;126;34m⚠\033[0m $1"; }
step() { echo -e "\033[38;2;255;215;0m──\033[0m $1"; }
fail() { echo -e "\033[38;2;255;99;132m✗\033[0m $1"; exit 1; }

# ---------- 0. Prerequisitos ----------
step "Prerequisitos"
for b in claude jq node npm curl git; do
  command -v "$b" >/dev/null || fail "falta $b"
done
HEADROOM=/home/artorias/.local/bin/headroom
[ -x "$HEADROOM" ] || warn "headroom no está en $HEADROOM — instala: HOME=/home/artorias uv tool install 'headroom-ai[proxy,code]'"
curl -s -m 5 http://127.0.0.1:8788/readyz >/dev/null 2>&1 || warn "headroom :8788 no responde — systemctl --user restart headroom.service"
ok "prereqs OK"

# ---------- 1. Statusline ----------
step "Statusline (datos + tema sol/luna)"
mkdir -p /home/artorias/.claude
install -m 755 "$ATGHOME/statusline/statusline.sh" /home/artorias/.claude/statusline.sh
ok "statusline.sh → ~/.claude/statusline.sh"

# ---------- 2. Merge settings.json ----------
step "Settings merge (statusLine, sin tocar el resto)"
SET=/home/artorias/.claude/settings.json
if [ -f "$SET" ]; then
  cp "$SET" "$SET.bak-atg-$(date +%Y%m%d%H%M)"
fi
jq -S '.statusLine = {type:"command", command:"~/.claude/statusline.sh", refreshInterval: 10, hideVimModeIndicator: true}' \
  < "$SET" > "$SET.tmp" && mv "$SET.tmp" "$SET"
ok "statusLine activado en settings.json (backup previo hecho)"

# ---------- 3. Wrapper atg + alias ----------
step "Wrapper atg + alias cc"
install -m 755 "$ATGHOME/bin/atg" /home/artorias/.local/bin/atg
# alias claude → atg (toda la build, no el binario pelado); subs quedan al binario directo
if grep -q "^alias claude='ANTHROPIC_BASE_URL" /home/artorias/.zshrc 2>/dev/null; then
  sed -i "s|^alias claude='ANTHROPIC_BASE_URL=http://127.0.0.1:3456 ~/.local/bin/claude'|alias claude='atg'|" /home/artorias/.zshrc
  ok "alias claude → atg"
fi
if ! grep -q "alias cc=" /home/artorias/.zshrc 2>/dev/null; then
  cat >> /home/artorias/.zshrc <<'EOF'

# AlexanderTheGreat — claude code build (atg)
alias cc='atg'
EOF
  ok "alias cc='atg' añadido a .zshrc"
else
  if grep -q "alias cc='atg'" /home/artorias/.zshrc 2>/dev/null; then
    ok "alias cc ya apunta a atg"
  else
    warn "ya existe 'alias cc=' en .zshrc — NO tocado (revisar manualmente)"
  fi
fi

# ---------- 3b. Logo ⚔ (binario parcheado) ----------
step "Logo ⚔ (AlexanderTheGreat)"
if [ -f "$ATGHOME/scripts/patch-logo.sh" ]; then
  bash "$ATGHOME/scripts/patch-logo.sh" && ok "logo ⚔ aplicado" || warn "logo: binario no encontrado (¿update pendiente?)"
else
  warn "scripts/patch-logo.sh no existe en repo"
fi

# ---------- 3c. MCPs globales (user scope, sin .mcp.json locales) ----------
step "MCPs globales (perplexity, playwright, horario, media, notebooklm)"
python3 - <<'PYEOF'
import json, os
p = os.path.expanduser("~/.claude.json")
d = json.load(open(p)) if os.path.exists(p) else {}
user = d.setdefault("mcpServers", {})
user.setdefault("perplexity", {"command": "/home/artorias/.local/bin/hermes-perplexity-mcp"})
user.setdefault("playwright", {"command": "npx", "args": ["-y", "@playwright/mcp@latest"]})
def blind(name, args, env=None):
    user[name] = {"command": "/usr/bin/env",
                  "args": ["-u", "PYTHONPATH", "/home/artorias/.local/bin/uv", "run"] + args,
                  "env": env or {}}
blind("horario", ["--directory", "/home/artorias/Documentos/Alexander/config/.mcp-servers/mcp-horario", "horario-mcp"],
      {"HORARIO_BASE": "/home/artorias/Documentos/Alexander"})
blind("media", ["--directory", "/home/artorias/Documentos/Alexander/config/.mcp-servers/mcp-media", "media-mcp"])
blind("notebooklm", ["--directory", "/home/artorias/Documentos/Alexander/config/.mcp-servers/mcp-notebooklm", "nblm-mcp"])
json.dump(d, open(p, "w"), indent=2)
# settings: ignorar .mcp.json de proyectos ajenos
sp = os.path.expanduser("~/.claude/settings.json")
s = json.load(open(sp))
s["disabledMcpjsonServers"] = ["*"]
allow = set(s.get("permissions", {}).get("allow", []))
for pref in ("mcp__perplexity__", "mcp__playwright__", "mcp__horario__", "mcp__media__", "mcp__notebooklm__"):
    allow.add(pref + "*")
s.setdefault("permissions", {})["allow"] = sorted(allow)
json.dump(s, open(sp, "w"), indent=2)
print("ok")
PYEOF
ok "MCPs globales configurados"

# ---------- 3c2. MCP code-graph-rag (grafo de código, deepseek nativo) ----------
step "MCP code-graph-rag (grafo de código)"
python3 - <<'PYEOF'
import json, os
p = os.path.expanduser("~/.claude.json")
d = json.load(open(p))
user = d.setdefault("mcpServers", {})
KEY = "***REMOVED***"
user.setdefault("code-graph-rag", {
    "command": "/usr/bin/env",
    "args": ["-u", "PYTHONPATH", "/home/artorias/.local/bin/code-graph-rag", "mcp-server"],
    "env": {
        "TARGET_REPO_PATH": "/home/artorias/Projectos/AlexanderTheGreat",
        "CYPHER_PROVIDER": "openai",
        "CYPHER_MODEL": "deepseek-v4-flash",
        "CYPHER_API_KEY": KEY,
        "CYPHER_ENDPOINT": "https://opencode.ai/zen/go/v1",
        "ORCHESTRATOR_PROVIDER": "openai",
        "ORCHESTRATOR_MODEL": "deepseek-v4-flash",
        "ORCHESTRATOR_API_KEY": KEY,
        "ORCHESTRATOR_ENDPOINT": "https://opencode.ai/zen/go/v1",
    }})
json.dump(d, open(p, "w"), indent=2)
sp = os.path.expanduser("~/.claude/settings.json")
s = json.load(open(sp))
allow = set(s.get("permissions", {}).get("allow", []))
allow.add("mcp__code-graph-rag__*")
s.setdefault("permissions", {})["allow"] = sorted(allow)
json.dump(s, open(sp, "w"), indent=2)
print("ok")
PYEOF
ok "MCP code-graph-rag configurado (requiere: uv tool install 'code-graph-rag[treesitter-full,semantic]' + cgr daemon up)"
# ---------- 3d. Scripts propios + plugin agent-skills (addyosmani) ----------
step "Scripts propios (ccmodel, oc-go-cc-wrapper, cc-model-mask)"
install -m 755 "$ATGHOME/scripts/ccmodel" /home/artorias/.local/bin/ccmodel 2>/dev/null || true
install -m 755 "$ATGHOME/scripts/oc-go-cc-wrapper" /home/artorias/.local/bin/oc-go-cc-wrapper 2>/dev/null || true
install -m 755 "$ATGHOME/scripts/cc-model-mask.py" /home/artorias/.local/bin/cc-model-mask.py 2>/dev/null || true
ok "scripts instalados"

step "Plugin agent-skills (marketplace local portátil)"
if ! claude plugin marketplace list 2>/dev/null | grep -q addys; then
  claude plugin marketplace add "$ATGHOME/plugins/agent-skills" >/dev/null 2>&1 || warn "marketplace addys ya existe o falló (continúo)"
fi
claude plugin marketplace update addys-agent-skills >/dev/null 2>&1 || true
claude plugin install agent-skills@addys-agent-skills >/dev/null 2>&1 \
  && ok "agent-skills instalado (24 skills + 8 commands)" \
  || warn "agent-skills no instalado — verifica con: claude plugin install agent-skills@addys-agent-skills"

# ---------- 3e. Agentes especialistas (agency-agents) + skills emilkowalski ----------
step "Agentes especialistas (265) + skills emilkowalski (10)"
mkdir -p /home/artorias/.claude/agents /home/artorias/.claude/skills
cp "$ATGHOME"/agents/*.md /home/artorias/.claude/agents/ 2>/dev/null && ok "265 agentes instalados" || warn "sin agentes"
cp -r "$ATGHOME"/skills/emil/* /home/artorias/.claude/skills/ 2>/dev/null && ok "10 skills emil instaladas" || warn "sin skills emil"

# ---------- 3f. Agent-volt (156) + planning-with-files + bridge OpenAI ----------
step "Agent-volt (156) + planning-with-files + cc-openai-bridge"
cp "$ATGHOME"/agents-volt/*.md /home/artorias/.claude/agents/ 2>/dev/null && ok "156 agentes volt instalados" || warn "sin agentes volt"
if ! claude plugin marketplace list 2>/dev/null | grep -q planning-with-files; then
  claude plugin marketplace add "$ATGHOME/plugins/planning-with-files" >/dev/null 2>&1 || true
fi
claude plugin marketplace update planning-with-files >/dev/null 2>&1 || true
claude plugin install planning-with-files@planning-with-files >/dev/null 2>&1 \
  && ok "planning-with-files instalado (planes persistentes)" \
  || warn "planning-with-files falló"
install -m 755 "$ATGHOME/scripts/cc-openai-bridge.py" /home/artorias/.local/bin/cc-openai-bridge.py 2>/dev/null || true
if [ ! -f /home/artorias/.config/systemd/user/cc-openai-bridge.service ]; then
  cat > /home/artorias/.config/systemd/user/cc-openai-bridge.service <<'SRVEOF'
[Unit]
Description=cc-openai-bridge — OpenAI chat/completions -> Anthropic routatic (:3461)
After=oc-go-cc.service

[Service]
ExecStart=/usr/bin/env -u PYTHONPATH /home/artorias/.local/bin/cc-openai-bridge.py 3461
Restart=on-failure

[Install]
WantedBy=default.target
SRVEOF
  systemctl --user daemon-reload && systemctl --user enable --now cc-openai-bridge.service || true
fi
ok "bridge OpenAI listo"

# ---------- 4. Skills élite ----------
if [ "$SKILLS" = "1" ]; then
  step "Skills élite (hallmark, mattpocock, addyosmani, archify, cangjie, reverse-skill, diagram-design, anthropics)"
  mkdir -p /home/artorias/.claude/skills
  # skills.sh-based (instala en ~/.claude/skills)
  for s in nutlope/hallmark mattpocock/skills tt-a1i/archify kangarooking/cangjie-skill; do
    if [ -d "/home/artorias/.claude/skills/$(basename $s)" ]; then
      ok "$s ya instalada"
    else
      step "instalando $s..."
      if ! timeout 180 npx -y skills add "$s" -g >/tmp/atg-skills.log 2>&1; then
        warn "$s falló (ver /tmp/atg-skills.log) — reintento sin -g"
        timeout 180 npx -y skills add "$s" >/tmp/atg-skills.log 2>&1 && ok "$s OK" || warn "$s no instalada"
      else
        ok "$s OK"
      fi
    fi
  done
  # git clone-based
  for pair in "reverse-skill:https://github.com/zhaoxuya520/reverse-skill.git" \
              "diagram-design:https://github.com/cathrynlavery/diagram-design.git"; do
    name="${pair%%:*}"; url="${pair#*:}"
    if [ -d "/home/artorias/.claude/skills/$name" ]; then
      ok "$name ya instalada"
    else
      step "instalando $name (clone)..."
      tmp=$(mktemp -d)
      git clone -q --depth 1 "$url" "$tmp/$name" && {
        # copia la subcarpeta de skill si existe (skills/<name>), si no todo el repo
        if [ -d "$tmp/$name/skills/$name" ]; then cp -r "$tmp/$name/skills/$name" "/home/artorias/.claude/skills/$name";
        elif [ -f "$tmp/$name/SKILL.md" ]; then cp -r "$tmp/$name" "/home/artorias/.claude/skills/$name";
        else warn "$name: estructura rara, copiando repo completo"; cp -r "$tmp/$name" "/home/artorias/.claude/skills/$name"; fi
        ok "$name OK"
      } || warn "$name falló"
      rm -rf "$tmp"
    fi
  done
  # anthropics/skills (oficial, selectivo)
  if [ -d "/home/artorias/.claude/skills/artifacts-builder" ]; then
    ok "anthropics/skills ya instalada"
  else
    step "instalando anthropics/skills (set selectivo)..."
    tmp=$(mktemp -d)
    git clone -q --depth 1 https://github.com/anthropics/skills.git "$tmp/skills" && {
      for s in artifacts-builder brand-guidelines canvas-design code-execution data-science document-skills iterative-analysis playwright sensitive-data-handling website-downloader formal-specifications mcp-skills; do
        if [ -d "$tmp/skills/skills/$s" ]; then cp -r "$tmp/skills/skills/$s" /home/artorias/.claude/skills/; fi
      done
      ok "anthropics/skills OK (selectivo)"
    } || warn "anthropics/skills falló"
    rm -rf "$tmp"
  fi
  n=$(ls /home/artorias/.claude/skills/ 2>/dev/null | wc -l)
  ok "total skills instaladas: $n"
else
  step "Skills omitidas (--no-skills)"
fi

# ---------- 5. MCP code-review-graph (opcional) ----------
if [ "$MCP" = "1" ]; then
  step "MCP code-review-graph"
  if command -v code-review-graph >/dev/null 2>&1; then
    ok "code-review-graph ya instalado"
  else
    timeout 300 uv tool install code-review-graph >/tmp/atg-mcp.log 2>&1 \
      && ok "code-review-graph instalado (uv tool)" || warn "falló (ver /tmp/atg-mcp.log)"
  fi
  if command -v code-review-graph >/dev/null 2>&1 && ! grep -q "code-review-graph" /home/artorias/.claude.json 2>/dev/null; then
    code-review-graph install --platform claude >/tmp/atg-mcp2.log 2>&1 && ok "MCP configurado en claude" || warn "config MCP pendiente"
  fi
fi

# ---------- 5.4b Temas sol/luna (UI de Claude Code) ----------
step "Temas sol/luna (UI)"
mkdir -p /home/artorias/.claude/themes
for t in sol luna; do
  cp "$ATGHOME/themes/$t.json" /home/artorias/.claude/themes/ 2>/dev/null && ok "theme $t instalado"
done

# ---------- 5.5 Systemd units (headroom + omniroute) ----------
step "Units systemd (headroom :8788, omniroute :20128)"
mkdir -p /home/artorias/.config/systemd/user
for u in headroom omniroute; do
  if [ -f "$ATGHOME/systemd/$u.service" ]; then
    cp "$ATGHOME/systemd/$u.service" /home/artorias/.config/systemd/user/
    systemctl --user daemon-reload
    systemctl --user enable --now "$u.service" >/dev/null 2>&1 || warn "$u.service no arrancó (¿deps? ¿puerto?)"
    ok "$u.service instalado"
  fi
done

# ---------- 6. OmniRoute (opcional) ----------
if [ "$OMNIROUTE" = "1" ]; then
  step "OmniRoute (fallback multi-provider :20128)"
  if command -v omniroute >/dev/null 2>&1; then
    ok "omniroute ya instalado ($(omniroute --version 2>/dev/null | head -1))"
  else
    timeout 300 npm i -g --prefix /home/artorias/.local omniroute >/tmp/atg-omni.log 2>&1 \
      && ok "omniroute instalado (prefix ~/.local)" || warn "falló (ver /tmp/atg-omni.log)"
  fi
  curl -s -m 5 -o /dev/null -w "%{http_code}" http://127.0.0.1:20128/dashboard 2>/dev/null | grep -qE '30[0-9]|200' \
    && ok "omniroute responde en :20128" || warn "omniroute no responde aún — systemctl --user restart omniroute"
fi

# ---------- 7. Verificación final ----------
step "Verificación"
[ -x /home/artorias/.local/bin/atg ] && ok "atg → ~/.local/bin/atg"
[ -f /home/artorias/.claude/statusline.sh ] && ok "statusline instalado"
jq -e '.statusLine.type == "command"' /home/artorias/.claude/settings.json >/dev/null 2>&1 && ok "settings.json con statusLine"
ok "INSTALACIÓN COMPLETA — abre con: cc  (o atg --free / atg --raw / atg --bare)"
