#!/bin/bash
# session-reset — cada sesión empieza limpia: iter=0 (contador de commits de
# ESTA sesión), last_commit=HEAD. Preserva max_iter/target (cap de la sesión).
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STATE="$DIR/state.toml"
[ -n "$ALX_SESSION_ID" ] && STATE="$DIR/state-$ALX_SESSION_ID.toml"
REPO="/home/artorias/Projectos/AlexanderTheGreat"
HEAD=$(git -C "$REPO" rev-parse --short HEAD 2>/dev/null || echo "")
MAX=$(grep -E '^max_iter' "$STATE" | head -1 | cut -d= -f2 | tr -d ' ' | grep -oP '\d+' || echo 20)
TARGET=$(grep -E '^target_iter' "$STATE" | head -1 | cut -d= -f2 | tr -d ' ' | grep -oP '\d+' || echo "$MAX")
cat > "$STATE" <<S
iter = 0
max_iter = $MAX
work_unit = "(sesión nueva — iteración por commit de trabajo real)"
max_auto = $TARGET
target_iter = $TARGET
awaiting_user = false
last_commit = "$HEAD"
S
exit 0
