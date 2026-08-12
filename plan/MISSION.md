# MISIÓN ALEXANDER — Memoria Permanente

> **Este archivo es la memoria maestra. RELEERLO al inicio de cada sesión y cada vez que haya duda de dirección.**
> No depende de comandos ni de que el usuario repita instrucciones. La AI se auto-recuerda solita.

## 1. Comando del usuario (verbatim, 2026-08-12)

> "Oye despliega multiples agentes planeemos el software definitivo de desarrollo usando Claude, le llamaremos Alexander y algo mas tipo crea el nombre tendras que crear el software mergear harnesses etc, todo en rust, un super plugin con todo sabes? optimizaciones gestion de tareas un workflow entero creado en rust con harnesses mcp todo que la ai con hooks etc etc super completo en cada fase del proyecto"

> "Aca tienes un proyecto que intento serlo pero es que no termina de serlo solo es un instalador y ya aparte aca nosotros nos enfocaremos en hacer hooks etc todo plugins todo conectado asegurarnos con testes que funciona que hace la tarea etc todo nosotros comprendido?" (ref: /home/artorias/Projectos/aicli-ultimate/)

> "trabajaras en esto autonomamente ok? osea queremos dejar de depender en comandos, en todo solo harness comprendes? skills harness un plugin en el que tu solo tengas tu propio sistema de todo donde te mejoras el harness de documentacion, de optimizacion de hablar, como estableces objetivos todo comprendes? solo 1 plugin para todo. todo queda bajo tu criterio eres un agente autonomo, nada de comandos todo sera automatico comprendes? la manera de documentar los pasos en planes, la manera de escribir codigo autocargarte lsp servers, todo lint, hacer el codigo optimizado aprobado matematicamente optimizado performance osea veras todo lo que podrias hacer y skills te permiten crearas conectaras con el workflow de todo el sistema completo comprendes? tendras que conectar todo crearlo a tu forma que se conecte se use etc para todo esto obviamente tendras que crear el plan de todo esto no crear un plan asi como asi tendras que para este mega plugin de harness darle el nombre crear el funcionamiento entero de harness hooking todo osea porque cada cosa tiene hooks que devs no usan pero siempre le repiten a la ai recuerdan solucionan cosas que dicen q si la ai se auto recordara solita etc comprendes? se que puedes hacerlo asi que vamos crea el mermaid architecture tendras que iterarlo por lo menos 20 veces. Recuerda eres autonomo guarda todo lo que te dije para que lo recuerdes a cada rato lo re leas para que pues puedas continuar haciendolo sin perderte"

## 2. Traducción a requisitos de ingeniería

| # | Requisito | Componente |
|---|---|---|
| R1 | Sistema autónomo definitivo de dev con IA | ALEXANDER (sistema global) |
| R2 | Motor en Rust, compilado, rápido | ALEXANDRIA (workspace de crates) |
| R3 | UN solo plugin para todo | PHALANX (mega-plugin harness) |
| R4 | Nada de comandos manuales — todo automático vía hooks | alx-hooks + hook catalog |
| R5 | Harness por fase del workflow (spec, plan, build, test, review, docs, ship) | alx-harness |
| R6 | La AI se auto-recuerda solita (sin repetirle) | alx-memory (auto-recalls) |
| R7 | Optimización de hablar (compresión, caveman, budget) | alx-governor |
| R8 | Establecimiento de objetivos automático | alx-governor (goal engine) |
| R9 | Autocargar LSP servers, lint, tests | alx-gate (auto-tooling) |
| R10 | Código óptimo "aprobado matemáticamente" (perf) | alx-bench (métricas, umbrales) |
| R11 | Conectar skills + workflow completo | alx-mcp (bus de integración) |
| R12 | Mermaid architecture iterado ≥20 veces | plan/iterations/ |
| R13 | Guardar memoria para no perderse | plan/MISSION.md + alx-memory |
| R14 | Autónomo, bajo mi criterio | night-ops protocol + governor |

## 3. Principios no negociables

1. **Evidencia verificable en cada fase.** Nada de "debería funcionar". Tests reales, output capturado.
2. **Barato y rápido por defecto.** Compresión entre agentes, modelo correcto para cada tarea, cero tokens desperdiciados.
3. **La AI se auto-recuerda.** El conocimiento repetido por el dev se captura en hooks y se re-inyecta solo.
4. **Un solo plugin, todo conectado.** PHALANX es el único punto de entrada.
5. **Autónomo.** Si algo es ambiguo: elige el supuesto, anótalo, sigue. No te detengas.
6. **Harness por fase.** Cada fase del workflow tiene su harness con contrato de entrada/salida y compuertas de verificación.
7. **Errores = datos.** Escribe qué falló y qué aprendiste, no borres el error.
8. **Cada cosa que el dev "siempre repite" es un hook que falta.**

## 4. Estado de la build (actualizar siempre)

- [x] Fase 0: Plan maestro + memoria (este archivo + plan/)
- [ ] Fase 1: Workspace Rust + tipos core + CLI skeleton
- [ ] Fase 2: alx-mcp (server de tools)
- [ ] Fase 3: alx-hooks (engine de eventos)
- [ ] Fase 4: alx-memory (auto-recalls)
- [ ] Fase 5: alx-governor (routing + compresión + presupuesto + objetivos)
- [ ] Fase 6: alx-harness (pipeline spec→ship)
- [ ] Fase 7: alx-task (DAG de tareas)
- [ ] Fase 8: alx-gate (verificación + LSP auto + lint)
- [ ] Fase 9: alx-bench (performance matemático)
- [ ] Fase 10: alx-night (modo autónomo)
- [ ] Fase 11: Integración total (atg, headroom, MCP existentes)
- [ ] Fase 12: Dogfood — usar ALEXANDER para construir una feature real
- [ ] Fase 13: Docs + packaging + instalador

## 5. Progreso de este intento (bitácora)

- 2026-08-12: Creación plan maestro. Auditados aicli-ultimate (solo instalador) y AlexanderTheGreat (harness disperso, sin motor). Decisión: ALEXANDER/ALEXANDRIA/PHALANX. Ver plan/.
