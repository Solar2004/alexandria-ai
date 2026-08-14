#!/bin/bash
# ⚡ ALEXANDRIA — 1-command installer.
#   curl -fsSL https://raw.githubusercontent.com/Solar2004/alexandria-ai/main/install.sh | bash
set -euo pipefail

REPO="https://github.com/Solar2004/alexandria-ai"
DEST="${ALX_DEST:-$HOME/alexandria-ai}"
BIN="${ALX_BIN:-$HOME/.local/bin}"

echo "⚡ Installing ALEXANDRIA → $DEST"
mkdir -p "$BIN"

# 1. Clone (or update if exists)
if [ -d "$DEST/.git" ]; then
  echo "→ updating repo..."
  git -C "$DEST" pull --rebase
else
  git clone -q "$REPO" "$DEST"
fi
cd "$DEST"

# 2. Binary: download release OR compile
if [ -f /tmp/alx-release ]; then
  cp /tmp/alx-release "$BIN/alx"
elif command -v cargo >/dev/null 2>&1; then
  echo "→ building (cargo build --release)..."
  cargo build --release --manifest-path alexandria/Cargo.toml
  cp alexandria/target/release/alx "$BIN/alx"
else
  echo "→ downloading release binary..."
  curl -fsSL -o "$BIN/alx" "$REPO/releases/latest/download/alx"
fi
chmod +x "$BIN/alx"
echo "✓ binary: $BIN/alx"

# 3. Setup (interactive: core + categories)
"$BIN/alx" setup

echo "✅ ALEXANDRIA installed. Run 'alx setup' anytime to re-configure."
