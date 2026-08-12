# 00 · Visión — ALEXANDER / ALEXANDRIA / PHALANX

## 1. Nombres

| Nombre | Rol |
|---|---|
| **ALEXANDER** | El sistema global. La conquista: software definitivo de desarrollo autónomo con IA. |
| **ALEXANDRIA** | El motor Rust. Ciudad del saber — la biblioteca que centraliza todo el conocimiento y las herramientas en un solo lugar. |
| **PHALANX** | El mega-plugin harness. La falange macedonia: una sola formación, cada lanza en su sitio, imparable. Es EL ÚNICO plugin. |

Motto: *"Una sola formación. Un solo reino."*

## 2. El problema que resuelve

Hoy el harness de Alexander está **disperso y manual**:

- Wrapper `atg` en bash, hooks en `.claude/hooks/`, 265+ agentes en markdown, skills en plugins, memory en `.remember/`, planners en `planning-with-files`, night-ops como skill suelta.
- **Cada cosa es un sistema aparte** que hay que invocar, recordar, mantener.
- El dev (Alexander) **repite las mismas instrucciones** a la AI cada sesión — porque no hay auto-memoria funcional.
- **Cada herramienta cuesta tokens y atención** — no hay gobernador de coste.
- **No hay verificación automática** — "debería funcionar" en vez de evidencia.

aicli-ultimate intentó empaquetar esto, pero es **solo un instalador**: configura CLIs, copia archivos, y se detiene. No hay motor, no hay hooks, no hay pipeline, no hay gobernador.

## 3. La solución en una frase

**PHALANX** es un único plugin que, montado sobre el motor Rust **ALEXANDRIA**, convierte a la AI en un sistema autónomo: se auto-recuerda, se auto-verifica, se auto-optimiza, ejecuta el workflow completo (spec → plan → build → test → review → docs → ship) sin comandos manuales, gastando el mínimo de tokens posible.

## 4. Requisitos del sistema (derivados del comando del usuario)

1. Un solo plugin para todo (R3).
2. Nada de comandos — todo automático vía hooks (R4).
3. La AI se auto-recuerda solita (R6).
4. Harness por fase, cada fase con compuertas de verificación (R5).
5. Auto-herramientas: LSP autocargado, lint, tests (R9).
6. Performance con aprobación matemática: métricas + umbrales (R10).
7. Motor en Rust: rápido, barato, un binario (R2).
8. Optimización del habla: compresión + routing de modelos + presupuesto (R7).
9. Conecta skills + workflow completo vía MCP (R11).
10. Autónomo bajo mi criterio (R14).

## 5. Principios no negociables

Ver `MISSION.md` §3. Resumen:

- **Evidencia > fe**: nada de "debería funcionar", todo verificado.
- **Barato por diseño**: la compresión y el routing no son opcionales, son la arquitectura.
- **Auto-memoria**: todo conocimiento repetido se captura en un hook, nunca se vuelve a pedir.
- **Un punto de entrada**: PHALANX. Todo lo demás es interna del motor.
- **Harness por fase**: contrato claro entrada→salida, compuerta de salida verificable.

## 6. Qué NO es

- No es un instalador (eso ya lo hizo aicli-ultimate).
- No es bash glue (eso ya lo hizo `atg`).
- No es un skill más suelto.
- No es una interfaz nueva — es el sistema nervioso que conecta lo que ya existe + lo que falta.

## 7. Métricas de éxito

- **Fases**: el pipeline completo corre de principio a fin sin intervención humana.
- **Coste**: ≥60% menos tokens que una sesión manual equivalente (medible por gobernador).
- **Verificación**: 100% de fases terminadas con evidencia de build/test/lint capturada.
- **Memoria**: cero repeticiones de instrucciones entre sesiones (la AI recuerda sola).
- **Performance**: código mergeable solo si pasa umbrales de `alx-bench`.
- **Dogfood**: ALEXANDER construye una feature real de principio a fin por sí mismo.
