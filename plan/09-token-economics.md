# 09 · Token Economics — `alx-governor`

> El "barato y rápido" hecho arquitectura. Un gobernador que decide QUÉ modelo, CON QUÉ compresión, CON QUÉ presupuesto, y mide TODO. Objetivo: ≥60% menos tokens que sesión manual equivalente.

## 1. Model routing (dificultad → tier → ruta)

Clasificador de dificultad — reglas + heurística sobre el prompt:

| Señal | Peso |
|---|---|
| Ambigüedad (sin spec, sin test) | +0.3 |
| Superficie de archivos a tocar | +0.2 |
| Riesgo (auth, pagos, migrations) | +0.3 |
| Repetitivo/mechánico (search, format, rename) | −0.4 |
| Ya hay spec + tests verdes previos | −0.3 |

Score → tier:

| Score | Tier | Qué controla | Ruta real | Uso |
|---|---|---|---|---|
| < 0.3 | T1Cheap | budget 2k, effort bajo | routatic :3456 (deepseek-v4-flash) | mecánico, search, format, lint-fix |
| 0.3–0.7 | T2Medium | budget 15k, effort medio | headroom :8788 → mask :3460 → routatic :3456 | implementación normal, tests |
| > 0.7 | T3Premium | budget 60k, effort alto | headroom :8788 → mask :3460 → routatic :3456 | planificación, review, ambigüedad, seguridad |

**Nota de red (corregida, iteración 41)**: hoy el provider real es UNO — `routatic` → `deepseek-v4-flash`. `headroom` comprime el contexto antes, `cc-model-mask` enmascara el nombre de modelo (`claude-opus-4-6[1m]` ↔ `deepseek-v4-flash`) para que CC no se desconfíe. Los tiers T1/T2/T3 **no seleccionan modelos distintos todavía**: controlan presupuesto, effort y nivel de compresión. El `mask` oculta el modelo real en la cadena.

**Ruta por disponibilidad**: governor comprueba `/readyz` de headroom. Cadena canónica: `headroom → mask → routatic`. **Fallback**: si `routatic` no responde → `omniroute :20128` (gateway multi-proveedor) como única alternativa. Si `headroom` no responde → directo a routatic (sin compresión). Omniroute NO es parte de la cadena principal — solo entra cuando routatic cae.

## 2. Compresión (optimización de hablar)

- **Entre agentes**: todo mensaje inter-agente pasa por compresión caveman (reglas: quitar artículos/filler/hedging, fragmentos, términos técnicos exactos). Pérdida estimada de contexto: <5%, ahorro: ~50–70% tokens.
- **Envelope mínimo** (ver 06): cada agente recibe solo su contexto, no el histórico completo.
- **Cache**: prompts de sistema (MISSION.md, skills, frontmatter) con hash estable → cache de prompt; solo el delta entra.
- **Context budget**: `CLAUDE_CODE_AUTO_COMPACT_WINDOW` ya está en 900k; governor marca umbrales de compactación por fase (Spec no necesita 900k).
- **Resumen jerárquico**: PreCompact → memoria escribe resumen caveman; al reanudar, el resumen sustituye al histórico.

## 3. Presupuesto por tarea

```
alx-governor asigna al crear tarea:
  T1:  2k  tokens/iteración
  T2:  15k tokens/iteración
  T3:  60k tokens/iteración
  tope por fase: max(3 iteraciones) → luego Failed + diagnóstico
```

- `warn_at_pct=80` → el hook `governor.budget-check` avisa al agente: "recorta, quedas al 20%".
- `hard_cap_pct=100` → aborta la fase, deja la tarea `Failed` con diagnóstico de dónde se fue el presupuesto.
- **Ledger**: `state/budget.ledger.jsonl` registra cada gasto: `{task_id, phase, tool, tokens, cost_usd, ts}`.

## 4. Objetivos y presupuesto de sesión

- Objetivo de sesión (p. ej. "terminar esta feature ≤ 150k tokens"): governor hace el DAG y asigna presupuesto por tarea que sume ≤ objetivo.
- Al exceder 80% del objetivo de sesión, `governor.classify` baja de tier las tareas mecánicas restantes (prioridad: preservar T3 para review/seguridad).

## 5. Cost-report (transparencia)

`alx governor cost-report` → por tarea/fase/sesión:

```
Feature X — 128k tokens, $1.2 (vs manual ~380k, $3.4)  —63%
├─ Ingest  T2  18k
├─ Spec    T3  42k
├─ Build   T2  51k   (2 reintentos: +12k)
├─ Test    T1   9k
└─ Review  T3   8k
```

Este informe es la evidencia de "barato y rápido". Va en el informe nocturno y en `bench.report`.

## 6. Decisiones

- **Tier por defecto = T2, no T3**: subir a premium cuesta tokens; solo cuando el clasificador lo justifica.
- **La compresión es determinista**: reglas fijas (caveman), no "resumir por IA" en cada salto (eso cuesta). La IA solo comprime lo que ya pasó por reglas.
- **El presupuesto es del Task, no de la sesión**: cada tarea muere sola si se desborda; no arrastra a las demás.
- **Métricas siempre**: si no se mide, no se puede afirmar "barato". El ledger es obligatorio.
