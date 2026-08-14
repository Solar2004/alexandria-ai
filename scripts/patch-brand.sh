#!/bin/bash
# patch-brand.sh — branding de texto "Claude Code" → "ALEXANDRIA " en el binario.
# Reemplazo de MISMO byte-length (11=11) solo en strings de display
# (welcome/children/title). NO toca lógica/errores. Backup automático.
# Re-aplicar tras: claude update (aunque autoupdate está desactivado).
set -euo pipefail
BIN="${1:-/home/artorias/.local/share/claude/versions/2.1.231}"
[ -f "$BIN" ] || { echo "✗ binario no encontrado: $BIN"; exit 1; }
python3 - "$BIN" <<'PYEOF'
import os, sys
p = sys.argv[1]
data = open(p, "rb").read()
bak = p + ".atg-brand.bak"
if not os.path.exists(bak):
    open(bak, "wb").write(data)
pairs = [
    (b"Welcome to Claude Code", b"Welcome to ALEXANDRIA "),
    (b'children:"Claude Code"', b'children:"ALEXANDRIA "'),
    (b'children:["Claude Code"', b'children:["ALEXANDRIA "'),
]
total = 0
for old, new in pairs:
    assert len(old) == len(new), f"len {old} vs {new}"
    n = data.count(old)
    if n:
        data = data.replace(old, new)
        total += n
tmp = p + ".new"
open(tmp, "wb").write(data)
os.replace(tmp, p)
print(f"✓ branding ALEXANDRIA aplicado ({total} reemplazos)")
PYEOF
