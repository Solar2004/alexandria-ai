Error para mandar al programador del harness
Error exacto:
UserPromptSubmit hook error
Failed with non-blocking status code:
/bin/sh: linha 1: ./hooks/agent-dispatch.sh: Ficheiro ou pasta inexistente
Origen: Plugin alexander-harness v0.1.0 instalado en ~/.claude/plugins/cache/alexander-harness/alexander-harness/0.1.0/
Archivo con el bug: .claude-plugin/plugin.json
"hooks": {
  "UserPromptSubmit": [
    { "hooks": [ { "type": "command", "command": "./hooks/agent-dispatch.sh", "timeout": 10 } ] }
  ]
}
El problema: el command usa una ruta relativa ./hooks/agent-dispatch.sh. Claude Code resuelve los comandos de hooks del plugin contra el directorio de trabajo del proyecto (CWD), NO contra el directorio del plugin. Entonces:
- Ejecutando en ~/Projectos/Centaury/ → busca ~/Projectos/Centaury/hooks/agent-dispatch.sh → no existe → error.
- El script real está en el plugin: ~/.claude/plugins/cache/alexander-harness/alexander-harness/0.1.0/hooks/agent-dispatch.sh (existe y es ejecutable).
Fix: usar ruta absoluta al script del plugin en plugin.json, o resolver el hook relativo al directorio del plugin:
"command": "/home/artorias/.claude/plugins/cache/alexander-harness/alexander-harness/0.1.0/hooks/agent-dispatch.sh"
Nota: es error no bloqueante (el hook falla pero Claude sigue), solo ensucia la salida. El script en sí funciona bien — es solo la ruta.
