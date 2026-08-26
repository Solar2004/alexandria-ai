# ALEXANDRIA — Quickstart

> De cero a usar el motor. Verificado.

## 1. Instalar / actualizar

```bash
cd ~/Projectos/AlexanderTheGreat
./install.sh                 # motor alx + integración
scripts/routa/install.sh     # cadena de modelos (gateway + CLI routa)
alx setup                    # sincroniza hooks completos + statusline + MCP
```

## 2. Estado

```bash
alx tui          # dashboard ratatui vivo: red, gobernador, harnesses, bucle
alx status       # estado simple
alx network      # red real con probes GET sin coste
routa doctor     # salud de la cadena + generación real de prueba
routa auto       # encuentra un modelo vivo y lo activa (si el actual 500s)
```

## 3. Motor

```bash
alx build                        # dogfood build
alx run "tarea" --real           # pipeline REAL: cadena LLM + critic + ledger
alx feature "feature"            # artefacto en artifacts/features/ + verificación
alx spawn general-purpose "task" # agente real contra la cadena
alx doctor                       # valida el ecosistema
alx cost                         # coste acumulado real
```

## 4. Proyectos: `.alexandria/` (el harness se adapta a CADA proyecto)

```bash
cd mi-proyecto
alx init                # crea .alexandria/: registry propio, rúbricas, skills,
                        # diario de lecciones y config del pulido
alx harness-list        # registry DE ESTE proyecto (resuelto automático)
alx harness-new <slug> --objective "..." --doc "..." [--kind permanent]
alx evolve              # watcher: promueve por uso, retira cumplidos
alx patterns --apply    # bloqueos recurrentes -> harnesses permanentes
```

## 5. Investigación profunda (plan/17) — pensamiento de experto

```bash
alx research "pregunta"   # 7 artefactos: pregunta → fundamentos → iceberg →
                          # simulaciones → frenos → evidencia → síntesis
alx research-check        # COMPUERTA: falla si está superficial (exit 1);
                          # el hook Stop bloquea cerrar sesión a medias
alx polish fichero.md --rubric research
                          # pulido dosificado: para solo viendo la mejora
                          # (meseta → parada; max_rounds solo techa)
alx skills-fetch --search "claude skills"   # buscar en GitHub POR ESTRELLAS
alx skills-fetch owner/repo                 # instalar reglas del experto
```

## 6. Cadena de modelos (routa)

```bash
routa show / models / use <model>   # cambiar modelo = un comando
routa status / key next / logs gateway
```
El gateway hace failover automático si el modelo activo cae arriba.
