# 16 · Harness Evolutivo — `alx-evolve`

> La AI crea harnesses en tiempo real. **Self-evolución**: mientras el sistema trabaja, se construye a sí mismo — detecta qué formalizar, crea harnesses, los documenta, los aplica, y los promueve o destruye según sirvan. Nada se escapa sin documentación.

## 1. La idea (R20–R23)

Fallo actual: los harnesses/skills/reglas los define un humano por adelantado. La AI ejecuta lo que le dieron, y lo que aprende en el trabajo se pierde.

Solución: **alx-evolve** — la AI (el motor) detecta en pleno trabajo qué reglas, patrones, objetivos o convenciones merecen formalizarse como harness, los crea sobre la marcha, les pone documentación mínima, los aplica, y decide su destino: **temporal** (muere al cumplir su objetivo) o **permanente** (sirve al proyecto y se queda).

## 2. Lifecycle de un harness

```
1. DETECTAR   → en pleno trabajo (PostToolUse, fase), surge una regla/objetivo a formalizar
2. CREAR      → alx-evolve genera el Harness (spec mínima)
3. DOCUMENTAR → doc-min obligatoria: nombre, propósito, trigger, verificación (nada se escapa)
4. APLICAR    → el harness corre donde debe (gate, hook, critic)
5. VIGILAR    → el watcher de objetivos revisa: ¿cumplió su objetivo?
6a. DESTRUIR  → temporal + objetivo cumplido → se retira (Retired) y se archiva
6b. PROMOVER  → demostró servir → pasa a permanente para el proyecto
7. MEJORAR    → con datos de uso, el critic refine el harness (reglas más precisas)
```

## 3. Estructura del Harness

```rust
enum HarnessKind { Temporal, Permanent }
enum HarnessState { Active, WaitingObjective, Retired, Promoted }

struct Harness {
    id: AlxId,                 // "hx-<slug>"
    name: String,
    kind: HarnessKind,
    /// Cuándo corre: evento del bus, fase del pipeline, o comando.
    trigger: Trigger,
    /// Qué verifica (comando(s) o regla determinista). Vacío = regla semántica.
    verify: Option<GateSpec>,
    /// Objetivo. Para Temporal: condición de cumplimiento → autodestrucción.
    objective: String,
    /// Documentación mínima obligatoria (prosa corta: qué, por qué, cuándo).
    doc: String,
    state: HarnessState,
    created_by: AlxId,         // el agente/harness que lo creó
    created: u64,
    uses: u32,                 // nº de veces que se aplicó (para decidir promover/retirar)
}

enum Trigger {
    Event(EventKind),          // PostToolUse, PhasePassed, ...
    Phase(PhaseId),
    Manual,
}
```

## 4. El watcher de objetivos (autodestrucción)

Un harness vigilante (también generado) revisa a los demás:

```
Watcher corre tras cada PhasePassed o Stop
  → para cada Harness Temporal en WaitingObjective:
      → verifica objective (¿se cumplió?)
        → SÍ  → Retired + archivo en harnesses/archive/
        → NO  → si uses > UMBRAL sin cumplirse → aviso "harness zombie" → decide: retirar o redefinir
```

Reglas:
- **Temporal sin cumplir tras N aplicaciones** = harness zombie → se retira con diagnóstico (no se acumula basura).
- **Temporal que demostró servir** (evitó errores repetidos) → el critic lo propone a **permanente**.
- **Permanente** solo muere si el critic demuestra que ya no aplica (cambio de dirección del proyecto).

## 5. Documentación mínima obligatoria (doc-min)

**Regla de oro**: todo lo que la AI crea (código, harness, skill, config) lleva doc-min en el momento de crearse. El harness `doc-min` corre en PostToolUse:

```
PostToolUse (Edit|Write) → doc-min verifica:
  → ¿el archivo creado/modificado tiene doc de cabecera? (función/struct/comando)
  → ¿el harness nuevo tiene `doc` no vacía? (qué, por qué, cuándo)
  → si falta → Recall al autor + complementa con la mínima necesaria
```

Resultado: **el sistema se auto-documenta mientras se construye**. Nada se escapa.

## 6. Integración con critic y memoria

- Los harnesses alimentan `must_check` del critic (plan/15): una regla permanente (ej: "usa `<` no `<=`") se vuelve check automático.
- Los Recalls de `alx-memory` son candidatos a harness: si un aprendizaje evita errores repetidos, `alx-evolve` lo formaliza como harness.
- El critic revisa los harnesses nuevos (¿la verificación es correcta? ¿el objetivo es alcanzable?).

## 7. Ejemplo del usuario — harness de diseño

La AI construye una web. Detecta (fase Build) que los colores/tipografías se inventan por nodo. `alx-evolve` crea:

```rust
Harness {
    name: "design-system",
    kind: Permanent,
    trigger: Phase(Build),
    verify: Some(gate "paleta = tokens/colors; tipografía = tokens/fonts; sin hex literales"),
    objective: "consistencia visual del proyecto",
    doc: "Toda UI usa tokens de diseño (colors/fonts/spacing). Sin valores hardcodeados.",
    state: Active,
    ...
}
```

Cada componente nuevo se verifica contra él. Si un PR usa `#F00` hardcodeado → gate lo rechaza con evidencia. El harness se refine con cada iteración (el critic añade reglas nuevas que descubre). **Eso es "harness específico de diseño establecido".**

## 8. Registry

```
proyecto-final/harnesses/
├── active/         # harnesses vivos (permanentes + temporales en curso)
│   ├── design-system.toml
│   └── ...
├── archive/        # retirados (temporales cumplidos + zombies) — historial, no ruido
└── index.toml      # registry: id, nombre, kind, state
```

`alx-audit` valida los harnesses (schema, doc-min, trigger válido) como parte del doctor.

## 9. Iteraciones de mejora (R23)

El harness evolutivo también se mejora a sí mismo:
- **Datos de uso**: `uses`, nº de rechazos que evitó, nº de falsos positivos.
- **Critic refine**: si un harness rechaza 3 veces seguido algo que era correcto (falso positivo) → el critic ajusta la regla.
- **Auto-poda**: harnesses sin uso en N días → el watcher los propone a retirar.

## 10. Hooks del evolve

| Hook | Evento | Qué hace |
|---|---|---|
| `evolve.detect` | PostToolUse | Detecta candidatos a harness (regla/patrón/objetivo) |
| `evolve.create` | Detect | Genera el Harness + doc-min + lo registra |
| `evolve.watch` | PhasePassed/Stop | Watcher de objetivos: retirar/promover zombies |
| `docmin.verify` | PostToolUse Edit/Write | Verifica doc-min; complementa si falta |
| `evolve.refine` | PhaseFailed | Si una regla dio falso positivo → ajusta el harness |

## 11. Decisiones

- **Harness = datos** (`.toml` en `harnesses/active/`), no código hardcodeado. El motor los lee y ejecuta.
- **Temporal por defecto**: todo harness nuevo empieza temporal; solo se promueve con evidencia de utilidad.
- **Doc-min es compuerta**: un harness sin `doc` no se registra (gate lo rechaza). "Nada se escapa."
- **El watcher evita la basura**: sin autodestrucción, los harnesses temporales se acumulan como skills muertas.
- **Self-evolución real**: el sistema que se construye a sí mismo, documentándose y podándose solo.
