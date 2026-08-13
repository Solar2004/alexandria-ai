#!/bin/bash
# night-run.sh — modo autónomo nocturno: corre `alx night` y guarda el informe.
# Para cron/systemd: ejecutar a las 02:00 (NightSchedule del plan).
# Ejemplo cron: 0 2 * * * /home/artorias/Projectos/AlexanderTheGreat/proyecto-final/scripts/night-run.sh

set -euo pipefail

REPORT="/home/artorias/Projectos/AlexanderTheGreat/plan/night-report.md"

{
    echo "# Informe nocturno — $(date '+%Y-%m-%d %H:%M')"
    echo
    alx night
    echo
    echo "--- Reporte completo del motor ---"
    alx report
} > "${REPORT}"

echo "Informe nocturno: ${REPORT}"
