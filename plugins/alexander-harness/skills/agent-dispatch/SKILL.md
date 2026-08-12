---
name: agent-dispatch
description: Seleccion y spawn de subagentes. Usa cuando una tarea es delegable, cuando el dispatcher sugiere un agente, o cuando pides "spawnea", "delega" o "usa el agente X". 選賢與能 — elige al agente correcto y delega con criterio.
---

# Agent Dispatch — delegación con criterio

Tienes **421 subagentes especializados** indexados en `agent-index.json`. El dispatcher (hook) sugiere candidatos por prompt, pero la decisión final es tuya.

## Cuándo delegar
- Tarea grande, multi-paso o de dominio específico → DELEGA. No la hagas inline.
- Tarea trivial (1-2 pasos, sin contexto) → hazla tú.
- Regla: si existe un agente cuyo nombre/descripción encaja con la tarea, úsalo. 421 agentes cubren casi todo: marketing-*, engineering-*, security-*, design-*, academic-*, data-*, legal-*, health-*, finance-*, mlops, devops, frontend/backend, QA, research, writing…

## Cómo elegir
1. Lee el índice: `cat /home/artorias/Projectos/AlexanderTheGreat/plugins/alexander-harness/agent-index.json` (o busca: `grep -o '"name":"[^"]*"' <indice>`).
2. Cruza las palabras clave de la tarea contra nombres y descripciones.
3. Elige 1 agente principal (nunca más de 2-3 en paralelo salvo que la tarea lo exija).

## Cómo spawnea (tool Task)
```
Task(agent_type="<nombre-del-agente>", description="<objetivo preciso, contexto mínimo, formato de salida>")
```
- Descripción autocontenida: el subagente no ve esta conversación.
- Pide salida verificable (rutas, comandos, resultados reales).
- Si el agente falla: cambia de agente o hazlo tú — no bucles.

## Verificación
- Todo lo que devuelva un subagente: verifícalo (archivos existen, tests pasan, comandos reales).
- Errores = datos: anota qué falló y qué agente sirvió.
