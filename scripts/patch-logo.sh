#!/usr/bin/env bash
# patch-logo.sh — cambia el logo ASCII de Claude Code por ⚔ (AlexanderTheGreat)
# Seguro: mismo byte-length exacto (⚔ = U+2694 = 3 bytes UTF-8, igual que █).
# Re-aplicar tras: claude update, patch-cc apply, restore.
set -euo pipefail

BIN="${1:-/home/artorias/.local/share/claude/versions/2.1.227}"
BAK="$BIN.atg.bak"

if [ ! -f "$BIN" ]; then
  echo "✗ binario no encontrado: $BIN"
  exit 1
fi

# ya parcheado?
if grep -aq '▐▛⚔⚔⚔▜▌' "$BIN" 2>/dev/null; then
  echo "✓ logo ya es ⚔ (skip)"
  exit 0
fi

[ -f "$BAK" ] || cp "$BIN" "$BAK"
echo "backup: $BAK"

python3 - "$BIN" <<'PYEOF'
import sys
p = sys.argv[1]
data = open(p, "rb").read()
L1 = "▐▛███▜▌".encode()
L1n = "▐▛⚔⚔⚔▜▌".encode()
L2 = "▝▜█████▛▘".encode()
L2n = "▝▜⚔⚔⚔⚔⚔▛▘".encode()
assert len(L1) == len(L1n) and len(L2) == len(L2n), "byte-length mismatch"
c1, c2 = data.count(L1), data.count(L2)
data = data.replace(L1, L1n).replace(L2, L2n)
open(p, "wb").write(data)
print(f"✓ reemplazado: línea1 x{c1}, línea2 x{c2}")
PYEOF

echo "✓ logo ⚔ aplicado"
