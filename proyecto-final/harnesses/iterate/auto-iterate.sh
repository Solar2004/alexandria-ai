#!/bin/bash
# auto-iterate — state.toml se auto-avanza según trabajo REAL (git commits).
# Stop hook: si hay un commit nuevo → iter+1 y work_unit = mensaje del commit.
# El agente YA NO edita state.toml a mano: el sistema lo deriva del trabajo.
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STATE="$DIR/state.toml"
REPO="/home/artorias/Projectos/AlexanderTheGreat"
[ -f "$STATE" ] || exit 0

LAST=$(grep -E '^last_commit' "$STATE" | head -1 | cut -d= -f2 | tr -d ' "')
HEAD=$(git -C "$REPO" rev-parse --short HEAD 2>/dev/null || echo "")
[ -z "$HEAD" ] || [ "$LAST" = "$HEAD" ] && exit 0

ITER=$(grep -E '^iter' "$STATE" | head -1 | cut -d= -f2 | tr -d ' ')
ITER=$((ITER + 1))
MSG=$(git -C "$REPO" log -1 --pretty=%s 2>/dev/null | sed 's/"/\\"/g')

sed -i "s/^iter = .*/iter = $ITER/" "$STATE"
sed -i "s/^work_unit = .*/work_unit = \"$MSG\"/" "$STATE"
if grep -q '^last_commit' "$STATE"; then
  sed -i "s/^last_commit = .*/last_commit = \"$HEAD\"/" "$STATE"
else
  echo "last_commit = \"$HEAD\"" >> "$STATE"
fi
exit 0
