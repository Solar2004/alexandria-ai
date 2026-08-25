#!/usr/bin/env bash
# install.sh — instala/actualiza la suite routa (gateway + CLI) y retira muse-stack.
#
#   - Copia routa-gateway.py y routa a ~/.local/bin
#   - Instala routa-gateway.service (:3460 Anthropic + :3461 OpenAI)
#   - Desactiva y borra cc-model-mask.service y cc-openai-bridge.service
#     (los proxies auxiliares de muse-stack quedan sustituidos por el gateway)
#   - Sustituye oc-go-cc-wrapper por la version sticky (sin rotacion sorpresa)
#
# Idempotente: se puede relanzar tras cada actualizacion.
set -euo pipefail

SRC_DIR="$(cd "$(dirname "$0")" && pwd)"
BIN="$HOME/.local/bin"
SD="$HOME/.config/systemd/user"

mkdir -p "$BIN" "$SD"

echo "== copiando binarios =="
install -m 755 "$SRC_DIR/routa-gateway.py" "$BIN/routa-gateway.py"
install -m 755 "$SRC_DIR/routa" "$BIN/routa"

if [ -f "$SRC_DIR/../oc-go-cc-wrapper" ]; then
  echo "== sustituyendo oc-go-cc-wrapper (clave sticky) =="
  install -m 755 "$SRC_DIR/../oc-go-cc-wrapper" "$BIN/oc-go-cc-wrapper"
fi

echo "== retirando muse-stack (mask + bridge) =="
for unidad in cc-model-mask.service cc-openai-bridge.service; do
  if [ -f "$SD/$unidad" ]; then
    systemctl --user disable --now "$unidad" 2>/dev/null || true
    rm -f "$SD/$unidad"
    echo "  retirado $unidad"
  fi
done
systemctl --user daemon-reload
rm -f "$BIN/cc-model-mask.py" "$BIN/cc-openai-bridge.py"

echo "== instalando routa-gateway.service =="
install -m 644 "$SRC_DIR/routa-gateway.service" "$SD/routa-gateway.service"
systemctl --user daemon-reload

echo "== arrancando servicios =="
systemctl --user enable --now oc-go-cc.service >/dev/null 2>&1 || true
systemctl --user restart oc-go-cc.service
sleep 1
systemctl --user enable --now routa-gateway.service >/dev/null 2>&1 || true
systemctl --user restart routa-gateway.service
sleep 1

echo "== verificacion =="
if curl -sf http://127.0.0.1:3460/health | grep -q upstream_alive; then
  echo "✓ routa-gateway vivo en :3460 (+ OpenAI :3461)"
else
  echo "✗ gateway sin respuesta; mira: journalctl --user -u routa-gateway -n 30"
  exit 1
fi
echo "listo. comandos: routa show · routa models · routa use <model> · routa doctor"
