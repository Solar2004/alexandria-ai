# 17 · Integración orquestrator-package → ALEXANDRIA

> Auditoría de `/home/artorias/Projectos/orquestrator-package/` y qué aporta al sistema. La visión ALEXANDRIA absorbe el protocolo y lo convierte en motor; el orquestrator aporta el "cómo" concreto de varias ideas que ya estaban en el plan.

## 1. Qué es

Orquestrator = **modo delegación pura** para Claude Code SDD. No codea, planifica y delega vía `hcom` a workers. Dos roles: Orquestrador (lead) + Worker (executor).

## 2. Qué aporta (lo que sirve)

| Aporte | Detalle | Destino en ALEXANDRIA |
|---|---|---|
| **Dual-language protocol** | Orq→Worker en caveman wenyan-ultra; Worker→Orq en caveman; orq traduce. Human nunca ve caveman, worker nunca ve fluff | alx-governor (compresión entre agentes) — R7. Formaliza el "cómo" |
| **Verify handoff** | System prompt separado para agentes de verificación | alx-critic (el crítico con su propio envelope) |
| **Event-driven verify** | Worker done → verificar automáticamente contra spec | alx-harness PhasePassed → alx-critic.run |
| **Auto-kill idle worker** | Worker sin reporte >5min → kill (anti-zombie) | alx-hooks timeout/lock + alx-task timeout |
| **STATUS.md dashboard** | Tabla viva de tareas/estado/ETA | alx-task progress.md + informe |
| **SDD templates** | Concepts, Decisions, Plans, Walkthrough, SystemMap, spec-template | PHALANX plans/ (planning-with-files) |
| **Token strategy** | Auto-compact agresivo (16k), haiku para orq, `--intent inform` fire-and-forget | alx-governor (compact + tier + comunicaciones) |
| **Task/Agent handoff** | Handoff con placeholders por tarea | alx-agents (AgentEnvelope) |
| **Reminders por hook** | Hook UserPromptSubmit inyecta el reminder de modo cada turno | alx-hooks (phalanx.mission — ya en plan) |
| **SDD pipeline** | Pre-SDD → Archive flujo completo | alx-harness (Spec→Ship) |

## 3. Cómo ALEXANDRIA mejora el orquestrator

| Limitación del orquestrator | Cómo ALEXANDRIA la supera |
|---|---|
| Es skills/scripts bash sueltos, sin motor | 16 crates Rust compilados, estado persistente, tests |
| hcom = protocolo ad-hoc entre sesiones | event bus + DAG de tareas + MCP bus |
| "Duda → ask human" | alx-critic (se critica sola) + alx-evolve (aprende) |
| Sin verificación forzada | gate por fase + critic loop + iteration loop por hook |
| Duplicado con agent-skills/planning-with-files | registry único dedup (alx-audit) |
| Coste variable (1 orq + N workers sin presupuesto) | governor con budget por tarea + ledger |

## 4. El dual-language protocol formalizado (para alx-governor)

El orquestrator demuestra un protocolo que el governor debe implementar:

```
Orq → Worker (wenyan-ultra):  "signup.tsx email field. validate format+uniqueness."
Worker → Orq (caveman):       "task#1 done. docs/specs/email.md. code done."
```

Reglas para alx-governor (compresión):
1. **Orquestrador/líder** habla comprimido pero legible (caveman full) — necesita comunicar intención.
2. **Worker/ejecutor** recibe la tarea mínima exacta (wenyan-ultra) — solo lo que necesita.
3. **Informes** worker→líder ultra-comprimidos: qué, dónde, estado.
4. **Traducción humana**: el líder expande solo lo que el humano necesita ver (informes finales).
5. **`--intent inform` > `--intent request`**: comunicaciones fire-and-forget cuando no requieren respuesta (menos contexto pendiente).

## 5. Decisión

Integrar: el orquestrator-package se convierte en **PHALANX mode "orquestrator"** — una configuración del motor (no skills sueltas) que activa delegación pura con dual-language, verify automático, y los templates SDD. El motor es el orquestrador; los workers son `alx-agents` headless.
