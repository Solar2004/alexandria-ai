# routa — gobierno de la cadena de modelos (⚠️ RETIRADO)

> **Estado (2026-09): routatic-proxy y routa-gateway ya no existen.** El puerto
> `:3456` lo ocupa **token-proxy** (opencode-go con rotación de 2 keys), que ya
> enmascara `claude-*` → `muse-spark-1.2-contributor` en su propio config
> (`~/.config/token-proxy/config.jsonc`). Este directorio se conserva como
> referencia histórica; `alx network` ya no sondea `:3460`.

La cadena de ALEXANDRIA (v3 token-proxy):

```
Claude Code ──> headroom :8788 ──> token-proxy :3456 ──> opencode-go (muse-spark)
```

| Puerto | Servicio | Para qué |
|--------|----------|----------|
| `:3456` | token-proxy | PROVIDER opencode-go (rotación 2 keys). Mapea `claude-*` → modelo real |
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
