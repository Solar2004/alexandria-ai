#!/usr/bin/env python3
"""cc-openai-bridge — traduce OpenAI /chat/completions -> Anthropic /v1/messages (routatic :3456).
Permite que herramientas que solo hablan OpenAI (p.ej. code-graph-rag Cypher gen) usen el stack local.
"""
import http.server, json, urllib.request, sys

ROUTATIC = "http://127.0.0.1:3456"
KEY = "***REMOVED***"
PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 3461

class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.0"

    def _translate(self, body):
        # body OpenAI: {messages|input: [{role, content}], max_tokens?, temperature?}
        msgs = []
        for m in body.get("messages") or body.get("input") or []:
            role = m["role"]
            if role == "system":
                msgs.append({"role": "user", "content": "[Sistema] " + m["content"]})
            elif role == "assistant":
                msgs.append({"role": "assistant", "content": m.get("content") or ""})
            else:
                msgs.append({"role": "user", "content": m.get("content") or ""})
        payload = {
            "model": "deepseek-v4-flash",
            "max_tokens": body.get("max_tokens", 2048),
            "messages": msgs,
        }
        req = urllib.request.Request(ROUTATIC + "/v1/messages",
                                     data=json.dumps(payload).encode(),
                                     headers={"Content-Type": "application/json",
                                              "x-api-key": KEY,
                                              "anthropic-version": "2023-06-01"})
        with urllib.request.urlopen(req, timeout=300) as up:
            return json.loads(up.read().decode())

    def do_POST(self):
        length = int(self.headers.get("Content-Length") or 0)
        try:
            body = json.loads(self.rfile.read(length) or b"{}")
            data = self._translate(body)
            text = ""
            for c in data.get("content", []):
                if c.get("type") == "text":
                    text += c.get("text", "")
            out = {"id": data.get("id"), "object": "chat.completion",
                   "model": "deepseek-v4-flash",
                   "choices": [{"index": 0, "message": {"role": "assistant", "content": text},
                                "finish_reason": data.get("stop_reason") or "stop"}],
                   "output": [{"type": "message", "role": "assistant",
                               "content": [{"type": "output_text", "text": text, "annotations": []}]}],
                   "usage": data.get("usage", {})}
            raw = json.dumps(out).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(raw)))
            self.end_headers()
            self.wfile.write(raw)
        except Exception as e:
            raw = json.dumps({"error": {"message": str(e)}}).encode()
            self.send_response(502)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(raw)))
            self.end_headers()
            self.wfile.write(raw)

    def log_message(self, *a):
        pass

http.server.ThreadingHTTPServer(("127.0.0.1", PORT), H).serve_forever()
