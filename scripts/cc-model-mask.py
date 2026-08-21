#!/usr/bin/env python3
"""cc-model-mask — traduce el modelo que CC ve (claude-opus-4-6[1m]) al real (deepseek-v4-flash).

Por qué: Claude Code asume 200k de window para modelos que no conoce. Con el nombre
'claude-opus-4-6[1m]' usa 1M (no compacta hasta ~920k). Este proxy:
  request:  model=claude-opus-4-6[1m] (o claude-opus-4-6, forma que headroom sanea)
            -> model=deepseek-v4-flash (upstream routatic)
  response: model=deepseek-v4-flash    -> model=claude-opus-4-6[1m] (para CC)
El prefijo tolera el sanitize de headroom: 'claude-opus-4-6[1m]' se sanea a
'claude-opus-4-6' (los corchetes parecen código ANSI), y ambos deben mapear a deepseek.
"""
import http.server, json, os, urllib.request, sys

# Configurables por entorno para que el proxy sirva en otras maquinas sin
# tocar el codigo. Los valores por defecto son los del stack de Alexander.
UPSTREAM = os.environ.get("CC_MASK_UPSTREAM", "http://127.0.0.1:3456")
MASK = os.environ.get("CC_MASK_VISIBLE", "claude-opus-4-6[1m]")
REAL = os.environ.get("CC_MASK_REAL", "muse-spark-1.2-contributor")
MIN_MAX_TOKENS = 1024  # suelo: muse necesita ~256 tokens de razonamiento antes de emitir texto

# --- Health probes: se responden aqui, nunca llegan al modelo ---------------
#
# `alx network` (v0.2.0) sondea cada hop con un POST a /v1/messages y este body:
#   {"model":"...","max_tokens":1,"messages":[{"role":"user","content":"ping"}]}
# Su comentario dice "max_tokens=1 -> barato y rapido", y lo seria si llegara
# intacto. Pero el suelo de arriba lo sube a 1024, asi que el probe se convertia
# en una generacion completa: ~44 s y ~308 tokens medidos el 2026-08-21. Lanzado
# desde el statusline en cada refresco -> 6-8/min y 4-7 generaciones concurrentes
# permanentes, opencode-go saturado, y las peticiones reales de Claude Code
# devolvian "all models failed" -> 502.
#
# No basta con dejar el max_tokens=1 en paz: con presupuesto minusculo muse
# devuelve content vacio, routatic lo trata como 400 y el probe daria ✗ con todo
# sano. Asi que el probe se corta aqui y su veracidad se apoya en un GET
# /v1/models a routatic, que es instantaneo y gratis.
#
# Limite conocido: comprueba que la cadena responde, no que el modelo genere. Es
# lo que un indicador de statusline necesita; para verificar generacion de verdad
# existe `alx bench`.
PROBE_MAX_TOKENS = 8      # por encima de esto ya no es un probe
PROBE_MAX_CHARS = 32      # "ping" y similares; un prompt real es mucho mayor


def es_probe(data):
    """True si el cuerpo es un health-check y no trabajo real."""
    if not isinstance(data, dict):
        return False
    mt = data.get("max_tokens")
    if not isinstance(mt, int) or mt > PROBE_MAX_TOKENS:
        return False
    # Nada que sugiera trabajo real: sin tools, sin system, un solo mensaje.
    if data.get("tools") or data.get("system") or data.get("stream"):
        return False
    msgs = data.get("messages")
    if not isinstance(msgs, list) or len(msgs) != 1:
        return False
    c = msgs[0].get("content") if isinstance(msgs[0], dict) else None
    if isinstance(c, list):  # forma en bloques
        c = "".join(b.get("text", "") for b in c if isinstance(b, dict))
    return isinstance(c, str) and len(c) <= PROBE_MAX_CHARS


def upstream_vivo():
    """GET /v1/models a routatic: instantaneo y sin coste de modelo."""
    try:
        req = urllib.request.Request(UPSTREAM + "/v1/models", method="GET")
        with urllib.request.urlopen(req, timeout=5) as r:
            return 200 <= r.status < 300
    except Exception:
        return False


RESPUESTA_PROBE = {
    "id": "msg_probe", "type": "message", "role": "assistant", "model": MASK,
    "content": [{"type": "text", "text": "ok"}],
    "stop_reason": "end_turn", "stop_sequence": None,
    "usage": {"input_tokens": 0, "output_tokens": 0},
}

def rewrite_model(obj):
    if isinstance(obj, dict):
        for k in ("model",):
            if k in obj and isinstance(obj[k], str) and obj[k] == REAL:
                obj[k] = MASK
        for v in obj.values():
            rewrite_model(v)
    elif isinstance(obj, list):
        for v in obj:
            rewrite_model(v)

class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.0"  # cierra la conexión al final → delimita el body (SSE sin chunked)

    def _do(self):
        length = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(length) if length else b""
        if body:
            try:
                data = json.loads(body)
                # Un health-check se contesta aqui: nunca debe costar una
                # generacion. Se corta antes del suelo de max_tokens, que es
                # justo lo que lo encarecia.
                if es_probe(data):
                    vivo = upstream_vivo()
                    cuerpo = json.dumps(
                        RESPUESTA_PROBE if vivo else
                        {"error": {"type": "api_error",
                                   "message": "upstream no responde"}}
                    ).encode()
                    self.send_response(200 if vivo else 502)
                    self.send_header("Content-Type", "application/json")
                    self.send_header("Content-Length", str(len(cuerpo)))
                    self.end_headers()
                    self.wfile.write(cuerpo)
                    self.wfile.flush()
                    return
                changed = False
                m = data.get("model")
                if isinstance(m, str) and m.startswith(MASK.split("[")[0]):
                    data["model"] = REAL
                    changed = True
                # Muse razona antes de emitir texto: con un presupuesto minúsculo
                # gasta todo pensando y devuelve content vacío, que routatic
                # interpreta como error 400. Claude Code sondea con max_tokens=1.
                mt = data.get("max_tokens")
                if isinstance(mt, int) and mt < MIN_MAX_TOKENS:
                    data["max_tokens"] = MIN_MAX_TOKENS
                    changed = True
                if changed:
                    body = json.dumps(data).encode()
            except Exception:
                pass
        req = urllib.request.Request(UPSTREAM + self.path, data=body, method=self.command)
        for h, v in self.headers.items():
            if h.lower() not in ("host", "content-length", "connection"):
                req.add_header(h, v)
        try:
            with urllib.request.urlopen(req, timeout=600) as up:
                ctype = up.headers.get("Content-Type", "")
                if "text/event-stream" in ctype:
                    # streaming: reenviar headers, SIN content-length (chunked)
                    self.send_response(up.status)
                    for h, v in up.headers.items():
                        if h.lower() not in ("transfer-encoding", "connection", "content-length"):
                            self.send_header(h, v)
                    self.end_headers()
                    for raw in up:
                        line = raw.decode("utf-8", "replace").rstrip("\n")
                        if line.startswith("data: ") and line != "data: [DONE]":
                            try:
                                ev = json.loads(line[6:])
                                if ev.get("type") == "message_start" and ev.get("message", {}).get("model") == REAL:
                                    ev["message"]["model"] = MASK
                                rewrite_model(ev)
                                line = "data: " + json.dumps(ev)
                            except Exception:
                                pass
                        try:
                            self.wfile.write((line + "\n").encode())
                            self.wfile.flush()
                        except Exception:
                            return
                else:
                    raw = up.read()
                    try:
                        obj = json.loads(raw)
                        rewrite_model(obj)
                        raw = json.dumps(obj).encode()
                    except Exception:
                        pass
                    self.send_response(up.status)
                    for h, v in up.headers.items():
                        if h.lower() not in ("transfer-encoding", "connection", "content-length"):
                            self.send_header(h, v)
                    self.send_header("Content-Length", str(len(raw)))
                    self.end_headers()
                    try:
                        self.wfile.write(raw)
                        self.wfile.flush()
                    except Exception:
                        pass
        except Exception as e:
            try:
                self.send_response(502)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({"error": {"type": "api_error", "message": str(e)}}).encode())
            except Exception:
                pass

    do_GET = do_POST = do_PUT = do_DELETE = do_PATCH = _do
    def log_message(self, *a): pass

if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 3460
    http.server.ThreadingHTTPServer(("127.0.0.1", port), H).serve_forever()
