# 04 · Harness Pipeline — `alx-harness`

> El corazón del workflow. Cada fase es un harness con contrato de entrada, ejecución, salida y compuerta de verificación. Reanudable. Sin comandos manuales.

## 1. Las fases

```
Ingest → Spec → Plan → Build → Test → Review → Docs → Ship
```

| Fase | Entrada | Ejecuta (agentes/skills) | Salida | Compuerta (gate) |
|---|---|---|---|---|
| Ingest | Prompt / objetivo | researcher, code-explorer | `ingest.md`: contexto, archivos, deuda | archivos citados existen |
| Spec | ingest.md | spec-driven-development | `spec.md`: requisitos + aceptación | cada requisito = testable |
| Plan | spec.md | planning-and-task-breakdown | `plan/`: DAG de tareas + fases | todo requisito → ≥1 tarea |
| Build | plan | incremental-implementation, frontend/backend agents | código + diff | `cargo build` / `npm test` / lint verde |
| Test | código | test-driven-development, test-engineer | tests + cobertura | `alx-gate test` → suite verde |
| Review | diff + tests | code-review-and-quality, security-and-hardening | findings + fixes | 0 blocker, 0 security |
| Docs | código + decisiones | documentation-and-adrs | docs + ADRs + changelog | enlaces válidos, sin rotura |
| Ship | todo lo anterior | shipping-and-launch | commit atómico + PR + checklist | checks CI verdes |

## 2. Contrato de fase

```rust
struct Phase {
    id: PhaseId,
    input_artifacts: Vec<Artifact>,      // rutas + tipos (md, diff, test)
    agents: Vec<AgentSpec>,              // qué agentes/skills corren
    output_artifacts: Vec<Artifact>,     // lo que la fase produce
    gate: GateSpec,                      // comando(s) que prueban la salida
    retries: u8,                         // reintentos permitidos (max 2, luego fail)
    budget: TokenBudget,                 // heredado del Task
}

struct Artifact {
    path: PathBuf,
    kind: ArtifactKind,                  // Markdown | Diff | Test | Report | CommandOutput
    produced_by: AlxId,
}
```

## 3. El runner

1. Toma el Task con `status == Ready` y `phase == X`.
2. Monta el prompt del agente para la fase X (governor decide tier + compresión).
3. Ejecuta. Captura stdout/stderr y artefactos de salida.
4. Corre la **compuerta**: `alx-gate` ejecuta los comandos del `GateSpec`.
5. Compuerta verde → `Evidence` se anexa al Task, avanza `phase`, emite `PhasePassed`.
6. Compuerta roja → reintento (con feedback del error al agente) hasta `retries`, luego `PhaseFailed` + informe.

## 4. Reanudable

- El estado de la fase se persiste en `state/phases/<phase>.jsonl`.
- Si ALEXANDRIA se corta a mitad de Build, al arrancar re-materializa desde `Spec` (artefactos ya en disco) y continúa — **nunca repite trabajo verificado**.
- Idempotencia: `PhasePassed` con evidencia existente → skip directo.

## 5. Mapeo a skills existentes (PHALANX skills)

| Fase | Skill base (agent-skills) |
|---|---|
| Spec | `spec-driven-development` |
| Plan | `planning-and-task-breakdown` + planning-with-files |
| Build | `incremental-implementation` + `frontend-ui-engineering` |
| Test | `test-driven-development` |
| Review | `code-review-and-quality` + `security-and-hardening` |
| Docs | `documentation-and-adrs` |
| Ship | `shipping-and-launch` + `git-workflow-and-versioning` |

## 6. Loop de mejora propia (R3 "te mejoras el harness")

- Cada fase registra métricas: tokens gastados, tiempo, intentos de compuerta, causa de fallo.
- `alx-bench` compara fases entre ejecuciones. Si una fase falla >N veces por la misma causa, PHALANX **emite un Recall** al sistema: *"en este repo, Build falla por X — inyectar esta nota en el prompt de Build"*.
- El sistema aprende de sus propios fallos. Eso es "mejorarse el harness de documentación, de optimización de hablar" hecho realidad.

## 7. Decisiones

- **Compuerta por fase, no solo al final**: fallar pronto cuesta poco; fallar en Ship cuesta todo.
- **Artefactos como prueba**: ninguna fase avanza sin archivos de salida en disco.
- **Reintentos con feedback**: el segundo intento recibe el error de la compuerta; no repite a ciegas.
