#!/bin/bash
# command-catalog — inyecta el catálogo de comandos + política de autonomía.
# El agente autónomo DEBE conocer y usar los slash commands correctos por fase.
cat <<'CTX'
━━━ [COMANDOS CLAUDE CODE] ━━━
Claude Code tiene 60+ slash commands; el agente autónomo debe elegir por fase:
- /simplify  → después de escribir código (refinar, reducir complejidad)
- /security-review → cambios que tocan input/auth/datos
- /review     → revisar el código antes de entregar (5 ejes)
- /loop       → iterar sobre una tarea hasta criterio
- /skill-generator → crear un skill nuevo (reutilizable)
- /caveman    → lenguaje interno comprimido (modo trabajo)
- /compact    → comprimir contexto al límite
- /memory     → guardar/consultar memoria del proyecto
- /agent      → invocar agentes especializados
- /graphify   → convertir input a knowledge graph
- /benchmark  → medir el sistema (alx bench)
━━━ [POLÍTICA DE AUTONOMÍA] ━━━
Cuando el agente trabaje SOLO (sin usuario presente):
1. Activar /caveman (lenguaje interno comprimido) para pensar y actuar.
2. Elegir el comando correcto según la fase: /simplify tras código,
   /security-review si es sensible, /review antes de entregar.
3. NO restringirse: usar TODAS las herramientas y comandos disponibles.
4. Completar con verificación real (ejecutar, no "debería funcionar").
5. Si hay que crear algo reutilizable → /skill-generator.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
CTX
