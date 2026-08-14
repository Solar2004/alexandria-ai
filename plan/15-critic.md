# 15 · Auto-Crítica por Código — `alx-critic` + Decomposition Engine

> El sistema se critica a sí mismo. No espera a que un humano diga "esto falló": después de cada fase, un crítico barato revisa la salida contra criterios, encuentra fallos Y mejoras, y el ejecutor itera. Bucle hasta que el crítico aprueba. El humano solo ve el resultado final pulido.

## 1. Por qué (R18)

El usuario: *"necesitamos hacer un sistema por código que ayude a la ai a ser su propia crítica y solita pueda hacer más iteraciones, mejorar ver errores, ver no solo errores sino mejorar lo que ya tiene sin que se lo digan... la ai siga y siga... que sean solo detalles y aun si continúe lustrándolo"*.

Fallo actual: la IA produce una vez y espera aprobación. Error si el humano no mira. Mejora no ocurre si el humano no la pide.

Solución: **alx-critic** = bucle de crítica ejecutado por código tras cada fase, que itera hasta que la salida cruza el umbral de calidad. Sin humano en el bucle.

## 2. El bucle de crítica

```
Fase X produce artefacto
   ↓
Critic (T1 barato) revisa contra CRITERIOS
   ├── fallos → Feedback (bloqueante) → ejecutor corrige → re-critic
   ├── mejoras → Feedback (opcional) → ejecutor aplica si presupuesto lo permite
   └── aprueba → Evidence + avanza fase
```

Reglas:
- **Critic siempre** (T1 o T2 según fase; nunca T3 para ahorrar).
- **Máx iteraciones** (default 3). Si el crítico sigue bloqueando a la 4ª, escala a agente T3 con el historial completo de feedback (el crítico barato no puede, el caro decide).
- **Feedback estructurado**: cada hallazgo con `severity (blocker/major/minor/suggestion)`, `evidence` (línea, comando), `fix_hint`.
- **Critic-report** persistido en `state/critics/<task>/<phase>.jsonl` — evidencia del pulido.

## 3. Criterios de crítica (por fase)

```rust
struct CriticCriteria {
    phase: PhaseId,
    rules: Vec<Rule>,   // ej: "spec requisitos son testables", "build no tiene warnings"
    thresholds: Vec<Threshold>,  // ej: "coverage >= 80", "complexity <= 20"
    must_check: Vec<Check>,      // ej: "no hardcodea secrets", "usa < no <="
}
```

Los criterios vienen de `phalanx/critics.toml` (config, no código). Combinan:
- Reglas genéricas (harness de calidad: evidencia, no inventar, tests).
- Reglas por fase (spec→testable, build→compila, review→sin blocker).
- Reglas aprendidas: los Recalls de `alx-memory` se convierten en `must_check` (ej: *"auth usa `<` no `<=`"* → check automático).

**Ese es el truco clave**: lo que la AI aprende de sus errores se convierte en check del crítico. Así el sistema mejora su propia crítica — *"mejorar lo que ya tiene sin que se lo digan"*.

## 4. Critic técnico (reglas, no opinión)

Dos capas:
1. **Reglas deterministas** (Rust, cero tokens): grep de secrets, `cargo clippy` warnings, cobertura, complejidad ciclomática, archivos sin test, diffs que tocan fuera del repo.
2. **Reglas semánticas** (agente T1): lee la salida y aplica criterios que requieren juicio (¿el spec es ambiguo? ¿la fix contradice la spec?).

Orden: determinista primero (barato y exacto), semántico después (solo si el determinista no bloquea).

## 5. Decomposition engine (R17 — "imposible que falle")

El harness rompe tareas grandes en micro-tareas antes de ejecutar:

```rust
struct MicroTask {
    id: AlxId,
    parent: AlxId,
    step: String,          // un paso atómico ("renombrar variable X en archivo Y")
    assert: String,        // cómo se verifica ESTE paso ("grep X ya no existe en Y")
    tools: Vec<String>,    // tools mínimas
    done_when: GateSpec,   // comando que prueba el paso
}
```

Reglas de descomposición:
- **Atomicidad**: un paso = un cambio verificable. Si el paso tiene 2 asserts, se divide en 2.
- **Assert por paso**: cada micro-tarea lleva su `done_when`. El gate corre por micro-tarea, no solo por fase.
- **Contexto mínimo**: cada micro-tarea se ejecuta como sesión headless con el contexto que necesita (R19 — la idea de ideas.md). El agente padre "summona" sub-agentes con el contexto relevante.
- **Fallar barato**: si la micro-tarea 3 de 40 falla, se reintenta SOLO la 3, no toda la feature. El resto del DAG sigue.

Ejemplo: "añadir endpoint de login" →
```
t1: crear schema de users (assert: migration aplica)
t2: escribir endpoint POST /login (assert: cargo build pasa, ruta existe)
t3: validar input (assert: tests de validación verdes)
t4: auth con `<` correcto (assert: test unitario cubre token expirado)
```

## 6. Sesiones headless simples (R19, de ideas.md)

El usuario: *"lanzar sesiones simples osea tareas simples usando claude headless solo se le pasa la tarea y agentes... un agente puede summonar a otro darle el contexto que ve necesario... ahorramos tokens"*.

Implementación en `alx-agent`:
- `alx agent headless --task "renombrar X" --context <envelope mínimo>` → lanza Claude Code headless con presupuesto T1 y un solo objetivo.
- El agente padre decide qué contexto pasar (envelope mínimo, no todo).
- El sub-agente devuelve: resultado + evidencia (diff, test) + `done_when` verificado.
- El padre integra; si falla, relega con feedback.
- Hook de harness: `headless.spawn` automatiza esto — cuando una fase detecta micro-tareas independientes, las reparte como sesiones headless en paralelo.

## 7. Hooks del critic

| Hook | Evento | Qué hace |
|---|---|---|
| `critic.run` | PhasePassed | Corre el crítico de la fase; si bloquea → reabre la fase (no avanza) |
| `critic.learn` | PostToolUse | Si una tool falló y luego se corrigió → genera `must_check` para futuros critics |
| `critic.escalate` | CriticMaxIterations | Sube a agente T3 con historial de feedback |
| `decompose.run` | TaskCreated | Descompone la tarea grande en micro-tareas antes de ejecutar |
| `headless.spawn` | MicroTaskReady | Lanza micro-tareas independientes como sesiones headless paralelas |

## 8. Iteration loop por hook (R24)

El problema real detectado por el usuario: *"la AI por código finaliza de trabajar y entonces el hook le diría vuelve a trabajar, primera iteración... tú no sabes lo que es una iteración, haces 1 trabajo listo y no repites para ver."*

El critic loop (§2) itera DENTRO de una fase. Falta el **disparador por hook** que obliga a iterar CUALQUIER trabajo que la AI da por terminado.

```
Trabajo termina (Stop / TaskDone)
  → hook iterate.trigger:
      → lee state/iteration-state.toml (iter, max_iter, feedback[])
      → ¿el trabajo pasó el criterio (critic aprueba)?
          → SÍ → fin + informe (evidencia del pulido)
      → NO → ¿iter < max_iter?
          → iter += 1
          → emite IterateRequest(iter, feedback_acumulado)
          → la AI vuelve a trabajar CON el feedback de las iteraciones previas
      → iter == max_iter → fin con informe de N iteraciones
```

```rust
struct IterationState {
    task_id: AlxId,
    iter: u32,
    max_iter: u32,          // phalanx config, default 3
    feedback: Vec<String>,  // acumulado: qué falló / qué mejorar (cada iteración)
    passed: bool,           // el critic aprobó
}
```

Reglas:
- **El hook no deja "terminar" sin iterar**: todo trabajo que produjo artefactos y no pasó verificación entra al loop.
- **Iteración ≠ re-hacer**: iteración = verificar + criticar + MEJORAR con el feedback previo.
- **Feedback acumulado**: la iteración N recibe los feedbacks 1..N-1 — no repite a ciegas, converge.
- **max_iter configurable** (phalanx config, default 3). Al agotar → informe con historial de iteraciones.
- **Escalada**: si tras max_iter el critic sigue bloqueando → escala a T3 (ver §2).

Hook:

| Hook | Evento | Qué hace |
|---|---|---|
| `iterate.trigger` | Stop / TaskDone | Fuerza iteración: lee/actualiza IterationState, emite IterateRequest con feedback acumulado |

## 9. Decisiones

- **Critic barato siempre**: criticar con T3 cuesta tanto como la propia fase. T1/T2 con reglas deterministas primero.
- **El feedback es dato**: se persiste y alimenta la memoria; el sistema aprende de su propia crítica.
- **Escalada controlada**: 3 iteraciones de critic barato, luego un T3 decide. Nunca bucle infinito.
- **Descomposición obligatoria**: toda tarea que el planificador estime > N tokens de fase se descompone. Pequeño = imposible que falle.
- **Iteración es la norma, no la excepción**: el hook `iterate.trigger` la hace automática. Un solo pase sin verificar = trabajo incompleto.
- **El humano ve el resultado pulido, no el proceso**: el bucle corre solo; los critic-reports e iteration-state quedan como evidencia.
