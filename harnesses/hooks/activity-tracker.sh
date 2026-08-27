#!/bin/bash
# activity-tracker — registra la actividad de Claude/ALEXANDRIA en vivo.
# Registrado en UserPromptSubmit, PreToolUse, PostToolUse, Notification y Stop.
# Escribe una línea JSON por evento en alexandria/state/activity.jsonl para que
# `alx watch` pinte los estados (codificando, buscando skill en GitHub,
# verificando, agente desplegado, planificando...) sin tocar al agente.
set -uo pipefail

REPO="/home/artorias/Projectos/AlexanderTheGreat"
LOG="$REPO/alexandria/state/activity.jsonl"

IN=$(cat)   # drena y parsea el JSON del hook
EV=$(printf '%s' "$IN" | python3 -c 'import json,sys;d=json.load(sys.stdin);print(d.get("hook_event_name",""))' 2>/dev/null)
SESSION=$(printf '%s' "$IN" | python3 -c 'import json,sys;d=json.load(sys.stdin);print(d.get("session_id","")[:8])' 2>/dev/null)
TOOL=$(printf '%s' "$IN" | python3 -c 'import json,sys;d=json.load(sys.stdin);print(d.get("tool_name",""))' 2>/dev/null)

# detalle según evento/herramienta (barato, solo strings del input)
DETAIL=$(printf '%s' "$IN" | python3 -c '
import json, sys
d = json.load(sys.stdin)
ev = d.get("hook_event_name", "")
ti = d.get("tool_input") or {}
if ev == "UserPromptSubmit":
    print(str(d.get("prompt", ""))[:90]); raise SystemExit
if ev == "Notification":
    print(str(d.get("message", ""))[:90]); raise SystemExit
tool = d.get("tool_name", "")
if tool == "Bash":
    print(str(ti.get("command", ""))[:90])
elif tool in ("Edit","MultiEdit","Write"):
    print(ti.get("file_path", ""))
elif tool == "Task":
    print("agente:", str(ti.get("description") or ti.get("agent_type") or "")[:60])
elif tool == "Skill":
    print(ti.get("skill", ""))
elif tool in ("WebFetch","WebSearch"):
    q = ti.get("query") or ti.get("url") or ""
    print(str(q)[:80])
elif tool == "TodoWrite":
    todos = ti.get("todos") or []
    cur = next((t.get("content","") for t in todos if t.get("status")=="in_progress"), "")
    print(("plan: " + str(cur))[:90] if cur else "plan actualizado")
else:
    print("")
' 2>/dev/null)

TS=$(date +%s%3N)
printf '{"ts":%s,"ev":"%s","tool":"%s","detail":"%s","session":"%s"}\n' \
  "$TS" \
  "$(printf '%s' "$EV" | tr -d '"')" \
  "$(printf '%s' "$TOOL" | tr -d '"')" \
  "$(printf '%s' "$DETAIL" | tr -d '"' | tr '\n' ' ')" \
  "$SESSION" >> "$LOG" 2>/dev/null

exit 0