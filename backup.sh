#!/bin/bash
# ============================================================
#  AlexanderTheGreat — backup / restore de la build
#  Uso: ./backup.sh            → crea backups/atg-<fecha>.tar.gz
#       ./backup.sh --restore  → restaura el más reciente
#  Incluye: ~/.claude (settings, skills, statusline, plugins),
#  ~/.config/headroom/env, lista de paquetes del stack.
# ============================================================
set -euo pipefail
ATGHOME="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STAMP=$(date +%Y%m%d-%H%M%S)
OUT="$ATGHOME/backups/atg-$STAMP.tar.gz"
mkdir -p "$ATGHOME/backups"

if [ "${1:-}" = "--restore" ]; then
  LATEST=$(ls -t "$ATGHOME"/backups/atg-*.tar.gz 2>/dev/null | head -1)
  [ -n "${LATEST}" ] || { echo "no hay backups"; exit 1; }
  echo "Restaurando $LATEST ..."
  tar -xzf "$LATEST" -C /home/artorias
  echo "OK — ~/.claude y ~/.config/headroom restaurados"
  exit 0
fi

# lista de paquetes del stack (para reinstalar tras reset)
{
  echo "# AlexanderTheGreat — stack instalado $(date)"
  echo "# reinstalar con: HOME=/home/artorias uv tool install 'headroom-ai[proxy,code]'"
  echo "#                HOME=/home/artorias uv tool install code-review-graph"
  echo "#                npm i -g omniroute"
} > /tmp/atg-stack.txt

tar -czf "$OUT" \
  -C /home/artorias \
  .claude/settings.json .claude/skills .claude/statusline.sh \
  .config/headroom/env \
  -C /tmp atg-stack.txt \
  2>/dev/null || tar -czf "$OUT" -C /home/artorias .claude/settings.json .claude/skills .claude/statusline.sh .config/headroom/env

echo "Backup: $OUT"
du -h "$OUT" | cut -f1 | xargs -I{} echo "Tamaño: {}"
echo "Restaurar: ./backup.sh --restore"
