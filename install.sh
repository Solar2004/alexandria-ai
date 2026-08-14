#!/bin/bash
# ⚡ ALEXANDRIA — instalador de 1 comando.
#   curl -fsSL https://raw.githubusercontent.com/Solar2004/alexandria-ai/main/install.sh | bash
set -euo pipefail

REPO="https://github.com/Solar2004/alexandria-ai"
DEST="${ALX_DEST:-$HOME/alexandria-ai}"
BIN="${ALX_BIN:-$HOME/.local/bin}"

echo "⚡ Instalando ALEXANDRIA → $DEST"
mkdir -p "$BIN"

# 1. Clonar (o actualizar si existe)
if [ -d "$DEST/.git" ]; then
  echo "→ actualizando repo..."
  git -C "$DEST" pull --rebase
else
  git clone -q "$REPO" "$DEST"
fi
cd "$DEST"

# 2. Binario: descargar release O compilar
if [ -f /tmp/alx-release ]; then
  cp /tmp/alx-release "$BIN/alx"
elif command -v cargo >/dev/null 2>&1; then
  echo "→ compilando (cargo build --release)..."
  cargo build --release --manifest-path alexandria/Cargo.toml
  cp alexandria/target/release/alx "$BIN/alx"
else
  echo "→ descargando release binario..."
  curl -fsSL -o "$BIN/alx" "$REPO/releases/latest/download/alx"
fi
chmod +x "$BIN/alx"
echo "✓ binario: $BIN/alx"

# 3. Setup (interactivo: core + categorías)
"$BIN/alx" setup

echo "✅ ALEXANDRIA instalado. Ejecuta 'alx setup' cuando quieras re-configurar."
