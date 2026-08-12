# 10 · Testing — `alx-gate` + `alx-bench`

> Cómo ALEXANDRIA se prueba a sí misma, y cómo verifica el código que produce. El "aprobado matemáticamente" = métricas + umbrales, no opiniones.

## 1. Tres capas de testing

| Capa | Qué prueba | Herramienta |
|---|---|---|
| Unit | crates individuales | `cargo test` por crate |
| Integration | flujo de eventos, hooks, pipeline | `alx test` (test harness nativo) |
| Eval | comportamiento de agentes/skills | evals de agent-skills + casos propios |
| Bench | performance (tiempo, memoria, complejidad) | `alx-bench` (criterion + umbrales) |

## 2. Unit + integration (`alx test`)

- `cargo test` en el workspace: cada crate con tests de sus estados y transiciones.
- **Tests de contrato de fase**: dado un artefacto de entrada, la fase produce el artefacto de salida esperado (golden files en `tests/fixtures/`).
- **Tests de event bus**: emitir evento → verificar cadena de hooks disparada, orden, timeouts, lock.
- **Tests de DAG**: transiciones de estado válidas/inválidas (p. ej. `Pending → Done` sin deps = error).
- **Tests de gobernador**: clasificador de dificultad con corpus de prompts etiquetados; presupuestos límite.
- **Tests de memoria**: captura→compresión→inyección roundtrip; deduplicación; caducidad.
- **Property-based** (`proptest`): las transiciones de estado no pierden tareas; el replay del JSONL reconstruye el DAG idéntico.

## 3. Eval de agentes/skills

- Se reutiliza `plugins/agent-skills/evals/` (23 skills con casos + fixtures).
- `alx eval run <skill>` → corre el caso contra el skill, compara con golden.
- **Métrica de skills**: éxito por skill, tokens por caso, tiempo por caso. Se compara entre iteraciones de PHALANX — así "te mejoras el harness" es medible.

## 4. Bench — umbrales matemáticos (R10)

`phalanx/bench.toml` define umbrales por fase:

```toml
[bench.defaults]
max_runtime_s = 60          # fase no puede tardar más
max_memory_mb = 512
max_complexity = 20         # ciclomática promedio de código nuevo

[bench.Build]
max_runtime_s = 180
min_test_coverage = 80      # %
max_linear_scans = 0        # scans O(n) en loops (ocultos O(n²))

[bench.Review]
max_security_findings = 0
max_blockers = 0
```

- `alx-bench` mide contra el umbral; si excede → `gate` falla la fase con el número, no con una opinión.
- **Diff bench**: antes/después de cada cambio en el motor — tiempo de build, tamaño binario, tests. Regresión de >10% → bloquea merge.

## 5. Verificación de fase (gate)

Cada `PhasePassed` exige `Evidence` no vacío:

```
gate.run(Build) → ejecuta: cargo build / npm run build
                → captura: exit_code, stdout_head, duración
                → pass = (exit_code == 0) AND (duración <= umbral) AND (sin warnings si policy=strict)
```

La evidencia se anexa al Task (`evidence[]`) y es la prueba del "asegurarnos con testes que funciona".

## 6. CI

- `.github/workflows/alx.yml`: `cargo test` + `alx test` + `alx eval run all` + `alx bench --check` en cada PR.
- Compuerta de merge: los 4 verdes, o no se mergea.
- `alx doctor` como linter del propio sistema: valida agents, hooks, skills, config antes del CI.

## 7. Decisiones

- **Golden files como contrato**: el fixture es la verdad; si el comportamiento cambia, el golden cambia explícitamente (nunca a escondidas).
- **Umbrales en config, no en código**: PHALANX ajusta política sin recompilar.
- **Bench bloquea regresión**: el motor no se degrada sin que el número lo grite.
- **Eval antes de cambiar skills**: nunca editar un skill sin correr su eval antes y después.
