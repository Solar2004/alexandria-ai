# Informe nocturno — 2026-10-29 02:58

## Informe nocturno

Fecha: 2026-08-13

Resumen: 0 hechas, 2 pendientes

### Hechas (0)
- (ninguna)

### Pendientes (2)
- preparar contexto
- ejecutar paso

Coste estimado: 0.00 USD


--- Reporte completo del motor ---
[1;33m╔═ ALEXANDRIA — Motor de desarrollo IA autónomo ═════════════════╗[0m
[1;36m│ Motor:[0m 16 crates · 205 tests · `alx` en PATH
[1;36m│ Red:[0m [31m✗[0m token-proxy [31m✗[0m headroom [32m✓[0m alx-proxy [31m✗[0m omniroute 
[1;36m│ Coste:[0m Coste: n/a
[1;36m│ Doctor:[0m Total items: 350
[1;36m│ Telemetría:[0m 49 eventos · night systemd: 02:00
[1;36m│ Métricas:[0m Total: 16778 líneas
[1;36m│ Agentes reales:[0m 0 en el ecosistema
[1;36m│ Comandos ejecutados:[0m 86677
[1;36m│ Comandos:[0m status network build run --real night mcp phalanx feature evolve doctor cost agents spawn tui
[1;33m╚══════════════════════════════════════════════════════════════════╝[0m


── Cost report (governor) 
llamadas reales 102
tokens         3257 in / 5009 out
coste total    $0.005660

Eventos por día:
  día 20678 49


## Doctor ALEXANDRIA
Total items: 350
== Informe Doctor del Registry ==
Total items: 350

== Items por kind ==
Kind       Count
-----      -----
Skill      50
Agent      265
Plugin     21
Hook       10
McpServer  1
Harness    3

== Items inválidos ==
(ninguno)

== Duplicados detectados ==
- alx-night [Plugin] x2 -> crates/alx-night | systemd/user/alx-night.service

## LSP
Servers disponibles: 1/4
  ✓ clangd (c/c++) → clangd version 22.1.8


## Agentes ALEXANDRIA

- general-purpose (T2Medium, fase cualquiera): Agente general para cualquier fase del pipeline ALEXANDRIA.
- code-reviewer (T3Premium, fase Review): Revisa el código contra criterios de calidad y detecta bugs.
- test-engineer (T2Medium, fase Test): Diseña y ejecuta tests para verificar cada micro-tarea.

## Envelope (spawn general-purpose)
system: Agente general para cualquier fase del pipeline ALEXANDRIA.

REGLAS:
- Estilo caveman: técnico, sin relleno.
- Toda afirmación con evidencia: comandos reales ejecutados, no 'debería funcionar'.
task: verificar que el build pasa
budget: 15000 tokens

## Agentes reales del ecosistema: 421 ficheros
- EXECUTIVE-BRIEF
- PULL_REQUEST_TEMPLATE
- QUICKSTART
- SECURITY
- academic-anthropologist
- academic-geographer
- academic-historian
- academic-narratologist
… y 413 más (`alx agents-show <nombre>`)


## Métricas por crate
  alx-agents: 667 líneas
  alx-audit: 462 líneas
  alx-bench: 348 líneas
  alx-cli: 6266 líneas
  alx-core: 471 líneas
  alx-critic: 554 líneas
  alx-evolve: 697 líneas
  alx-gate: 928 líneas
  alx-governor: 772 líneas
  alx-harness: 380 líneas
  alx-hooks: 431 líneas
  alx-lib: 198 líneas
  alx-mcp: 508 líneas
  alx-memory: 317 líneas
  alx-night: 239 líneas
  alx-proxy: 1523 líneas
  alx-task: 431 líneas
  alx-tui: 1394 líneas
  alx-ui: 192 líneas
Total: 16778 líneas


--- Resumen semanal ---
## Resumen semanal ALEXANDRIA
── Cost report (governor) 
llamadas reales 102
tokens         3257 in / 5009 out
coste total    $0.005660

Eventos por día:
  día 20678 49

## Harnesses evolutivos
Vivos: 1 · Retirados: 2
## Agentes
3 especializados (general-purpose, code-reviewer, test-engineer)
## Métricas
Total: 16778 líneas


--- Salud de la cadena (routa) ---
routa no disponible en PATH

--- Salud de la cadena (routa) ---
routa no disponible en PATH
