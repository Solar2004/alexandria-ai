#!/bin/bash
# cgr-mcp-wrapper.sh — wrapper dinámico para code-graph-rag MCP.
# Modelo ACTIVO: ALX_MODEL > config heredado de routatic (si existe) >
# alias visible claude-opus-4-6[1m (token-proxy lo traduce a muse-spark).
# Antes se leía en vivo de routatic (`routa use`); routatic ya no existe.
set -euo pipefail
ACTIVE="${ALX_MODEL:-$(python3 -c "
import json, os
try:
    d=json.load(open(os.path.expanduser('~/.config/routatic-proxy/config.json')))
    m=d['models']['default']['model_id']
    # routatic guardaba sin prefijo (muse-spark-1.2-contributor), pero el
    # bridge necesita opencode-go/ como prefijo para rutear
    if '/' not in m and m.startswith('muse-spark'):
        m='opencode-go/'+m
    print(m)
except: print('claude-opus-4-6[1m]')
" 2>/dev/null)}"
if [ -n "${ACTIVE:-}" ]; then
  export CYPHER_MODEL="$ACTIVE"
  export ORCHESTRATOR_MODEL="$ACTIVE"
fi
exec /usr/bin/env -u PYTHONPATH /home/artorias/.local/bin/code-graph-rag mcp-server "$@"
