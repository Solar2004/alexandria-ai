#!/bin/bash
# agent-dispatch.sh — selecciona el subagente adecuado para cada prompt.
# Lee el prompt de stdin (JSON), lo compara contra agent-index.json y emite
# la sugerencia como contexto adicional (el modelo spawnea con la tool Task).
# Salida vacia = sin sugerencia (no interfiere).
set -uo pipefail

PLUGIN_DIR="$(cd "$(dirname "$0")/.." && pwd)"
INDEX="$PLUGIN_DIR/agent-index.json"
[ -f "$INDEX" ] || exit 0

INPUT="$(cat)"
PROMPT="$(printf '%s' "$INPUT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("prompt",""))' 2>/dev/null)"
[ -n "$PROMPT" ] || exit 0

SUGGESTION="$(python3 - "$PROMPT" "$INDEX" <<'PYEOF'
import json, sys, re

prompt = sys.argv[1].lower()
idx = json.load(open(sys.argv[2], encoding="utf-8"))

STOP = {"de", "la", "el", "los", "las", "un", "una", "unos", "unas", "que", "como",
        "para", "con", "por", "del", "al", "y", "o", "e", "en", "es", "se", "su",
        "me", "te", "mi", "tu", "esto", "esta", "este", "quiere", "hacer", "puedes",
        "the", "a", "an", "of", "to", "in", "for", "on", "with", "and", "or", "is",
        "are", "can", "you", "please", "code", "file", "archivo", "por favor"}

SYN = {
    "seguridad": "security", "auditoria": "audit", "codigo": "code", "revisa": "review",
    "revisar": "review", "riesgo": "risk", "riesgos": "risk", "prueba": "test", "pruebas": "test",
    "diseno": "design", "disenar": "design", "desarrollo": "development", "base": "database",
    "datos": "data", "red": "network", "aplicacion": "application", "aplicaciones": "application",
    "web": "web", "movil": "mobile", "escritorio": "desktop", "documentacion": "documentation",
    "documentar": "documentation", "escribir": "writing", "articulo": "content", "marketing": "marketing",
    "ventas": "sales", "finanzas": "finance", "legal": "legal", "salud": "healthcare",
    "investigacion": "research", "investigar": "research", "aprender": "learn", "ensena": "teach",
    "refactorizar": "refactoring", "rendimiento": "performance", "optimizar": "optimization",
    "optimizacion": "optimization", "error": "error", "errores": "error", "bug": "bug",
    "api": "api", "base de datos": "database", "sql": "sql", "python": "python", "javascript": "javascript",
    "react": "react", "frontend": "frontend", "backend": "backend", "devops": "devops",
    "docker": "docker", "kubernetes": "kubernetes", "cloud": "cloud", "infraestructura": "infrastructure",
    "analizar": "analysis", "analisis": "analysis", "reporte": "report", "resumen": "summary",
    "traducir": "translation", "contenido": "content", "seo": "seo", "growth": "growth",
    "legal": "legal", "compliance": "compliance", "gdpr": "gdpr", "qa": "qa", "calidad": "quality",
    "arquitectura": "architect", "planificar": "planning", "plan": "planning", "deploy": "deployment",
    "desplegar": "deployment", "monitoreo": "monitoring", "monitorizar": "monitoring",
}

words = [w for w in re.findall(r"[a-z0-9]+", prompt) if len(w) > 3 and w not in STOP]
words += [SYN[w] for w in words if w in SYN]
words = list(dict.fromkeys(words))
if len(words) < 2:
    sys.exit(0)

scored = []
for a in idx["agents"]:
    d = a["description"]
    if not d:
        continue
    hit = sum(1 for w in words if w in d)
    # el nombre del agente tambien cuenta (react-expert -> "react")
    name_parts = set(a["name"].split("-"))
    hit += sum(1 for w in words if w in name_parts)
    if hit >= 2:
        scored.append((hit, a["name"], d))

if not scored:
    sys.exit(0)

scored.sort(reverse=True)
top = scored[:3]
lines = ["Seleccion de agente (dispatcher):"]
for hit, name, d in scored[:3]:
    lines.append(f"- {name} (score {hit}): {d[:120]}")
lines.append("Usa la tool Task con agent_type del primer candidato si encaja; si no, elige el mas apropiado y justifica.")
print("\n".join(lines))
PYEOF
)"

[ -n "$SUGGESTION" ] || exit 0

python3 - "$SUGGESTION" <<'PYEOF'
import json, sys
print(json.dumps({
    "hookSpecificOutput": {
        "hookEventName": "UserPromptSubmit",
        "additionalContext": sys.argv[1]
    }
}))
PYEOF
