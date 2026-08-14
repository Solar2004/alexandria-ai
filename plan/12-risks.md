# 12 · Riesgos, tradeoffs y decisiones pendientes

## 1. Riesgos

| # | Riesgo | Impacto | Mitigación |
|---|---|---|---|
| R1 | El proyecto se infla (15 fases, 12 crates) | nunca termina | Dogfood en Fase 14 desde temprano; "fase completa = tests verdes" como religión |
| R2 | Depender de proxies (headroom/routatic/omniroute) que caen | pipeline muere | governor con `/readyz` + fallback + ruta directa; sistema degrada a T2 local |
| R3 | Compresión caveman pierde contexto crítico | agentes producen basura | reglas deterministas, test de roundtrip en Fase 4.2, budget de inyección |
| R4 | 420 agentes sin schema rompen el registry | doctor lleno de errores | validación gradual: primero los 8 de `.claude/agents` + 4 de agent-skills; el resto se va arreglando |
| R5 | Hooks con lock tiran la sesión | UX rota | lock solo en 4 hooks críticos; resto best-effort; timeout default 5s |
| R6 | El sistema se vuelve un framework más (nadie lo usa) | muere | dogfood inmediato: Fase 14 usa alx para construir alx |
| R7 | MCP: protocolo cambia / servidores ajenos inestables | cliente roto | envolver cada cliente con timeout y degradación; el bus no se cae si un servidor muere |
| R8 | El coste se dispara en modelos premium | factura | tier default T2, presupuesto por tarea con cap duro, ledger obligatorio |
| R9 | Git en night (commits atómicos) hace commits basura | historia sucia | gate estricto antes de commit; mensajes convencionales; nunca force-push |
| R10 | Scope creep: el usuario pide features a mitad | plan se desvía | MISSION.md como ancla; cambios → se anotan en 12 §3, se priorizan en roadmap |

## 2. Tradeoffs aceptados

| Decisión | A favor | En contra | Veredicto |
|---|---|---|---|
| JSONL primero, SQLite después | portable, grepeable, simple | lento a muy gran escala | correcto para fase 1 |
| Compresión determinista (reglas) no IA-por-salto | barato, predecible | menos "inteligente" | correcto: la IA no debe pagar por comprimir |
| Workspace de 12 crates | cada subsistema escala | compilación inicial más larga | correcto: depende hacia abajo |
| Reutilizar 420 agentes (validarlos) en vez de regenerar | no perder trabajo | schema heterogéneo | correcto: el doctor ordena |
| Hooks como datos (.toml) | configurable sin recompilar | más archivos | correcto: es el punto de PHALANX |
| MCP como bus obligatorio | un solo patrón de integración | overhead de protocolo | correcto: futuro multi-host |

## 3. Decisiones pendientes (resolver durante la build)

1. **¿RocksDB o SQLite** cuando JSONL no alcance? — decidir en Fase 6 si el DAG crece.
2. **¿Modelo de deepseek local como T1 real?** — el governor decide por `/readyz`; probar rendimiento real en Fase 5.
3. **¿Compresión caveman en nivel ultra siempre?** — nivel configurable por fase; Spec/Review quizá lite.
4. **¿PHALANX vive en este repo o en su propio repo?** — por ahora en `AlexanderTheGreat/phalanx/`; si se vuelve portable, propio repo.
5. **¿Soporte multi-host (Codex, Cursor) en v1?** — v1 = Claude Code; MCP server prepara el resto.
6. **¿Horario/castigos afectan a night?** — integrar `horario` MCP en night para no romper rutinas (depende del MCP).

## 4. Cómo se prioriza si algo cambia

Regla: **MISSION.md gana**. Si una petición nueva contradice un principio no negociable, se discute antes de implementar. Si es un extra, se anota aquí y se agenda en el roadmap.
