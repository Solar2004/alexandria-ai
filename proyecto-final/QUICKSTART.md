# ALEXANDRIA — Quickstart

> De cero a usar el motor en 3 pasos. Verificado.

## 1. Instalar

```bash
cd ~/Projectos/AlexanderTheGreat
./proyecto-final/install.sh
```
→ `~/.local/bin/alx` + verifica PHALANX.

## 2. Ver el estado

```bash
alx tui        # dashboard completo (motor, red, coste, doctor, comandos)
alx status     # estado simple
alx network    # red real (headroom→mask→routatic, fallback omniroute)
```

## 3. Ejecutar

```bash
alx build                       # verifica el build del workspace (dogfood)
alx run "mi tarea"              # pipeline demo (sin LLM)
alx run "mi tarea" --real       # pipeline REAL: cadena LLM + critic + ledger
alx feature "mi feature"        # genera artefacto en docs/features/ + verifica
alx spawn general-purpose "task"# ejecuta un agente real contra la cadena
alx doctor                      # valida el ecosistema (27 items)
alx cost                        # coste acumulado real
```

## Modo autónomo (hook de iteración)

- El sistema **itera solo** (R24): `state.toml` en `proyecto-final/harnesses/iterate/`.
- `awaiting_user = true` → la AI espera tu respuesta en vez de forzar iteración.
- `target_iter` → cuántas iteraciones se compromete la AI por trabajo.
- `iter = 0` → ciclo completado, el hook se apaga solo hasta el próximo trabajo.
- Cron nocturno: `systemctl --user list-timers alx-night` (02:00).

## MCP

`alx mcp` es un servidor MCP (registrado en `~/.claude.json`). Cualquier cliente lo conecta: `alx mcp`.

## Más

- Estado real: `proyecto-final/ESTADO.md`
- Plan y specs: `plan/` (00-vision → 17-orquestrator)
- Misión: `plan/MISSION.md`
