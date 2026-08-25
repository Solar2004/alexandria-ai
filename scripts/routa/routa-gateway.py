#!/usr/bin/env python3
"""routa-gateway — puerta de entrada unica de ALEXANDRIA hacia routatic.

Sustituye a los dos proxies auxiliares del antiguo muse-stack
(cc-model-mask :3460 y cc-openai-bridge :3461) con UN servicio que ademas
anade lo que faltaba:

  1. Mascara [1m]: CC pide `claude-opus-4-6[1m]` -> se envia el modelo REAL
     leido EN VIVO de ~/.config/routatic-proxy/config.json (fuente unica de
     verdad: cambiar el modelo con `routa use <model>` no requiere tocar
     nada aqui). En la respuesta el nombre real se reescribe al visible,
     asi Claude Code cree hablar con un modelo de 1M y no compacta pronto.
  2. Suelo de max_tokens: muse/deepseek razonan antes de emitir texto; con
     presupuesto minusculo devuelven content vacio y routatic lo trata como
     400 -> 502. Todo presupuesto < MIN_MAX_TOKENS se sube a 1024.
  3. Health-probes cortocircuitados: los sondeos tipo ping se responden aqui
     sin gastar generacion (leccio'n que costo horas de saturacion).
  4. GOBERNADOR DE ENTROPIA: semaforo global de concurrencia hacia upstream
     (cola con timeout), reintentos con backoff exponencial JITTERIZADO y
     circuit-breaker. Esto es la cura estructural del "too many connections
     make the models work bad": sin techo global, statusline + hooks + CC +
     agentes disparaban generaciones concurrentes que saturaban la cuenta y
     devolvian "all models failed" al segundo mensaje.
  5. Compat OpenAI en :3461 (/v1/chat/completions -> /v1/messages), para que
     cualquier herramienta que solo hable OpenAI siga funcionando.

Puertos:
  :3460  Anthropic Messages  (headroom apunta aqui; CC -> headroom -> esto)
  :3461  OpenAI chat/completions (traducido)

Config por entorno:
  ROUTA_UPSTREAM        (default http://127.0.0.1:3456)
  ROUTA_ROUTATIC_CONFIG (~/.config/routatic-proxy/config.json)
  ROUTA_VISIBLE         (claude-opus-4-6[1m])
  ROUTA_PORT / ROUTA_OAI_PORT   (3460 / 3461)
  ROUTA_MAX_CONCURRENCY (3)      semaforo global hacia upstream
  ROUTA_QUEUE_TIMEOUT   (120 s)  cuanto espera un pedido en la cola
  ROUTA_RETRIES         (2)      reintentos ante 5xx/429 (no-streaming)
"""
import http.server
import json
import os
import random
import socketserver
import sys
import threading
import time
import urllib.error
import urllib.request

# ---------------------------------------------------------------- configuracion
UPSTREAM = os.environ.get("ROUTA_UPSTREAM", "http://127.0.0.1:3456")
ROUTATIC_CONFIG = os.environ.get(
    "ROUTA_ROUTATIC_CONFIG",
    os.path.join(os.path.expanduser("~"), ".config/routatic-proxy/config.json"),
)
VISIBLE = os.environ.get("ROUTA_VISIBLE", "claude-opus-4-6[1m]")
PORT = int(os.environ.get("ROUTA_PORT", "3460"))
OAI_PORT = int(os.environ.get("ROUTA_OAI_PORT", "3461"))
MAX_CONCURRENCY = int(os.environ.get("ROUTA_MAX_CONCURRENCY", "3"))
QUEUE_TIMEOUT = float(os.environ.get("ROUTA_QUEUE_TIMEOUT", "120"))
RETRIES = int(os.environ.get("ROUTA_RETRIES", "2"))
MIN_MAX_TOKENS = 1024

PROBE_MAX_TOKENS = 8
PROBE_MAX_CHARS = 32

# Alias que SIEMPRE mapean al modelo real activo. Cualquier otro nombre pasa
# tal cual (p.ej. mimo-v2.5 para vision, o variantes muse pedidas a pelo).
ALIAS_EXACT = {
    VISIBLE,
    VISIBLE.split("[")[0],
    "deepseek-v4-flash",       # legado: la cadena vieja lo usaba como REAL
    "deepseek-v4-flash[1m]",
}


def alias_prefixes():
    return ("claude-opus", "claude-sonnet", "claude-haiku")


# ------------------------------------------------- config de routatic (en vivo)
_cfg_lock = threading.Lock()
_cfg_cache = {"mtime": None, "real": None}


def real_model():
    """Modelo REAL actual, leido del config de routatic con cache por mtime.

    Si el config desaparece o esta roto, se mantiene el ultimo valor bueno:
    un cambio mal hecho en el JSON no debe tumbar la cadena en caliente.
    """
    try:
        mtime = os.stat(ROUTATIC_CONFIG).st_mtime
    except OSError:
        return _cfg_cache["real"]
    if _cfg_cache["mtime"] != mtime:
        with _cfg_lock:
            if _cfg_cache["mtime"] != mtime:  # doble chequeo tras el lock
                try:
                    with open(ROUTATIC_CONFIG) as fh:
                        cfg = json.load(fh)
                    real = cfg["models"]["default"]["model_id"]
                    if isinstance(real, str) and real:
                        _cfg_cache["real"] = real
                        _cfg_cache["mtime"] = mtime
                except Exception:
                    pass  # config roto: conservar el ultimo bueno
    return _cfg_cache["real"]


def map_request_model(name):
    """Nombre pedido por el cliente -> nombre que va a routatic."""
    if not isinstance(name, str) or not name:
        return name
    if name in ALIAS_EXACT or name.startswith(alias_prefixes()):
        return real_model() or name
    return name


def rewrite_model(obj):
    """En respuestas: nombre real -> visible, recursivo (JSON y eventos SSE)."""
    target = real_model()
    if not target:
        return
    stack = [obj]
    while stack:
        cur = stack.pop()
        if isinstance(cur, dict):
            v = cur.get("model")
            if isinstance(v, str) and v == target:
                cur["model"] = VISIBLE
            stack.extend(cur.values())
        elif isinstance(cur, list):
            stack.extend(cur)


# ------------------------------------------------------- gobernador de entropia
class EntropyGovernor:
    """Techo global de concurrencia + backoff jitterizado + circuit-breaker.

    - Semaforo: como mucho MAX_CONCURRENCY peticiones en vuelo hacia upstream;
      el resto espera en cola hasta QUEUE_TIMEOUT (luego 429 con Retry-After).
    - Backoff: intento n espera base*factor**n +- jitter uniforme. El jitter es
      la entropia que desincroniza clientes concurrentes y evita la manada.
    - Breaker: FUSE_FAILS fallos seguidos abre el circuito FUSE_COOLDOWN s
      (+jitter); en abierto se falla rapido con mensaje claro en vez de colgar.
    """

    def __init__(self):
        self.sem = threading.BoundedSemaphore(MAX_CONCURRENCY)
        self.fails = 0
        self.open_until = 0.0
        self.lock = threading.Lock()
        self.stats = {
            "in_flight": 0, "queued_peak": 0, "queued_now": 0,
            "served": 0, "retries": 0, "rejected_429": 0,
            "breaker_opens": 0, "last_error": "",
        }

    # -- circuit breaker ----------------------------------------------------
    def _record_fail(self, msg):
        with self.lock:
            self.fails += 1
            self.stats["last_error"] = msg[:300]
            if self.fails >= FUSE_FAILS:
                self.open_until = time.time() + FUSE_COOLDOWN * (1 + random.uniform(0, 0.25))
                self.stats["breaker_opens"] += 1
                self.fails = 0

    def _record_ok(self):
        with self.lock:
            self.fails = 0

    def breaker_open(self):
        return time.time() < self.open_until

    # -- cola de concurrencia ----------------------------------------------
    class Full(Exception):
        pass

    def slot(self):
        self.stats["queued_now"] += 1
        if self.stats["queued_now"] > self.stats["queued_peak"]:
            self.stats["queued_peak"] = self.stats["queued_now"]
        try:
            if not self.sem.acquire(timeout=QUEUE_TIMEOUT):
                self.stats["rejected_429"] += 1
                raise EntropyGovernor.Full()
        finally:
            self.stats["queued_now"] -= 1
        self.stats["in_flight"] += 1

    def release(self):
        self.stats["in_flight"] -= 1
        self.stats["served"] += 1
        self.sem.release()

    # -- backoff ------------------------------------------------------------
    @staticmethod
    def sleep_backoff(attempt):
        base = BACKOFF_BASE * (BACKOFF_FACTOR ** attempt)
        time.sleep(base * (1 + random.uniform(-0.35, 0.65)))


FUSE_FAILS = int(os.environ.get("ROUTA_FUSE_FAILS", "5"))
FUSE_COOLDOWN = float(os.environ.get("ROUTA_FUSE_COOLDOWN", "30"))
BACKOFF_BASE = float(os.environ.get("ROUTA_BACKOFF_BASE", "0.6"))
BACKOFF_FACTOR = float(os.environ.get("ROUTA_BACKOFF_FACTOR", "2.0"))

GOV = EntropyGovernor()


# ------------------------------------------------------------------ health
def upstream_vivo():
    """GET /v1/models a routatic: instantaneo, gratis y sin generacion."""
    try:
        req = urllib.request.Request(UPSTREAM + "/v1/models", method="GET")
        with urllib.request.urlopen(req, timeout=5) as r:
            return 200 <= r.status < 300
    except Exception:
        return False


def es_probe(data):
    """True si el cuerpo es un health-check y no trabajo real."""
    if not isinstance(data, dict):
        return False
    mt = data.get("max_tokens")
    if not isinstance(mt, int) or mt > PROBE_MAX_TOKENS:
        return False
    if data.get("tools") or data.get("system") or data.get("stream"):
        return False
    msgs = data.get("messages")
    if not isinstance(msgs, list) or len(msgs) != 1:
        return False
    c = msgs[0].get("content") if isinstance(msgs[0], dict) else None
    if isinstance(c, list):
        c = "".join(b.get("text", "") for b in c if isinstance(b, dict))
    return isinstance(c, str) and len(c) <= PROBE_MAX_CHARS


RESPUESTA_PROBE = {
    "id": "msg_probe", "type": "message", "role": "assistant", "model": VISIBLE,
    "content": [{"type": "text", "text": "ok"}],
    "stop_reason": "end_turn", "stop_sequence": None,
    "usage": {"input_tokens": 0, "output_tokens": 0},
}


# ------------------------------------------------------------ HTTP plumbing
class ThreadingHTTPServer(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True
    allow_reuse_address = True


class BaseHandler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.0"  # cierra conexion al final: delimita el body

    def log_message(self, fmt, *args):  # al journal, una linea
        sys.stderr.write("%s %s\n" % (self.address_string(), fmt % args))

    # -- lectura de cuerpo (Content-Length y chunked) -----------------------
    def leer_cuerpo(self):
        if "chunked" in (self.headers.get("Transfer-Encoding") or "").lower():
            trozos = []
            while True:
                linea = self.rfile.readline(65536).strip()
                if not linea:
                    break
                try:
                    tam = int(linea.split(b";")[0], 16)
                except ValueError:
                    break
                if tam == 0:
                    self.rfile.readline(65536)
                    break
                resto = tam
                while resto > 0:
                    d = self.rfile.read(resto)
                    if not d:
                        break
                    trozos.append(d)
                    resto -= len(d)
                self.rfile.readline(65536)
            return b"".join(trozos)
        length = int(self.headers.get("Content-Length") or 0)
        return self.rfile.read(length) if length else b""

    # -- respuestas locales --------------------------------------------------
    def responder_json(self, code, obj, extra_headers=None):
        cuerpo = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(cuerpo)))
        for k, v in (extra_headers or {}).items():
            self.send_header(k, v)
        self.end_headers()
        try:
            self.wfile.write(cuerpo)
            self.wfile.flush()
        except Exception:
            pass

    def responder_error_upstream(self, e):
        detalle = ""
        if isinstance(e, urllib.error.HTTPError):
            try:
                detalle = e.read().decode("utf-8", "replace")[:500]
            except Exception:
                pass
            code = e.code
        else:
            code = 502
        self.responder_json(code, {
            "error": {"type": "api_error",
                      "message": f"routa-gateway: {e} | upstream: {detalle}"}
        })


# --------------------------------------------------- edge Anthropic (:3460)
class GatewayHandler(BaseHandler):
    def do_GET(self):
        if self.path in ("/health", "/healthz", "/readyz"):
            vivo = upstream_vivo()
            self.responder_json(200 if vivo else 503, {
                "service": "routa-gateway", "upstream": UPSTREAM,
                "upstream_alive": vivo, "real_model": real_model(),
                "visible_model": VISIBLE,
                "governor": dict(GOV.stats),
                "concurrency_limit": MAX_CONCURRENCY,
                "breaker_open": GOV.breaker_open(),
            })
            return
        if self.path == "/stats":
            self.responder_json(200, {"governor": dict(GOV.stats),
                                      "real_model": real_model()})
            return
        if self.path == "/v1/models":
            req = urllib.request.Request(UPSTREAM + self.path, method="GET")
            try:
                with urllib.request.urlopen(req, timeout=10) as up:
                    raw = up.read()
                self.send_response(up.status)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(raw)))
                self.end_headers()
                self.wfile.write(raw)
            except Exception as e:
                self.responder_error_upstream(e)
            return
        self.responder_json(404, {"error": {"type": "not_found_error",
                                            "message": "ruta desconocida"}})

    def do_POST(self):
        body = self.leer_cuerpo()
        data = None
        if body:
            try:
                data = json.loads(body)
            except Exception:
                data = None

        # 1) probes: nunca llegan al modelo
        if es_probe(data):
            vivo = upstream_vivo()
            self.responder_json(200 if vivo else 502,
                                RESPUESTA_PROBE if vivo else
                                {"error": {"type": "api_error",
                                           "message": "upstream no responde"}})
            return

        # 2) mascara de modelo + suelo de max_tokens
        streaming = bool(isinstance(data, dict) and data.get("stream"))
        changed = False
        if isinstance(data, dict):
            if "model" in data:
                nuevo = map_request_model(data["model"])
                if nuevo != data["model"]:
                    data["model"] = nuevo
                    changed = True
            mt = data.get("max_tokens")
            if isinstance(mt, int) and mt < MIN_MAX_TOKENS:
                data["max_tokens"] = MIN_MAX_TOKENS
                changed = True
            if changed:
                body = json.dumps(data).encode()

        # 3) gobernador: breaker abierto -> fallo rapido y claro
        if GOV.breaker_open() and not streaming:
            self.responder_json(503, {
                "error": {"type": "overloaded_error",
                          "message": "routa-gateway: circuit-breaker abierto "
                                     "(upstream fallando); reintenta en segundos"}})
            return

        # 4) envio con cola + reintentos jitterizados (no-streaming)
        try:
            GOV.slot()
        except EntropyGovernor.Full:
            self.responder_json(429, {
                "error": {"type": "rate_limit_error",
                          "message": "routa-gateway: cola llena"},
            }, extra_headers={"Retry-After": "5"})
            return
        try:
            ultimo = None
            for intento in range(RETRIES + 1):
                try:
                    self._reenviar(body, streaming)
                    GOV._record_ok()
                    return
                except urllib.error.HTTPError as e:
                    ultimo = e
                    recuperable = e.code >= 500 or e.code == 429
                    if streaming or not recuperable or intento >= RETRIES:
                        raise
                    GOV.stats["retries"] += 1
                    GOV.sleep_backoff(intento)
                except (urllib.error.URLError, OSError) as e:
                    ultimo = e
                    if streaming or intento >= RETRIES:
                        raise
                    GOV.stats["retries"] += 1
                    GOV.sleep_backoff(intento)
            raise ultimo if ultimo else RuntimeError("sin respuesta")
        except Exception as e:
            GOV._record_fail(str(e))
            self.responder_error_upstream(e)
        finally:
            GOV.release()

    def _reenviar(self, body, streaming):
        req = urllib.request.Request(UPSTREAM + self.path, data=body,
                                     method="POST")
        for h, v in self.headers.items():
            if h.lower() not in ("host", "content-length", "connection"):
                req.add_header(h, v)
        with urllib.request.urlopen(req, timeout=600) as up:
            ctype = up.headers.get("Content-Type", "")
            if "text/event-stream" in ctype or streaming:
                self.send_response(up.status)
                for h, v in up.headers.items():
                    if h.lower() in ("transfer-encoding", "connection",
                                     "content-length"):
                        continue
                    self.send_header(h, v)
                self.end_headers()
                for raw in up:
                    line = raw.decode("utf-8", "replace").rstrip("\n")
                    if line.startswith("data: ") and line != "data: [DONE]":
                        try:
                            ev = json.loads(line[6:])
                            rewrite_model(ev)
                            line = "data: " + json.dumps(ev)
                        except Exception:
                            pass
                    try:
                        self.wfile.write((line + "\n").encode())
                        self.wfile.flush()
                    except Exception:
                        return  # cliente se fue: abortar limpio
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
                    if h.lower() in ("transfer-encoding", "connection",
                                     "content-length"):
                        continue
                    self.send_header(h, v)
                self.send_header("Content-Length", str(len(raw)))
                self.end_headers()
                self.wfile.write(raw)
                self.wfile.flush()


# ----------------------------------------------- compat OpenAI (:3461)
def a_messages(payload):
    """chat/completions -> messages (subminimo suficiente para herramientas)."""
    msgs = []
    system = []
    for m in payload.get("messages", []):
        role = m.get("role")
        content = m.get("content")
        if isinstance(content, list):
            content = "".join(
                p.get("text", "") for p in content if isinstance(p, dict))
        if role == "system":
            system.append(content or "")
        elif role in ("user", "assistant"):
            msgs.append({"role": role, "content": content or ""})
    out = {"model": payload.get("model"), "messages": msgs}
    if system:
        out["system"] = "\n\n".join(system)
    if payload.get("max_tokens") or payload.get("max_completion_tokens"):
        out["max_tokens"] = payload.get("max_completion_tokens") or payload.get("max_tokens")
    if payload.get("temperature") is not None:
        out["temperature"] = payload["temperature"]
    out["stream"] = bool(payload.get("stream"))
    return out


def a_chat(resp):
    """messages -> chat/completion (respuesta no-streaming)."""
    texto = ""
    for b in resp.get("content", []):
        if isinstance(b, dict) and b.get("type") == "text":
            texto += b.get("text", "")
    return {
        "id": resp.get("id", "chatcmpl-routa"),
        "object": "chat.completion",
        "created": int(time.time()),
        "model": resp.get("model", ""),
        "choices": [{"index": 0,
                     "message": {"role": "assistant", "content": texto},
                     "finish_reason": "stop"}],
        "usage": {"prompt_tokens": resp.get("usage", {}).get("input_tokens", 0),
                  "completion_tokens": resp.get("usage", {}).get("output_tokens", 0),
                  "total_tokens": (resp.get("usage", {}).get("input_tokens", 0)
                                   + resp.get("usage", {}).get("output_tokens", 0))},
    }


class OpenAIHandler(BaseHandler):
    def do_GET(self):
        if self.path.rstrip("/") in ("/v1/models", ""):
            req = urllib.request.Request(UPSTREAM + "/v1/models", method="GET")
            try:
                with urllib.request.urlopen(req, timeout=10) as up:
                    raw = up.read()
                self.send_response(up.status)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(raw)))
                self.end_headers()
                self.wfile.write(raw)
            except Exception as e:
                self.responder_error_upstream(e)
        elif self.path.rstrip("/") in ("/health", "/healthz"):
            self.responder_json(200 if upstream_vivo() else 503,
                                {"service": "routa-gateway-openai"})
        else:
            self.responder_json(404, {"error": {"message": "not found"}})

    def do_POST(self):
        body = self.leer_cuerpo()
        try:
            payload = json.loads(body)
            messages_body = a_messages(payload)
            messages_body["model"] = map_request_model(messages_body.get("model"))
        except Exception as e:
            self.responder_json(400, {"error": {"message": f"cuerpo invalido: {e}"}})
            return

        if GOV.breaker_open():
            self.responder_json(503, {"error": {
                "message": "routa-gateway: circuit-breaker abierto"}})
            return
        try:
            GOV.slot()
        except EntropyGovernor.Full:
            self.responder_json(429, {"error": {"message": "cola llena"}},
                                extra_headers={"Retry-After": "5"})
            return
        try:
            ultimo = None
            for intento in range(RETRIES + 1):
                try:
                    self._enviar(messages_body, payload)
                    GOV._record_ok()
                    return
                except urllib.error.HTTPError as e:
                    ultimo = e
                    if (not (e.code >= 500 or e.code == 429)) or intento >= RETRIES:
                        raise
                    GOV.stats["retries"] += 1
                    GOV.sleep_backoff(intento)
                except (urllib.error.URLError, OSError) as e:
                    ultimo = e
                    if intento >= RETRIES:
                        raise
                    GOV.stats["retries"] += 1
                    GOV.sleep_backoff(intento)
            raise ultimo if ultimo else RuntimeError("sin respuesta")
        except Exception as e:
            GOV._record_fail(str(e))
            self.responder_json(getattr(e, "code", 502) if isinstance(e, urllib.error.HTTPError) else 502,
                                {"error": {"message": f"routa-gateway: {e}"}})
        finally:
            GOV.release()

    def _enviar(self, messages_body, payload_origen):
        body = json.dumps(messages_body).encode()
        req = urllib.request.Request(UPSTREAM + "/v1/messages", data=body,
                                     method="POST")
        req.add_header("content-type", "application/json")
        req.add_header("anthropic-version", "2023-06-01")
        auth = self.headers.get("Authorization") or self.headers.get("x-api-key")
        if auth:
            req.add_header("x-api-key", auth.replace("Bearer ", "", 1)
                           if auth.startswith("Bearer ") else auth)
        with urllib.request.urlopen(req, timeout=600) as up:
            raw = up.read()
        try:
            resp = json.loads(raw)
        except Exception:
            resp = None
        if resp is None or resp.get("type") != "message":
            # propagar error tal cual con su codigo
            self.send_response(up.status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(raw)))
            self.end_headers()
            self.wfile.write(raw)
            return
        salida = json.dumps(a_chat(resp)).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(salida)))
        self.end_headers()
        self.wfile.write(salida)
        self.wfile.flush()


# --------------------------------------------------------------------- main
def main():
    print(f"routa-gateway v1.0.0 upstream={UPSTREAM} visible={VISIBLE}", flush=True)
    servidores = []
    for puerto, handler, etiqueta in ((PORT, GatewayHandler, "anthropic"),
                                      (OAI_PORT, OpenAIHandler, "openai")):
        try:
            s = ThreadingHTTPServer(("127.0.0.1", puerto), handler)
            servidores.append((s, etiqueta, puerto))
        except OSError as e:
            print(f"no pude abrir :{puerto} ({etiqueta}): {e}", file=sys.stderr)

    hilos = []
    for s, etiqueta, puerto in servidores:
        t = threading.Thread(target=s.serve_forever, daemon=True)
        t.start()
        hilos.append(t)
        print(f"  escuchando :{puerto} ({etiqueta})", flush=True)
    if not servidores:
        sys.exit(1)
    try:
        while True:
            time.sleep(3600)
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
