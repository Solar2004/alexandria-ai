# docs — documentación viva

> Documentación mínima generada mientras la AI construye. Regla doc-min: nada se escapa.

## Qué va aquí

- Notas de diseño por crate (cuándo y por qué se tomó una decisión).
- ADRs (Architecture Decision Records) cuando aplica.
- Guías de uso del motor.

## Regla

Todo archivo de código nuevo lleva doc de cabecera. Si esto falta, el harness `docmin.verify` lo complementa automáticamente.
