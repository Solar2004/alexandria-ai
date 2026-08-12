# proyecto-final — ALEXANDRIA

> El proyecto definitivo. Motor Rust + documentación viva + harnesses evolutivos.
> La AI se construye a sí misma aquí: código, docs mínimas y harnesses temporales/permanentes.

## Estructura

```
proyecto-final/
├── alexandria/     # workspace Rust — 16 crates (alx-core ... alx-evolve)
├── docs/           # documentación viva generada mientras se construye
└── harnesses/      # harnesses evolutivos (active/*.toml + archive/)
```

## Estado

- **Motor**: 16 crates, 19 tests verdes (`cargo build && cargo test`).
- **Plan maestro**: `plan/` en la raíz del repo (00-vision → 16-evolve, 57 iteraciones de arquitectura).
- **Misión**: `plan/MISSION.md` (memoria maestra, auto-releída cada sesión).

## Cómo correr

```bash
cd proyecto-final/alexandria
cargo build
cargo test
```
