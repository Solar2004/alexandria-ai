#!/usr/bin/env python3
"""Genera agent-index.json para el plugin alexander-harness.
Escanea ~/.claude/agents/*.md (los 421) y extrae name + description corta + categoria.
El hook agent-dispatch.sh usa este indice para seleccionar el mejor agente por tarea."""
import os, re, json, glob

SRC = "/home/artorias/.claude/agents"
OUT = "/home/artorias/Projectos/AlexanderTheGreat/plugins/alexander-harness/agent-index.json"

agents = []
for f in sorted(glob.glob(os.path.join(SRC, "*.md"))):
    txt = open(f, encoding="utf-8", errors="ignore").read()
    if not txt.startswith("---"):
        continue
    parts = txt.split("---", 2)
    if len(parts) < 3:
        continue
    fm = parts[1]
    name = re.search(r"^name:\s*(\S+)", fm, re.M)
    desc = re.search(r'^description:\s*"([^"]*)"', fm, re.M)
    if not name:
        continue
    n = name.group(1).strip()
    d = (desc.group(1).strip() if desc else "").lower()
    cat = n.split("-")[0] if "-" in n else "general"
    agents.append({"name": n, "category": cat, "description": d})

data = {
    "generated": "2026-08-12",
    "total": len(agents),
    "categories": sorted({a["category"] for a in agents}),
    "agents": agents,
}
with open(OUT, "w", encoding="utf-8") as f:
    json.dump(data, f, ensure_ascii=False, separators=(",", ":"))
print(f"indice generado: {len(agents)} agentes, {len(data['categories'])} categorias -> {OUT}")
print("categorias:", data["categories"])
