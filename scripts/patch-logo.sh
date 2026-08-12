#!/usr/bin/env bash
# patch-logo.sh — cambia la mascota del launcher de Claude Code por el SOL DE VERGINA (☀, Alexander)
# Método: el logo de cabecera vive en el JS del binario como escapes \uXXXX:
#   - HAm = poses de la mascota (r1E: ▛███▜ -> ▛☀☀☀▜)
#   - centro L2 hardcoded (█████ -> ☀☀☀☀☀) y L3 (▘▘ ▝▝ -> ☀☀ ☀☀)
#   - DlH = arte del launcher grande (welcome screen): centro █ -> ☀
# Todos los reemplazos mantienen byte-length exacto (escape \uXXXX = 6 bytes).
# Re-aplicar tras: claude update, patch-cc apply, patch-cc restore.
set -euo pipefail

BIN="${1:-/home/artorias/.local/share/claude/versions/2.1.228}"
BAK="$BIN.atg.bak"

[ -f "$BIN" ] || { echo "✗ binario no encontrado: $BIN"; exit 1; }

# ya parcheado?
if grep -aq 'u2600' "$BIN" 2>/dev/null && grep -aq 'u259B\\u2600\\u2600\\u2600\\u259C' "$BIN" 2>/dev/null; then
  echo "✓ mascota ya es ☀ (skip)"
  exit 0
fi

[ -f "$BAK" ] || cp "$BIN" "$BAK"
echo "backup: $BAK"

python3 - "$BIN" <<'PYEOF'
import sys
p = sys.argv[1]
data = open(p, "rb").read()
pairs = [
    # poses de la mascota (r1E): centro ███ -> ☀☀☀
    (b'"\\u259B\\u2588\\u2588\\u2588\\u259C"', b'"\\u259B\\u2600\\u2600\\u2600\\u259C"'),   # default
    (b'"\\u259F\\u2588\\u2588\\u2588\\u259F"', b'"\\u259F\\u2600\\u2600\\u2600\\u259F"'),   # look-left
    (b'"\\u2599\\u2588\\u2588\\u2588\\u2599"', b'"\\u2599\\u2600\\u2600\\u2600\\u2599"'),   # look-right
    # centro fila 2 (5 bloques, contexto JSX único)
    (b'children:"\\u2588\\u2588\\u2588\\u2588\\u2588"', b'children:"\\u2600\\u2600\\u2600\\u2600\\u2600"'),
    # fila 3 (patas -> rayos)
    (b'"\\u2598\\u2598 \\u259D\\u259D"', b'"\\u2600\\u2600 \\u2600\\u2600"'),
    # launcher grande (welcome screen, DlH)
    (b"DlH=` \\u2590\\u259B\\u2588\\u2588\\u2588\\u259C\\u258C\n\\u259D\\u259C\\u2588\\u2588\\u2588\\u2588\\u2588\\u259B\\u2598\n  \\u2598\\u2598 \\u259D\\u259D`;",
     b"DlH=` \\u2590\\u2600\\u2600\\u2600\\u2600\\u259C\\u258C\n\\u259D\\u2600\\u2600\\u2600\\u2600\\u2600\\u2600\\u2600\\u2598\n  \\u2600\\u2600 \\u2600\\u2600`;"),
]
total = 0
for old, new in pairs:
    assert len(old) == len(new), f"len {old} vs {new}"
    n = data.count(old)
    total += n
    if n:
        data = data.replace(old, new)
        print(f"  ✓ x{n}")
open(p, "wb").write(data)
print(f"✓ mascota ☀ aplicada ({total} reemplazos)")
PYEOF
