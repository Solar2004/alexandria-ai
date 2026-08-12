# 06 · Agents System — `alx-agents`

> Registry de agentes: cargar 420+ agentes markdown, validarlos con schema, clasificarlos, rutearlos a fases, y ensamblar el prompt mínimo que necesitan. Nada de "usa este agente" manual.

## 1. Registry

- **Fuentes**: `agents/` (265), `agents-volt/` (~156), `.claude/agents/` (8), agent-skills agents (4).
- Al arrancar, `alx-agents` indexa todos los `.md` con frontmatter YAML.
- Cache de índice (`state/agents.index.json`) para no re-leer 420 archivos en cada arranque (hash de mtime).

## 2. Schema de agente (validación con `alx doctor`)

```yaml
---
name: engineering-code-reviewer
description: "Revisa diffs por bugs reales, fallos concretos, complejidad innecesaria"
tools: [Read, Grep, Glob, Bash, Edit]        # permitidos
tier: T3Premium                                # tier sugerido al gobernador
phase: Review                                  # fase donde se usa (opcional)
tags: [review, quality, security]
skip_if: ["diff vacío"]                        # condiciones de no-usar
---
```

**Validador** (`alx doctor agents`):
- frontmatter parseable YAML
- `name` único, slug válido
- `description` presente (≥ 20 chars) — sin descripción, el router no puede elegir
- `tools` ⊆ catálogo de tools de alx
- archivo no duplicado entre `agents/` y `agents-volt/`
- Referencias `[[wikilinks]]` dentro del md resuelven (a otro agente/doc)

## 3. Router de agentes

Reglas en `phalanx/router.toml`:

```toml
[phase.Build]
agents = ["incremental-implementation", "frontend-ui-engineering", "backend-developer"]
fallback = ["general-purpose"]

[phase.Review]
agents = ["code-review-and-quality", "security-and-hardening"]
must_run = ["security-and-hardening"]   # no se salta seguridad

[skill]
priority = ["skill-frontmatter-match", "description-match", "phase-match", "fallback"]
```

El router elige por: (1) match de skill/fase declarada, (2) similitud de descripción, (3) fase, (4) fallback.

## 4. Spawn de agente (prompt assembly)

**Principio: envelope mínimo.** El agente recibe solo lo que necesita — no todo el contexto.

```rust
struct AgentEnvelope {
    system: String,              // frontmatter + reglas globales (caveman, calidad)
    mission: String,             // extracto de MISSION.md relevante
    task: String,                // la tarea concreta (fase)
    inputs: Vec<Artifact>,       // artefactos de entrada (rutas, no contenido)
    memory: Vec<Recall>,         // recalls top-N (budget de memoria)
    tools: Vec<String>,          // tools permitidas
    budget: TokenBudget,
}
```

Pasos:
1. Governor clasifica la tarea → tier + ruta de modelo.
2. `alx-memory` aporta recalls relevantes (máx N tokens).
3. `alx-harness` pasa artefactos de la fase anterior (rutas).
4. Compresión del envelope (caveman) si el tamaño excede umbral.
5. Se lanza contra el host (Claude Code vía headroom, o codex, o ruta local).

## 5. Colaboración multi-agente

- `alx-task` orquesta: tareas paralelas → agentes paralelos (worker pool, max_threads del config).
- Resultados se consolidan como artefactos; dependencias del DAG gobiernan el orden.
- **Agente que falla su compuerta** → reintento con feedback del error (mismo envelope + error) o fallback a otro agente de la fase.

## 6. Decisiones

- **No reescribir agentes**: el valor está en el registry existente. `alx doctor agents` los valida; no los regenera.
- **Router configurable, no hardcodeado**: PHALANX decide la política (`router.toml`), el motor la ejecuta.
- **Agente sin descripción = no rut cable**: el validador lo marca, el router lo ignora, el informe lo lista.
- **Envelope mínimo = tokens mínimos**: regla de oro del "barato y rápido".
