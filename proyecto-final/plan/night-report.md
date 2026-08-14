# Informe nocturno — 2026-08-14 02:57

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
[1;36m│ Red:[0m [31m✗[0m headroom [31m✗[0m cc-model-mask [31m✗[0m routatic [31m✗[0m omniroute 
[1;36m│ Coste:[0m Coste estimado total: $0.005660
[1;36m│ Doctor:[0m Total items: 27
[1;36m│ Telemetría:[0m 49 eventos · night systemd: 02:00
[1;36m│ Métricas:[0m Total: 8901 líneas
[1;36m│ Agentes reales:[0m 421 en el ecosistema
[1;36m│ Comandos ejecutados:[0m 6639
[1;36m│ Comandos:[0m status network build run --real night mcp phalanx feature evolve doctor cost agents spawn tui
[1;33m╚══════════════════════════════════════════════════════════════════╝[0m


## Cost report (governor)
Llamadas reales: 102
Tokens: 3257 in / 5009 out
Coste estimado total: $0.005660

Eventos por día:
  día 20678: 49 eventos


## Doctor ALEXANDRIA
Total items: 27
== Informe Doctor del Registry ==
Total items: 27

== Items por kind ==
Kind       Count
-----      -----
Skill      0
Agent      0
Plugin     16
Hook       10
McpServer  0
Harness    1

== Items inválidos ==
(ninguno)

== Duplicados detectados ==
(ninguno)


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
## Agentes reales del ecosistema (repo): 421


## Métricas por crate
  alx-agents: 643 líneas
  alx-audit: 462 líneas
  alx-bench: 348 líneas
  alx-cli: 2473 líneas
  alx-core: 471 líneas
  alx-critic: 529 líneas
  alx-evolve: 688 líneas
  alx-gate: 335 líneas
  alx-governor: 542 líneas
  alx-harness: 380 líneas
  alx-hooks: 431 líneas
  alx-lib: 198 líneas
  alx-mcp: 414 líneas
  alx-memory: 317 líneas
  alx-night: 239 líneas
  alx-task: 431 líneas
Total: 8901 líneas


--- Resumen semanal ---
## Resumen semanal ALEXANDRIA
## Cost report (governor)
Llamadas reales: 102
Tokens: 3257 in / 5009 out
Coste estimado total: $0.005660

Eventos por día:
  día 20678: 49 eventos

## Harnesses evolutivos
Vivos: 1 · Retirados: 0
## Agentes
3 especializados (general-purpose, code-reviewer, test-engineer)
## Métricas
Total: 8901 líneas

