#!/bin/bash
# night-run.sh — procesa la cola nocturna con Claude Code headless
set -uo pipefail
P=/home/artorias/Projectos/AlexanderTheGreat
Q="$P/night-ops/queue.md"
R="$P/night-ops/report/$(date +%Y-%m-%d).md"
export HOME=/home/artorias
export PATH="/home/artorias/.local/bin:$PATH"
mkdir -p "$P/night-ops/report"
cd "$P"
echo "# Informe nocturno $(date '+%Y-%m-%d %H:%M')" > "$R"

mapfile -t tasks < <(grep -n '^[[:space:]]*- \[ \]' "$Q" || true)
if [ ${#tasks[@]} -eq 0 ]; then
  echo "Sin tareas pendientes. Noche libre." >> "$R"
  exit 0
fi

done=0; fail=0
for line in "${tasks[@]}"; do
  n="${line%%:*}"; t="${line#*: }"
  echo "" >> "$R"
  echo "## Tarea $n: $t" >> "$R"
  log="/tmp/night-ops-$n.log"
  timeout 1500 claude -p "Ejecuta esta tarea usando la skill night-ops. Tarea: $t" --dangerously-skip-permissions > "$log" 2>&1
  rc=$?
  if [ $rc -eq 0 ]; then
    sed -i "${n}s/^\([[:space:]]*- \)\[ \]/\1[x]/" "$Q"
    echo "ESTADO: OK (rc=0)" >> "$R"
    tail -c 1500 "$log" >> "$R"
    done=$((done+1))
  else
    echo "ESTADO: FALLO (rc=$rc)" >> "$R"
    tail -c 800 "$log" >> "$R"
    fail=$((fail+1))
  fi
done
echo "" >> "$R"
echo "## Resumen: $done completadas, $fail fallidas" >> "$R"
