# 17 · Research Harness — investigación profunda tipo experto

> El estándar no es "responder la pregunta". Es **reconstruir el dominio como
> lo haría un científico senior**: mecanismos primero, luego el mapa por capas
> (iceberg), simulaciones contrafácticas, frenos y limitantes honestos, y solo
> al final, la respuesta. Cada paso queda guardado como artefacto en
> `.alexandria/research/<tema>/` — las simulaciones NO se pierden en el chat.

## 1. Por qué existe (R26)

Fallo observado: ante una pregunta de dominio ("¿cómo crecer más, endocrinológicamente?"),
una IA responde con lo obvio (HGH, dormir bien). Un experto reconstruye el sistema:
eje GH→IGF-1, el FRENO real (FGFR3, fusión estrogénica de placas), vías CNP-NPR2,
IHH-PTHrP, condrocito como diana... y ordena TODO por capas de acceso con evidencia.
La diferencia no es conocimiento: es **proceso**. Este harness es ese proceso.

## 2. Los 6 pasos obligatorios (cada uno = un fichero)

| Paso | Fichero | Pregunta guía |
|------|---------|---------------|
| 0 | `00-question.md` | ¿Qué preguntó el usuario REALMENTE? ¿Qué preguntaría un experto además? |
| 1 | `01-fundamentos.md` | ¿Cómo funciona el sistema DE VERDAD? Actores, vías, señales, feedback loops. |
| 2 | `02-mapas.md` | Mapa del iceberg: capas de lo conocido a la frontera, con estado de evidencia por capa. |
| 3 | `03-simulaciones.md` | Simulaciones contrafácticas: "si empujo X vía Y, ¿qué pasa aguas abajo?" — cada simulación se GUÍA aquí. |
| 4 | `04-limitantes.md` | ¿Dónde están los FRENOS? Contraindicaciones, riesgos, por qué-no, qué nadie ha probado y por qué. |
| 5 | `05-fuentes.md` | Tabla de evidencia: claim → fuente → calidad (RCT/meta/cohorte/preclínico/speculativo). |
| 6 | `06-respuesta.md` | Síntesis final: respuesta directa + mapa + advertencias + siguientes preguntas. |

## 3. Las reglas del pensamiento (checklist por paso)

- [ ] **Mecanismo antes que solución**: nunca nombrar un agente sin decir su vía y dónde actúa.
- [ ] **El freno importa más que el acelerador**: todo sistema biológico/social/técnico tiene
      limitantes dominantes; identificarlos PRIMERO reordena todo lo demás.
- [ ] **Cada capa del iceberg** responde: ¿existe?, ¿está aprobado?, ¿funciona en MI caso?,
      ¿practicidad? — si una capa no aplica, se dice explícitamente por qué.
- [ ] **Simular, no listar**: para cada candidato mental, recorrer el camino causal completo
      (efecto primario → secundarios → feedbacks → resultado neto) ANTES de puntuarlo.
- [ ] **Honestidad de evidencia**: preclínico ≠ clínico; off-label ≠ aprobado; teoría ≠ dato.
- [ ] **Conectar tópicos**: las vías se cruzan (ej: estrógeno cierra placas; CNP antagoniza
      FGFR3; IHH controla proliferación). El mapa debe mostrar cruces, no islas.
- [ ] **La respuesta final cita el mapa**, no lo sustituye.

## 4. Comandos

```bash
alx research "pregunta"          # crea .alexandria/research/<slug>/ con los 7 ficheros
alx polish .alexandria/research/<slug>/02-mapas.md --rubric research   # pule un paso
alx patterns                     # aprendizajes recurrentes → harnesses permanentes
```

La rúbrica `research` (instalada por `alx init` en `.alexandria/rubrics/`) exige:
profundidad de mecanismo, capas cubiertas ≥5, ≥2 simulaciones contrafácticas,
frenos identificados, evidencia graduada, cero afirmaciones sin fuente o marcadas especulativas.

## 5. Bucle de mejora continua del propio harness

Cuando el critic detecta recurrentemente el MISMO fallo de proceso (p.ej. "no identificaste
el freno"), `alx patterns` propone un harness permanente (`auto-blocked-research-*`) que
queda en el registry del proyecto y lo vigila el watcher. El método se automejora.

## 6. Decisiones

- **Artefactos > chat**: cada paso es un fichero versionable; una simulación que no
  se guardó no existió.
- **Rúbrica exigente por defecto**: mejor parar en meseta con score alto que iterar a ciegas.
- **El proceso es el producto**: aunque la pregunta cambie de dominio, los 7 pasos son
  invariantes (dominio-agnóstico por diseño).
