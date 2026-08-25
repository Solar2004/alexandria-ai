#!/bin/bash
# Stop hook — auto-continue: cuando Claude termina de hablar, en vez de parar
# reinyecta "Continuemos con el proximo ciclo de iteracion" automaticamente.
# Tope duro de ciclos consecutivos para no loopear infinito.
# Off: touch harnesses/iterate/.auto_off   |   Reset: rm harnesses/iterate/.auto_cycles

DIR="/home/artorias/Projectos/AlexanderTheGreat/harnesses/iterate"
STATE="$DIR/state.toml"
COUNT="$DIR/.auto_cycles"
MSG="Continuemos con el proximo ciclo de iteracion"

cat >/dev/null   # drena el JSON del hook

[ -f "$DIR/.auto_off" ] && exit 0
[ -f "$STATE" ] || exit 0

# awaiting_user=true → la AI terminó con una pregunta para el humano: NO
# forzar iteración, esperar la respuesta (el prompt real resetea el contador).
AW=$(grep -E '^awaiting_user' "$STATE" | head -1 | cut -d= -f2 | tr -d ' "')
if [ "$AW" = "true" ]; then
    exit 0
fi

# iter=0 → ciclo completado: el trabajo terminó y el hook se apaga SOLO
# (parada automática por código, sin contraseña). El próximo trabajo pone
# iter=1 y el hook vuelve a la vida.
ITER=$(grep -E '^iter' "$STATE" | head -1 | cut -d= -f2 | tr -d ' "')
if [ "$ITER" = "0" ]; then
    exit 0
fi

# target_iter declarado por la AI para este trabajo (idea del usuario); si no
# está, fallback a max_auto (tope duro).
MAX=$(grep -E '^target_iter' "$STATE" | head -1 | cut -d= -f2 | tr -d ' "')
case "$MAX" in ''|*[!0-9]*) MAX=$(grep -E '^max_auto' "$STATE" | head -1 | cut -d= -f2 | tr -d ' "');; esac
case "$MAX" in ''|*[!0-9]*) MAX=20;; esac

N=$(cat "$COUNT" 2>/dev/null)
case "$N" in ''|*[!0-9]*) N=0;; esac

if [ "$N" -ge "$MAX" ]; then
    echo "[auto-continue] tope $MAX ciclos alcanzado. rm $COUNT para seguir." >&2
    exit 0
fi

N=$((N + 1))
echo "$N" > "$COUNT"

printf '{"decision":"block","reason":"%s (auto %d/%d). Si la unidad de trabajo ya esta 100%% terminada y verificada, actualiza state.toml (iter+1 o nuevo work_unit) antes de seguir."}\n' \
    "$MSG" "$N" "$MAX"
