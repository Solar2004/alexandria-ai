#!/bin/bash
# iterate.trigger (R24) — la AI no "termina" sin iterar.
# Hook UserPromptSubmit: inyecta el recordatorio de iteración en cada turno
# (anti-olvido). La AI itera sola: verifica -> critica -> mejora.
# El estado vive en state.toml (iter, max_iter, work_unit).

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STATE="$DIR/state.toml"

# Prompt real del usuario (no el reinyectado por auto-continue.sh) → resetea el
# contador de ciclos automaticos.
IN=$(cat)
case "$IN" in
    *"Continuemos con el proximo ciclo de iteracion"*) ;;
    *) rm -f "$DIR/.auto_cycles" ;;
esac

[ -f "$STATE" ] || { echo "[iterate] sin estado — crea $DIR/state.toml"; exit 0; }

ITER=$(grep -E '^iter' "$STATE" | head -1 | cut -d= -f2 | tr -d '"')
MAX=$(grep -E '^max_iter' "$STATE" | head -1 | cut -d= -f2 | tr -d '"')
WORK=$(grep -E '^work_unit' "$STATE" | head -1 | cut -d= -f2- | sed 's/^ *"//; s/"$//')

[ -z "$ITER" ] && exit 0

cat <<EOF

[ITERATE.trigger — R24]
Iteración $ITER/$MAX. Unidad actual: ${WORK:-<sin nombre>}
Regla: al terminar cualquier unidad de trabajo → VERIFICA (tests/evidencia real, no "debería funcionar") → CRITICA (qué falló, qué mejorar) → MEJORA. Un solo pase sin verificar = INCOMPLETO.
El estado (iter/work_unit) se AUTO-actualiza con cada commit (auto-iterate.sh) — NO lo edites a mano.
EOF
