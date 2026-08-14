# PHALANX — El único plugin de ALEXANDRIA

PHALANX es la falange: un solo plugin que hace todo (skills, hooks, agentes, planes, configuración). El usuario instala UN plugin; ALEXANDRIA (motor Rust, 16 crates) lo ejecuta.

## Qué es

- **Configuración pura, sin código Rust.**
- `config.toml` **ES el sistema**: el motor lo lee y se configura solo. Cambiarlo cambia el comportamiento — sin recompilar.
- Los hooks en `hooks/*.toml` son el sistema nervioso: cada evento dispara una cadena de hooks (Pre / Async / Post) que capturan conocimiento, controlan coste y exigen evidencia.

## Estructura

```
phalanx/
├── config.toml      # TODO el sistema, un archivo
├── hooks/           # catálogo de hooks (un .toml por hook)
├── harnesses/       # enlace al sistema de harnesses evolutivos
└── README.md        # este archivo
```

## Cómo lo lee el motor

1. `alx run` arranca → lee `config.toml`.
2. Carga los hooks de `hooks/` (evento, prioridad, lock, retry) y los registra en el bus de eventos de `alx-hooks`.
3. Carga los clientes MCP de `[mcp.clients]` (los 5 default).
4. Configura `alx-governor` con las rutas de red (routatic = provider, headroom→mask→routatic para T2/T3, omniroute solo fallback).
5. Inyecta memoria (`[memory]`) y espera el primer prompt.

## La regla

> **PHALANX es configuración, no código. La lógica vive en ALEXANDRIA.**

Si necesitas lógica nueva, va al motor (crates Rust), no al plugin. El plugin solo declara qué quiere que el sistema haga.
