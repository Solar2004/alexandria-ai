# 08 · Task Management — `alx-task`

> Las tareas como grafo acíclico dirigido (DAG) con estados, dependencias y persistencia. Integra planning-with-files: los planes en archivos son la capa legible; el DAG es la máquina de estados.

## 1. Estados y transiciones

```
Pending ──(deps ready)──► Ready ──(pick)──► InProgress ──(gate ok)──► Done
   │                        │                  │
   └──────► Blocked ◄───────┘                  ├──(gate fail)──► Ready (retry) / Failed
                        (deps rotas)           └──(skip)──────► Skipped
```

| Estado | Significado | Guarda de salida |
|---|---|---|
| Pending | creada, sin deps listas | todas las `depends_on` Done |
| Ready | ejecutable | runner la toma |
| InProgress | un agente la trabaja | — |
| Blocked | dep falló o falta recurso | dep resuelta o skip manual |
| Done | compuerta pasó + evidencia | evidence no vacía |
| Failed | agotó reintentos | diagnóstico en informe |
| Skipped | no aplica (skip_if) | condición registrada |

## 2. Goal engine (R8 — "cómo estableces objetivos")

El objetivo del usuario se descompone en DAG automáticamente:

1. **Ingest** convierte el prompt en `goal.md` (objetivo, alcance, no-metas).
2. `alx-task` descompone: requisitos → fases → tareas atómicas con `depends_on`.
3. Prioridad: `P0` (bloquea ship) → `P2` (nice-to-have). El governor asigna presupuesto por prioridad.
4. El DAG se persiste y se re-materializa en cada arranque. Reanudable a mitad.

## 3. API de tareas

```
alx task create <título> [--phase <fase>] [--depends <id>...] [--priority <p0..p2>]
alx task list [--status <st>] [--phase <fase>] [--tree]     # DAG en árbol
alx task show <id>                                          # detalle + evidencia
alx task deps <id>                                          # grafo de dependencias
alx task update <id> --status <st> [--note <txt>]
alx task retry <id>                                         # Ready de nuevo con feedback
alx task skip <id> --reason <txt>
alx task plan-from <goal.md>                                # genera DAG desde objetivo
```

## 4. Persistencia

- `state/tasks.jsonl` — append-only, una línea por cambio de tarea.
- `state/dag.dot` — export del DAG (Graphviz) para `alx task --tree` y para la TUI.
- En arranque: replay del JSONL reconstruye el DAG completo.

## 5. Integración con planning-with-files

- `plan/` en el repo = **capa legible por humanos**: `task_plan.md`, `findings.md`, `progress.md`.
- El DAG de alx = **capa máquina**: estados, deps, evidencia.
- Sincronización bidireccional:
  - `alx task plan-from goal.md` → genera/escribe `plan/task_plan.md` + registra tareas en el DAG.
  - `alx task show` → actualiza `progress.md` (checkboxes) al finalizar cada tarea.
  - El informe nocturno apunta a `progress.md` como fuente de verdad para el humano.

## 6. Priorización y cola

- El runner (`alx-harness`) toma tareas `Ready` por: prioridad desc, dependencia (tareas que desbloquean más → antes), coste estimado asc.
- **Workers**: max_threads del config (default 6). Tareas independientes corren en paralelo; las dependientes esperan.
- **Nada de starvation**: una tarea Ready llevaba >N ciclos → `night.maybe-schedule` la agenda para la pasada nocturna.

## 7. Decisiones

- **DAG, no lista**: el valor está en las dependencias; sin ellas, "gestión de tareas" es una lista de TODO.
- **Humanos leen planes, máquinas leen DAG**: las dos capas coexisten; la sincronización es automática.
- **Reanudable**: un crash a mitad de Build → el DAG se re-materializa, las tareas con evidencia quedan Done, el resto Ready.
- **Skip explícito con razón**: nada muere sin diagnóstico; todo fallo/skip queda en el informe.
