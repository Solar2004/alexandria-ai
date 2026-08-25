# Benchmark BigCodeBench N=20 — Ciclo 10 (2026-08-25)

> Cadena: deepseek-v4-flash con failover automático activo durante la carrera.
> Verificación por unittest REAL de BigCodeBench (ICLR'25).

| Modo | Resultado | % |
|---|---|---|
| DIRECTA | 3/20 | 15% |
| HARNESS (plan-then-code + feedback) | 9/20 | 45% |
| **Multiplicador** | **3.0x** | |

## Por problema

| Problema | Directa | Harness |
|---|---|---|
| BigCodeBench/0 | ✓ | ✓ |
| BigCodeBench/1 | ✗ | ✓ |
| BigCodeBench/2 | ✗ | ✓ |
| BigCodeBench/3 | ✗ | ✗ |
| BigCodeBench/4 | ✗ | ✓ |
| BigCodeBench/5 | ✓ | ✓ |
| BigCodeBench/6 | ✗ | ✗ |
| BigCodeBench/7 | ✗ | ✓ |
| BigCodeBench/8 | ✓ | ✓ |
| BigCodeBench/10 | ✗ | ✗ |
| BigCodeBench/11 | ✗ | ✗ |
| BigCodeBench/12 | ✗ | ✗ |
| BigCodeBench/15 | ✗ | ✗ |
| BigCodeBench/16 | ✗ | ✓ |
| BigCodeBench/18 | ✗ | ✗ |
| BigCodeBench/22 | ✗ | ✓ |
| BigCodeBench/23 | ✗ | ✗ |
| BigCodeBench/30 | ✗ | ✗ |
| BigCodeBench/33 | ✗ | ✗ |
| BigCodeBench/48 | ✗ | ✗ |

## Notas

- Fix imprescindible previo: stdout truncado a 4000 chars cortaba respuestas
  largas -> falso 0/20. Ver commit ea8b4bc.
- Durante la carrera el failover actuó solo (deepseek → minimax-m3): el
  gobernador sostuvo ~90 min de generación continua sin saturar.
- Comparabilidad: modelo distinto al ciclo 7 (muse entonces, 57% harness);
  el multiplicador del sistema se mantiene (~3x), el techo absoluto depende
  del modelo activo.
- Log crudo: `bigcode20-ciclo10.out` (cada problema aparece 2x: streaming
  en vivo + recapitulación final; aquí deduplicado).
