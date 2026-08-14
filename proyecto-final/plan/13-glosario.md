# 13 · Glosario

> Términos del dominio. Se actualiza conforme el sistema crece.

| Término | Significado |
|---|---|
| ALEXANDER | El sistema global; la visión completa de desarrollo autónomo con IA |
| ALEXANDRIA | El motor Rust: workspace de 12 crates que ejecuta todo |
| PHALANX | El único plugin: configuración + skills + hooks + agentes + planes |
| `alx` | El binario CLI del motor |
| Harness | Contrato de fase: entrada → ejecución → salida → compuerta de verificación |
| Compuerta (gate) | Comando(s) que prueban la salida de una fase; sin evidencia verde, la fase no avanza |
| Hook | Disparador automático ante un evento (prompt, tool, stop, noche) |
| Evento | Señal del event bus que activa cadenas de hooks |
| Recall | Recuerdo comprimido de memoria; se inyecta en prompts futuros |
| Auto-memoria | El sistema se recuerda solo: captura → comprime → inyecta, sin que el dev repita |
| Tier de modelo | T1Cheap / T2Medium / T3Premium — decide el gobernador por dificultad |
| Governor | Gobernador de coste: routing, compresión, presupuesto, ledger |
| Ledger | Registro append-only de cada token/coste gastado |
| DAG | Grafo acíclico dirigido de tareas con dependencias |
| Goal engine | Descompone un objetivo en DAG de tareas con presupuestos |
| Envelope | Prompt mínimo que recibe un agente (solo lo que necesita) |
| Evidencia | Output capturado de un comando real: exit_code, stdout, métricas |
| Bench | Medición de performance contra umbrales matemáticos |
| Dogfood | Usar el sistema para construir el sistema |
| Night | Modo autónomo sin humano: cron, informe, commit atómico |
| MCP | Model Context Protocol: el bus que expone/consume herramientas |
| `alx doctor` | Linter del propio sistema: valida agents, hooks, skills, config |
| `alx eval` | Corre evals de skills/agentes contra fixtures golden |
| Compresión caveman | Reglas deterministas para hablar corto entre agentes (ahorra tokens) |
| Caveman level | Intensidad de compresión: lite/full/ultra/wenyan |
| `atg` | Wrapper legacy (bash) que ALEXANDRIA embebe como adaptador |
| Planning-with-files | Planes legibles en archivos; capa humana del DAG |
