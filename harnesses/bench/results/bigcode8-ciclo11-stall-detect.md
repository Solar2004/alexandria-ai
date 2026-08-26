# Benchmark BigCodeBench N=8 — Ciclo 11 (harness R28: detección de estancamiento + 6 intentos)

> Cadena: deepseek-v4-flash. Verificación por unittest REAL de BigCodeBench (ICLR'25).
> Cambio medido vs ciclo 10 (mismo modelo): directa 15% → harness 45% (3.0x).
> NUEVO: si el MISMO test falla 2x seguidas, el harness descarta el enfoque y
> reescribe desde cero con algoritmo distinto (R28 + intentos 4→6).

| Modo | Resultado | % |
|---|---|---|
| DIRECTA | 2/8 | 25% |
| HARNESS (plan-then-code + stall-detect + 6 intentos) | 7/8 | 87.5% |
| **Multiplicador** | **3.5x** | |

## Por problema

| Problema | Directa | Harness |
|---|---|---|
| BigCodeBench/0 | ✓ | ✓ |
| BigCodeBench/1 | ✗ | ✓ recuperado |
| BigCodeBench/2 | ✗ | ✓ recuperado |
| BigCodeBench/3 | ✗ | ✓ recuperado |
| BigCodeBench/4 | ✗ | ✓ recuperado |
| BigCodeBench/5 | ✓ | ✓ |
| BigCodeBench/6 | ✗ | ✗ (techo) |
| BigCodeBench/7 | ✗ | ✓ recuperado |

## Notas

- 5 de 6 fallos de la directa recuperados por el harness (recuperación 83%).
- El techo sigue siendo el modelo (problema 6 falla en ambos).
- Muestra pequeña (N=8); al persistir el artefacto queda pendiente validar con
  N=20 completo para el ratio oficial.
