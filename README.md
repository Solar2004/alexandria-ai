# ⚡ ALEXANDRIA — Autonomous AI Development Engine

[![Rust](https://img.shields.io/badge/Rust-16_crates-orange)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/License-MIT-blue)](LICENSE)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](https://github.com/Solar2004/alexandria-ai/pulls)
[![Benchmarks](https://img.shields.io/badge/Benchmarks-verified-green)](docs/benchmark-report.html)

**ALEXANDRIA** is an autonomous AI development engine written in Rust. It gives any LLM agent a **self-improving harness**: staged execution, real verification, self-critique, and iteration driven by actual work — not promises.

![Benchmark chart](docs/benchmark-chart.svg)

## 📊 Measured advantage (execution-verified, real datasets)

| Benchmark | Direct AI (no harness) | **ALEXANDRIA harness** | Advantage |
|---|---|---|---|
| BigCodeBench (ICLR'25) · 186 tasks | 11% | **67%** | **~6x** |
| BigCodeBench held-out · 30 tasks | 7% | **73%** | **~11x** |
| HumanEval · 164 tasks | 90% | **95%** | +6% |

> The harness **recovers** failures: when a raw model gets it wrong, ALEXANDRIA describes the algorithm, executes, compares, and iterates until the test passes. Model-agnostic, benchmark-agnostic, **honest** (limits reported too).

---

## ✨ What ALEXANDRIA does

- **Harness that improves results** — `plan-then-code` (describe → code → execute → verify → fix)
- **Hook-driven iteration (R24)** — the agent can't "finish" without verifying; state auto-advances with real commits
- **16-crate Rust engine** — gates, hooks, critic, memory, governor, task graph, MCP server, night ops
- **Self-setup** — `alx setup` generates the full Claude Code integration from a canonical template
- **Multi-session aware** — per-session iteration state via `ALX_SESSION_ID`
- **No telemetry** — privacy-first

## 📦 Quick start

Requires **Rust** (stable) + any Anthropic-compatible LLM endpoint.

```bash
git clone https://github.com/Solar2004/alexandria-ai.git
cd alexandria-ai

cargo build --release --manifest-path alexandria/Cargo.toml
cp alexandria/target/release/alx ~/.local/bin/alx

alx setup          # wires everything into Claude Code (hooks, statusline, MCP, themes, skills)
claude             # the harness runs automatically
alx bench          # re-run the benchmarks yourself
```

## 🧰 Commands

| Command | What it does |
|---|---|
| `alx setup` | Generate/verify the full Claude Code integration (`.claude/` regenerable) |
| `alx bench` | Real benchmarks · 3 families · execution-verified |
| `alx feature "task"` | Full pipeline: decompose → harness → verify → critic |
| `alx status` / `alx doctor` | System state / audit |
| `alx mcp` | Serve ALEXANDRIA as an MCP server |
| `alx night` | Autonomous night-ops report |

## 🏗️ Architecture

```
alexandria/       # Rust engine (16 crates: alx-gate, alx-critic, alx-harness, alx-governor...)
harnesses/        # iteration hooks (iterate.sh, auto-iterate.sh, session-reset.sh) + plugin hooks
integration/      # themes (sol/luna) + skills (fable, emil, night-ops)
phalanx/          # mega-plugin (config.toml + 10 hooks)
plan/             # mission + architecture specs (MISSION.md)
agents/           # agent registries
```

## 🧪 Benchmarks methodology

- **Real datasets**: BigCodeBench (ICLR'25), HumanEval, CodeContests — downloaded from official sources
- **Execution-verified**: tests run against real outputs, not text matching
- **Honest**: the harness improves function-completion (~6x); on hard competitive I/O it matches the model — measured, reported, no overclaiming
- **Reproducible**: `alx bench` re-runs everything; `alx setup` regenerates the full integration

## 🤝 Contributing

PRs welcome — the harness value is **measured**. If you improve it, the benchmarks show it.

1. Fork + clone
2. Improve the harness or add a benchmark family
3. Run `alx bench`
4. Open a PR with before/after numbers

## 📜 License

MIT — use it, build on it, improve it.

---

*ALEXANDRIA: measure everything. Never claim "it works" — prove it.*
