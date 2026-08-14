#!/usr/bin/env python3
"""cc-model-mask — traduce el modelo que CC ve (claude-opus-4-6[1m]) al real (deepseek-v4-flash).

Por qué: Claude Code asume 200k de window para modelos que no conoce. Con el nombre
'claude-opus-4-6[1m]' usa 1M (no compacta hasta ~920k). Este proxy:
  request:  model=claude-opus-4-6[1m]  -> model=deepseek-v4-flash (upstream routatic)
  response: model=deepseek-v4-flash    -> model=claude-opus-4-6[1m] (para CC)
"""
import http.server, json, urllib.request, sys, re

UPSTREAM = "http://127.0.0.1:3456"
MASK = "claude-opus-4-6[1m]"
REAL = "deepseek-v4-flash"

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
    protocol_version = "HTTP/1.1"

    def _do(self):
        length = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(length) if length else b""
        if body:
            try:
                data = json.loads(body)
                if data.get("model") == MASK:
                    data["model"] = REAL
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
