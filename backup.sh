#!/bin/bash
# ============================================================
#  AlexanderTheGreat — backup / restore TOTAL del stack
#  Uso: ./backup.sh            → crea backups/atg-<fecha>.tar.gz
#       ./backup.sh --restore  → restaura el más reciente
#
#  Incluye TODO lo necesario para reconstruir el stack tras un
#  reset de PC (con la carpeta del proyecto + este tar):
#   - ~/.claude (settings, skills, plugins, themes, statusline)
#   - ~/.claude.json (MCPs globales + directorios de trust)
#   - ~/.config (headroom/env, routatic-proxy, oc-go-cc keys,
#                ccstatusline, systemd user services)
#   - ~/.local/bin scripts propios (routa, routa-gateway.py, ccmodel, wrapper)
#   - ~/.zshrc (aliases cc/claude/sub*)
#   - lista de paquetes instalados (uv tool + npm -g) para reinstalar
#  ⚠️  El tar contiene CLAVES (headroom, oc-go-cc). Guárdalo en
#      disco privado (externo cifrado). Nunca lo subas a un repo.
# ============================================================
set -euo pipefail
ATGHOME="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STAMP=$(date +%Y%m%d-%H%M%S)
OUT="$ATGHOME/backups/atg-$STAMP.tar.gz"
mkdir -p "$ATGHOME/backups"

if [ "${1:-}" = "--restore" ]; then
  LATEST=$(ls -t "$ATGHOME"/backups/atg-*.tar.gz 2>/dev/null | head -1)
  [ -n "${LATEST}" ] || { echo "no hay backups en $ATGHOME/backups/"; exit 1; }
  echo "Restaurando $LATEST ..."
  tar -xzf "$LATEST" -C /home/artorias
  # reactivar servicios systemd user
  systemctl --user daemon-reload 2>/dev/null || true
  for s in headroom oc-go-cc routa-gateway omniroute; do
    systemctl --user enable "$s.service" 2>/dev/null || true
  done
  echo "OK — config, claves y servicios restaurados."
  echo "Siguiente: reinstalar binarios con la lista en backups/stack.txt"
  echo "y ejecutar ./install.sh"
  exit 0
fi

# lista de paquetes instalados (para reinstalar tras reset) — con timeout,
# porque uv/npm pueden colgarse en entornos con HOME virtual
{
  echo "# AlexanderTheGreat — paquetes del stack $(date)"
  echo "# --- uv tools ---"
  timeout 20 uv tool list 2>/dev/null || echo "(uv tool list no respondió)"
  echo "# --- npm global ---"
  timeout 20 npm ls -g --depth=0 2>/dev/null || echo "(npm ls no respondió)"
} > /tmp/atg-stack.txt

tar -czf "$OUT" \
  -C /home/artorias \
  .claude/settings.json \
  .claude.json \
  .claude/skills \
  .claude/plugins \
  .claude/themes \
  .claude/statusline.sh \
  .config/headroom/env \
  .config/routatic-proxy \
  .config/oc-go-cc \
  .config/ccstatusline \
  .config/systemd/user \
  .local/bin/routa \
  .local/bin/routa-gateway.py \
  .local/bin/ccmodel \
  .local/bin/oc-go-cc-wrapper \
  .local/bin/hermes-perplexity-mcp \
  .zshrc \
  CLAUDE.md \
  -C /tmp atg-stack.txt \
  2>/dev/null || {
  echo "ERROR: falló el tar (¿falta algún path?). Revisa la lista."
  exit 1
}

# copia de la lista dentro de backups/ para referencia rápida
cp /tmp/atg-stack.txt "$ATGHOME/backups/stack.txt" 2>/dev/null || true

echo "Backup: $OUT"
du -h "$OUT" | cut -f1 | xargs -I{} echo "Tamaño: {}"
echo ""
echo "Para reconstruir TODO tras un reset:"
echo "  1. copia esta carpeta (o el tar) a la PC nueva"
echo "  2. instala dependencias: node, uv, docker"
echo "  3. ./backup.sh --restore"
echo "  4. reinstala los paquetes de backups/stack.txt"
echo "  5. ./install.sh"
