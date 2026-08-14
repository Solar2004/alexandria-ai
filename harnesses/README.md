# harnesses — harnesses evolutivos

> La AI crea harnesses en tiempo real. Temporal por defecto, permanente con evidencia. Watcher de objetivos autodestruye los temporales cumplidos.

## Estructura

```
harnesses/
├── active/     # harnesses vivos: design-system.toml, doc-min.toml, ...
├── archive/    # retirados (temporales cumplidos + zombies) — historial, no ruido
└── index.toml  # registry: id, nombre, kind, state
```

## Reglas

- **Doc-min obligatoria**: un harness sin `doc` no se registra.
- **Temporal → permanente** solo con evidencia de uso (nº de rechazos evitados).
- **Zombie** (temporal sin cumplir tras N usos) → se retira con diagnóstico.

## Spec

`plan/16-evolve.md` — lifecycle completo: detectar → crear → documentar → aplicar → vigilar → destruir/promover → mejorar.
