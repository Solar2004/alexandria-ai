# Spec — Integración CodeContests al benchmark (generalidad, familia 3)

> Estado: ciclo 8, iteración ~18. Datos ya descargados: `harnesses/bench/codecontests-sample.jsonl` (30 problemas).
> Origen: `deepmind/code_contests` (HuggingFace, datasets-server). Problemas I/O-based.

## RESULTADO MEDIDO (ciclo 9, N=30): directa 12/30 = harness 12/30 (1.0x)

- **16 problemas duros fallan en ambos** (1575_B a 1580_F) — cero recuperación.
- El feedback expected/got NO converge cuando el modelo no produce una solución
  cercana: un algoritmo fundamentalmente mal no se arregla con el output.
- Conclusión: el harness NO aporta en I/O competitivo. Para ayudar aquí haría
  falta: (a) mejor modelo base, o (b) mecanismo distinto — p.ej. dar un ejemplo
  resuelto al modelo, o debug guiado por el test que falla (incluyendo el input
  completo, no solo expected/got).

## Formato de datos (verificado en el archivo)

```json
{
  "name": "1575_A. Another Sorting Problem",
  "description": "Andi and Budi were given an assignment...",   // problema completo
  "tests": [
    {"input": "5 2\nAA\nAB\nBB\nBA\nAZ\n", "output": "5 2 1 3 4 \n"}
  ]
}
```

Diferencia clave con las familias 1-2:
- NO hay función ni unittest — el problema es **I/O**: el programa lee stdin, escribe stdout.
- Verificación: correr la solución con cada `input`, comparar `stdout` normalizado con `output`.

## Verificador (run_codecontests)

```python
# solution = código generado por el modelo (lee stdin, escribe stdout)
# para cada test: python3 sol.py < input → comparar stdout.strip() con output.strip()
```

- Normalización: `.strip()` en ambos lados (trailing whitespace/newline).
- Timeout por test: 10s (Codeforces es estricto, pero local ok).
- Éxito = TODOS los tests pasan.

## Estructura en alx-cli (siguiendo render_bench_humaneval)

1. `run_codecontests(solution, tests) -> (bool, String)` — para cada test: escribe
   solución a temp, ejecuta `python3 sol.py` con input, compara output. Feedback:
   `test N fallo: expected X, got Y`.
2. `render_bench_codecontests() -> String` — lee codecontests-sample.jsonl, loop:
   - Directa: 1 generación + verificación.
   - Harness: plan-then-code (describir algoritmo antes del código) + feedback
     sobre el test fallido, hasta 4 intentos.
3. Comando CLI: `alx bench-codecontests`.
4. Métrica: codecontests_ok/30. Se suma a la métrica agregada (familia 3).

## Prompt del modelo (reutilizar campeón)

```
{description}\n\nEscribe SOLO codigo Python que lea de stdin y escriba a stdout
para resolver el problema. PRIMERO describe tu algoritmo en UNA frase, LUEGO
escribe el codigo entre marcadores ```python.
```

## Criterios

- Generalidad: si el harness mejora en la familia 3 (I/O-based, distinta de
  unittest/check), el mecanismo es universal.
- CodeContests es duro (competitivo, difficulty 1-8): la directa fallará mucho;
  el harness debe recuperar con el feedback de expected/got.

## Validación

1. `cargo clippy` 0 + tests.
2. `alx bench-codecontests` (30 problemas).
3. Comparar directa vs harness; añadir a métrica agregada.
