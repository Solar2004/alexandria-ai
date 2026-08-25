# routa — gobierno de la cadena de modelos

La cadena de ALEXANDRIA (v2, sin muse-stack):

```
Claude Code ──> headroom :8788 ──> routa-gateway :3460 ──> routatic :3456 ──> opencode-go
Hermes/herramientas OpenAI ─────────────────────────────> routa-gateway :3461 ─┘
```

| Puerto | Servicio | Para qué |
|--------|----------|----------|
| `:3456` | routatic-proxy | Enrutado por escenario hacia opencode-go. **Fuente única del modelo real**: `~/.config/routatic-proxy/config.json` |
| `:3460` | routa-gateway | Máscara `[1m]` + suelo de `max_tokens` + **gobernador de entropía** + health GET |
| `:3461` | routa-gateway | Compat OpenAI (`/v1/chat/completions` → `/v1/messages`) |
| `:8788` | headroom | Compresión de contexto (opcional pero recomendado) |

## Qué hace el gateway por ti

1. **Claude Code ve un modelo `[1m]`** (`claude-opus-4-6[1m]`) y no compacta la
   conversación antes de tiempo; el modelo real se lee EN VIVO del config de
   routatic, así que cambiar de modelo no toca el gateway.
2. **Suelo de max_tokens (1024)**: los modelos razonan antes de emitir texto;
   con presupuestos mínimos devuelven vacío y routatic lo toma como 400 → 502.
3. **Probes cortocircuitados**: los pings de salud nunca cuestan una generación.
4. **Gobernador de entropía**: techo global de concurrencia (`ROUTA_MAX_CONCURRENCY`,
   3 por defecto), cola con timeout, reintentos con backoff exponencial
   jitterizado y circuit-breaker. Es la cura estructural del "demasiadas
   conexiones tumban los modelos": sin techo ni ruido, cada cliente reintenta
   sincronizado y la ráfaga vuelve a saturar la cuenta.
5. Telemetría en `GET /stats` y salud en `GET /health`.

## CLI

```bash
routa show                # modelo activo por slot + salud de servicios
routa models              # catálogo disponible en opencode-go
routa use <modelo>        # CAMBIA EL MODELO POR DEFECTO (slots+aliases+restart)
routa use mimo-v2.5 --slot vision
routa status              # salud GET-only + telemetría del gobernador
routa doctor              # status + prueba real del modelo activo
routa key next            # rotar clave opencode-go (sticky: ya no rota sola)
routa logs gateway        # journalctl -f
```

## Claves

El wrapper `oc-go-cc-wrapper` (v2) usa la clave marcada en
`~/.config/oc-go-cc/.key_index` y NO rota en cada restart (antes un simple
restart podía caer en una clave sin créditos y tumbar la sesión).
Las claves sin créditos deben comentarse con `#` en `api_keys`.

## Instalación / actualización

```bash
cd AlexanderTheGreat/scripts/routa && ./install.sh
```

Idempotente; retira los restos de muse-stack si aún existieran.
