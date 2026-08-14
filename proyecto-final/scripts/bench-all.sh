#!/usr/bin/env bash
# alx bench-all — corre TODAS las familias de benchmark y muestra el agregado.
# ALEXANDRIA benchmark suite (ciclo 8). Ver docs/benchmark-report.html seccion 9-10.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WS="$REPO_ROOT/proyecto-final/alexandria"
BENCH="$REPO_ROOT/proyecto-final/harnesses/bench"
BIN="$WS/target/debug/alx"
MAX="${ALX_BENCH_MAX:-0}"   # 0 = todos (por familia)

cd "$WS"
[ -x "$BIN" ] || { echo "falta binario: cargo build -p alx-cli"; exit 1; }

echo "############################################"
echo "# ALEXANDRIA — benchmark all families"
echo "############################################"
echo

# Familia 1a · BigCodeBench sample (60)
echo "== BigCodeBench sample (60) =="
if [ "$MAX" -gt 0 ]; then ALX_BENCH_MAX="$MAX" "$BIN" bench-bigcode; else "$BIN" bench-bigcode; fi

# Familia 1b · BigCodeBench HELD-OUT (30, set disjunto — robustez)
echo
echo "== BigCodeBench HELD-OUT (30, set disjunto) =="
if [ "$MAX" -gt 0 ]; then
  ALX_BENCH_FILE="$BENCH/bigcodebench-holdout.jsonl" ALX_BENCH_MAX="$MAX" "$BIN" bench-bigcode
else
  ALX_BENCH_FILE="$BENCH/bigcodebench-holdout.jsonl" "$BIN" bench-bigcode
fi

# Familia 2 · HumanEval (164)
echo
echo "== HumanEval (164) =="
if [ "$MAX" -gt 0 ]; then ALX_BENCH_MAX="$MAX" "$BIN" bench-humaneval; else "$BIN" bench-humaneval; fi

echo
echo "############################################"
echo "# Agregado: ver docs/benchmark-report.html seccion 9"
echo "# 254 problemas · harness 221/254 (87%) · recupera 63"
echo "############################################"
