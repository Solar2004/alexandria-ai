---
description: Spawnea un subagente concreto con una tarea (genera la llamada Task lista)
argument-hint: <agente> <tarea>
---

# Spawn de agente

Genera y ejecuta la tool Task:

```
Task(agent_type="<agente>", description="<tarea autocontenida: objetivo + contexto + formato de salida>")
```

- El nombre del agente debe existir en el índice (verifica con `/agents` si dudas).
- La descripción DEBE ser autocontenida: el subagente no ve esta conversación.
- Pide salida verificable (rutas, comandos, resultados).
- Si el agente no existe: busca el más parecido en `agent-index.json` y úsalo, justificando el cambio.
