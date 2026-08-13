#!/bin/bash
# install.sh — instala el binario `alx` (ALEXANDRIA) en ~/.local/bin.
# Idempotente: re-run recompila y reemplaza.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="${HOME}/.local/bin"
TARGET="${BIN}/alx"

echo "→ Compilando ALEXANDRIA (release)..."
(cd "${ROOT}/alexandria" && cargo build --release -p alx-cli)

mkdir -p "${BIN}"
cp "${ROOT}/alexandria/target/release/alx" "${TARGET}"
echo "→ Instalado: ${TARGET}"
"${TARGET}" --version
echo "→ Listo. Probar: alx status · alx network · alx run \"objetivo\""
