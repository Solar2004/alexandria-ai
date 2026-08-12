# Plan ALEXANDER / ALEXANDRIA

> El sistema definitivo autónomo de desarrollo de software con IA.
> Motor en Rust. Harness por fase. MCP como bus. Hooks en todo. Barato y rápido.

## Qué es esto

Carpeta de plan maestro. Cada archivo es un spec accionable. Nada de humo —
todo lo que está aquí se construye, se testea y se verifica.

## Cómo navegar

| Archivo | Contenido |
|---|---|
| `00-vision.md` | Nombre, misión, principios no negociables |
| `01-context.md` | Auditoría de lo que ya existe (aicli-ultimate, AlexanderTheGreat) |
| `02-architecture.md` | Arquitectura del sistema, crates del workspace Rust |
| `03-core-engine.md` | Spec del motor `alx` (CLI, daemon, tipos, event bus) |
| `04-harness-pipeline.md` | Harness por fase: spec → plan → build → test → review → docs → ship |
| `05-hooks-system.md` | Sistema de eventos/hooks: catálogo completo, ciclo de vida |
| `06-agents-system.md` | Registry de agentes, creación, routing, prompt assembly |
| `07-mcp-servers.md` | Superficie de tools MCP, integración con servidores existentes |
| `08-task-management.md` | Gestión de tareas: DAG, estados, persistencia, integración planning-with-files |
| `09-token-economics.md` | Gobernador de coste: model routing, compresión, presupuestos |
| `10-testing.md` | Cómo el sistema se testea a sí mismo (unit, integración, evals) |
| `11-roadmap.md` | Fases de construcción paso a paso, tareas y subtareas |
| `12-risks.md` | Riesgos, tradeoffs, decisiones pendientes |
| `13-glosario.md` | Términos del dominio |
| `14-auditoria.md` | Auditoría exhaustiva del ecosistema (global + repo + MCP + red), duplicados |
| `15-critic.md` | Auto-crítica por código (`alx-critic`) + decomposition engine |
| `16-evolve.md` | Harness evolutivo (`alx-evolve`): la AI crea harnesses en tiempo real, temporal/permanente, doc-min |
| `17-orquestrator.md` | Integración orquestrator-package: dual-language, verify handoff, SDD templates, iteration loop |

## Orden de lectura

1. `00-vision.md` — para qué
2. `01-context.md` — desde dónde partimos
3. `02-architecture.md` — cómo se sostiene
4. `04-harness-pipeline.md` + `05-hooks-system.md` — el corazón
5. `09-token-economics.md` — el "barato y rápido"
6. `11-roadmap.md` — por dónde empezamos

## Vista visual

- `media/alexandria-mermaid.png` — render del Mermaid MEGA (sistema completo + pipeline con decisiones + red real). Generado con mermaid.js vía navegador.

## Regla de oro

Cada fase de la build termina con **evidencia verificable**: `cargo test` verde,
comando real ejecutado, output capturado. Nada de "debería funcionar".
