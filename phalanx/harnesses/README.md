# Harnesses de PHALANX

Este directorio enlaza al sistema de harnesses evolutivos del proyecto:
`proyecto-final/harnesses/` (ver su README para el protocolo completo).

## Cómo funciona

- El hook `evolve.detect` (PostToolUse) vigila operaciones repetidas o exitosas y las marca como candidatas a harness.
- `alx-evolve` materializa los candidatos en `proyecto-final/harnesses/`, los versiona y los reutiliza fase a fase del pipeline (Ingest..Ship).
- `config.toml [evolve]` declara `harnesses_dir = "harnesses"` y el `watcher_interval` del barrido.

Doc-min: los harnesses se versionan con su spec; no hay lógica de harness en este directorio, solo la referencia.
