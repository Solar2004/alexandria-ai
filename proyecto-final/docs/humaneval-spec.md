# Spec — Integración HumanEval al benchmark (generalidad, familia 2/4)

> Estado: ciclo 7, iteración 11. Datos ya descargados: `harnesses/bench/humaneval.jsonl` (164 problemas).
> Objetivo: agregar una 2ª familia de benchmark para probar GENERALIDAD (no sobreajuste a BigCodeBench).

## Formato de datos (verificado en el archivo)

```json
{
  "task_id": "HumanEval/0",
  "prompt": "from typing import List\n\n\ndef has_close_elements(numbers: List[float], threshold...",  // imports + def + docstring
  "test": "\n\nMETADATA = {...}\n\n\ndef check(candidate): ...",  // define check(candidate), incluye asserts
  "entry_point": "has_close_elements"
}
```

Diferencia clave con BigCodeBench:
- NO usa unittest. El `test` define `check(candidate)` que debe llamarse con la función resuelta.
- El `prompt` YA incluye imports + def + docstring (problema completo).

## Verificador (run_humaneval)

```python
# solution = prompt + <cuerpo generado por el modelo>  (función completa)
# test = bloque check(candidate)
exec(solution)
exec(test)
check(entry_point)   # AssertionError si falla
```

Ejecutar en un proceso python3, capturar stdout/stderr. Éxito = exit 0 sin AssertionError.
Feedback para el harness: las primeras líneas del AssertionError (valores expected/actual) + el nombre del assert.

## Estructura en alx-cli (siguiendo render_bench_bigcode)

1. `run_humaneval(solution, test, entry_point) -> (bool, String)` — escribe temp, ejecuta
   `python3`, evalúa exit + AssertionError. Timeout 60s (tests HumanEval son rápidos).
2. `render_bench_humaneval() -> String` — lee humaneval.jsonl, loop:
   - Directa: 1 generación + verificación.
   - Harness: plan-then-code (misma prompt que campeón BigCodeBench: describir algoritmo
     antes del código) + feedback sobre AssertionError, hasta 4 intentos.
3. Comando CLI: `alx bench-humaneval`.
4. Métrica: humaneval_ok/164. Reporte agregado (suma con BigCodeBench) para generalidad.

## Prompt del modelo (reutilizar campeón)

```
{prompt}\n\nCompleta {entry_point}: PRIMERO describe tu algoritmo en UNA frase, LUEGO
escribe SOLO el codigo python de la funcion completa entre marcadores ```python.
```

El modelo completa el cuerpo; el def ya viene en prompt.

## Criterios

- Éxito del harness: ≥ directa + N_extra (HumanEval es fácil para frontier ~90% pass@1,
  así que el harness debe acertar CASI todo: ≥90%; la recuperación se mide en el margen
  donde el modelo flashea).
- Generalidad: si el harness mejora o iguala en AMBAS familias (BigCodeBench + HumanEval),
  el mecanismo es general, no sobreajustado.

## Validación

1. `cargo clippy` 0 + tests.
2. `alx bench-humaneval` (164 problemas).
3. Comparar con scores publicados de HumanEval (frontier ~90%+): la directa de deepseek
   dará referencia; el harness debe ≥ directa.
