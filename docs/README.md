# 📚 ALEXANDRIA — Documentation

Guides and references for the engine. Generated assets live in `assets/`
(regenerate with `python3 scripts/gen-assets.py`).

## Guides

| Doc | What it covers |
|---|---|
| [quickstart.md](quickstart.md) | From zero to a working engine (`alx setup`, status, network, bench) |
| [crates.md](crates.md) | Engine crate breakdown (16 crates) |
| [inventory.md](inventory.md) | Full build inventory: model stack, plugins, skills, services |
| [status.md](status.md) | Real, verified state of the engine (from disk, not memory) |

## Benchmarks

| Doc | What it covers |
|---|---|
| [benchmark-chart.svg](benchmark-chart.svg) | Pass@1 comparison chart (harness vs direct AI) |
| [benchmark-report.html](benchmark-report.html) | Interactive benchmark report |
| [humaneval-spec.md](humaneval-spec.md) | HumanEval benchmark spec |
| [codecontests-spec.md](codecontests-spec.md) | CodeContests benchmark spec |
| [ensamble-spec.md](ensamble-spec.md) | Ensemble spec |

## Assets

- `assets/logo.svg` — project logo (Lábaro con ΑΛΕΞΑΝΔΡΕΙΑ)
- `assets/architecture.svg` — system architecture diagram
- `assets/iterate-loop.svg` — R24 iteration loop diagram

Regenerate all with:

```bash
python3 scripts/gen-assets.py
```

## Rule

Every source file carries a header doc. If missing, the `docmin.verify` hook
complements it automatically.