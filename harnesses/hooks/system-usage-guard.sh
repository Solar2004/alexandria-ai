#!/bin/bash
# system-usage-guard — verifica que la IA usó los sistemas de Alexandria
# al cerrar una unidad de trabajo. Si no creó task, no consultó memoria/MCP
# o no consideró harnesses, bloquea el Stop y le inyecta qué falta.
# Hook Stop: solo actúa cuando hay trabajo real (iter>0) y no es un probe.
set -uo pipefail
DIR="/home/artorias/Projectos/AlexanderTheGreat/harnesses/iterate"
STATE="$DIR/state.toml"
[ -n "${ALX_SESSION_ID:-}" ] && STATE="$DIR/state-$ALX_SESSION_ID.toml"
[ -f "$STATE" ] || exit 0

ITER=$(grep -E '^iter' "$STATE" 2>/dev/null | head -1 | cut -d= -f2 | tr -d ' "')
[ "$ITER" = "0" ] || [ -z "$ITER" ] && exit 0

# Si el usuario preguntó algo (awaiting_user), no bloquear
AW=$(grep -E '^awaiting_user' "$STATE" 2>/dev/null | head -1 | cut -d= -f2 | tr -d ' "')
[ "$AW" = "true" ] && exit 0

IN=$(cat)
# No bloquear probes ni continuaciones automáticas
case "$IN" in
  *"Continuemos con el proximo ciclo"*) exit 0 ;;
esac

FALTAS=()
# 1. Tasks: ¿existe al menos una task persistida?
TASKS="/home/artorias/Projectos/AlexanderTheGreat/alexandria/state/tasks.jsonl"
if [ ! -s "$TASKS" ]; then
  FALTAS+=("alx task add \"<título>\" (no hay tasks registradas)")
fi
# 2. MCP: ¿se usó al menos una tool del motor vía MCP en esta sesión?
#    (activity log tiene tool=mcp o detail con phalanx/cost/memory)
ACT="/home/artorias/Projectos/AlexanderTheGreat/alexandria/state/activity.jsonl"
if [ -f "$ACT" ]; then
  if ! grep -qi "mcp\|phalanx\|cost_report\|memory.recall\|task\.list" "$ACT" 2>/dev/null; then
    FALTAS+=("alx mcp → usa phalanx.status / governor.cost_report / memory.recall (no se detectó uso de MCP en esta sesión)")
  fi
fi
# 3. Harness por proyecto: si hay .alexandria/ y 0 harnesses, sugerir (no bloquear, solo avisar)
# 4. Skills: si se activó una skill en esta sesión y no se marcó ningún paso,
#    el skill-check bloquea (exit 2) con el checklist.
SKILL_MSG=$(alx skill-check 2>/dev/null)
SKILL_RC=$?
if [ "$SKILL_RC" = "2" ]; then
  MSG_JSON=$(printf '%s' "$SKILL_MSG" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))')
  printf '{"decision":"block","reason":%s}\n' "$MSG_JSON"
  exit 0
fi

if [ ${#FALTAS[@]} -eq 0 ]; then
  exit 0
fi

MSG="SYSTEM-GUARD: trabajo sin usar sistemas de Alexandria. Falta: $(IFS='; '; echo "${FALTAS[*]}"). Ejecútalos ahora antes de cerrar — el guard volverá a verificar."
# Escapar para JSON
MSG_JSON=$(printf '%s' "$MSG" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))')
printf '{"decision":"block","reason":%s}\n' "$MSG_JSON"
