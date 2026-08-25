#!/bin/bash
# research-guard.sh — hook Stop: si el proyecto tiene .alexandria/research/
# abierto, exige que cumpla el protocolo plan/17 ANTES de dejar cerrar.
#
# "No podemos dejar que la IA no lo haga o lo haga shallow" — este guard es
# la compuerta dura: alx research-check devuelve exit!=0 con los pasos vacíos
# y el hook reinyecta la obligación como razón de bloqueo (decision:block).
set -uo pipefail

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(pwd)}"
[ -d "$PROJECT_DIR/.alexandria/research" ] || exit 0

ALX="$HOME/.local/bin/alx"
command -v "$ALX" >/dev/null 2>&1 || ALX=alx

INFORME=$(cd "$PROJECT_DIR" && "$ALX" research-check 2>&1)
CODE=$?

if [ $CODE -eq 0 ]; then
    exit 0   # todo completo (o no hay research abierto)
fi

RAZON="${INFORME}

COMPLETA los pasos marcados en .alexandria/research/ antes de terminar:
rellena cada fichero siguiendo sus reglas (mecanismo → iceberg → simulaciones
guardadas → frenos → evidencia). Luego revalida: alx research-check"

python3 - "$RAZON" << 'PYEOF'
import json, sys
print(json.dumps({"decision": "block", "reason": sys.argv[1]}))
PYEOF
exit 0
