#!/bin/bash
# cgr-mcp-wrapper.sh — wrapper dinámico para code-graph-rag MCP.
# Lee el modelo ACTIVO de routatic en vivo (cambiar con `routa use <modelo>`)
# y lo inyecta como CYPHER/ORCHESTRATOR_MODEL. Así, al cambiar de modelo,
# code-graph-rag también usa muse spark sin reinstalar el plugin.
set -euo pipefail
ACTIVE="$(python3 -c "
import json, os
try:
    d=json.load(open(os.path.expanduser('~/.config/routatic-proxy/config.json')))
    m=d['models']['default']['model_id']
    # routatic guarda sin prefijo (muse-spark-1.2-contributor), pero el
    # bridge necesita opencode-go/ como prefijo para rutear
    if '/' not in m and m.startswith('muse-spark'):
        m='opencode-go/'+m
    print(m)
except: pass
" 2>/dev/null)"
if [ -n "${ACTIVE:-}" ]; then
  export CYPHER_MODEL="$ACTIVE"
  export ORCHESTRATOR_MODEL="$ACTIVE"
fi
exec /usr/bin/env -u PYTHONPATH /home/artorias/.local/bin/code-graph-rag mcp-server "$@"
