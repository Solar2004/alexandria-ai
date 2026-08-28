#!/bin/bash
# night-run.sh — modo autónomo nocturno: corre `alx night` y guarda el informe.
# Para cron/systemd: ejecutar a las 02:00 (NightSchedule del plan).
# Ejemplo cron: 0 2 * * * /home/artorias/Projectos/AlexanderTheGreat/scripts/night-run.sh

set -euo pipefail

# Ruta absoluta: systemd user no incluye ~/.local/bin en el PATH.
ALX=/home/artorias/.local/bin/alx
export PATH="/home/artorias/.local/bin:$PATH"   # routa y demás del informe
REPORT="/home/artorias/Projectos/AlexanderTheGreat/plan/night-report.md"

{
    echo "# Informe nocturno — $(date '+%Y-%m-%d %H:%M')"
    echo
    "${ALX}" night
    echo
    echo "--- Reporte completo del motor ---"
    "${ALX}" report
    echo
    echo "--- Resumen semanal ---"
    "${ALX}" weekly
} > "${REPORT}"

echo "Informe nocturno: ${REPORT}"

# --- Salud de la cadena (routa) ---
# Los modelos upstream caen sin avisar (muse 500, ox-alpha-free 500...).
# El informe nocturno debe revelarlo para que por la mañana sea obvio.
{
    echo
    echo "--- Salud de la cadena (routa) ---"
    if command -v routa >/dev/null 2>&1; then
        routa status || true
        echo
        routa doctor || true
    else
        echo "routa no disponible en PATH"
    fi
} >> "${REPORT}"
