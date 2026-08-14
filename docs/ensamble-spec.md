# Spec — Ensamble en harness de benchmark (objetivo 5x)

> Estado: CICLO 6, iteración 12. Datos base: BigCodeBench (ICLR'25), 60 problemas reales.
> Harness 34/60 (57%) vs Directa 9/60 (15%) = **3.8x**. Objetivo: **5x (~45/60)**.

## Datos que motivan el diseño (errores = datos)

| Config | Mecanismo | N | Harness | Directa |
|---|---|---|---|---|
| iter 9 | feedback simple, 4 intentos dependientes | 30 | 17/30 (57%) | 4/30 |
| iter 10 | feedback simple, 4 intentos dependientes | 60 | 34/60 (57%) | 9/60 |
| iter 11 | feedback rich expected/actual | 60 | 29/60 (48%) | 8/60 |

Hallazgos:
1. El feedback iterativo dependiente llega a ~57% y se estanca (techo).
2. Feedback con más detalle = ruido (empeora).
3. El techo lo ponen los problemas donde el modelo "se atasca" en un
   enfoque erróneo y el feedback no lo saca de ahí.

## Mecanismo nuevo: ENSAMBLE con feedback escalonado

Combinar DOS vías ortogonales de recuperación:

1. **Reintentos independientes (pass@k)**: generar soluciones SIN feedback
   acumulado. Cada intento es una generación fresca — si el modelo se atascó
   en un enfoque, un intento independiente prueba otro.
2. **Reintentos con feedback (loop actual)**: mantener el feedback simple
   (FAIL_TEST) que ya funciona.

Orden de intentos (hasta 5 LLM calls / problema):
```
t0: generar + testear (base, sin feedback)
t1: generar independiente + testear           <- vía ensamble
t2: generar independiente + testear           <- vía ensamble
t3: generar CON feedback(t0) + testear        <- vía loop actual
t4: generar CON feedback(t0,t1,t3) + testear  <- vía loop actual
acepta la primera que pasa los unittest.
```

### Cambio en `run_bigcode` / `render_bench_bigcode` (alx-cli)

Estructura actual del loop harness (esquemático):
```rust
let mut feedback = String::new();
for _ in 0..4 {
    let prompt = format!("{problem} ... {feedback} ...");
    let (ok, frag) = run_bigcode(&generar(&prompt), &test);
    if ok { h = true; break; }
    feedback = format!("El test fallo. Detalle: {frag}. Corrige task_func.");
}
```

Estructura nueva:
```rust
let mut trazas: Vec<String> = Vec::new();  // outputs fallidos por intento
let mut h = false;
// Fase ensamble: 3 intentos INDEPENDIENTES (sin feedback)
for _ in 0..3 {
    let (ok, frag) = run_bigcode(&generar(&prompt_base), &test);
    if ok { h = true; break; }
    trazas.push(frag);
}
// Fase feedback: 2 intentos con feedback de los fallos
if !h {
    let mut feedback = String::new();
    for i in 0..2 {
        feedback.push_str(&format!("Intento {} fallo: {}. ", i, trazas[i].chars().take(80).collect::<String>()));
        let (ok, frag) = run_bigcode(&generar(&format!("{problem} ... {feedback}")), &test);
        if ok { h = true; break; }
    }
}
```

Puntos de diseño (revisar al implementar):
- **Independencia real**: cada prompt de la fase ensamble NO incluye feedback.
- **Costo**: hasta 5 LLM calls/problema vs 4 actuales (+25%). Aceptable.
- **Test de validación**: correr N=60 y comparar con 34/60. Criterio de éxito:
  ≥41/60 (≥5x). Si <38/60, revertir a config iter 10.
- **Fallback**: si el ensamble empeora, quedarse con el loop simple (34/60
  es el suelo probado).

## Validación

1. `cargo clippy` 0 + tests alx-cli.
2. `ALX_BENCH_MAX=60 alx bench-bigcode` (mismo sample de 60).
3. Métrica: harness_ok/60. Éxito si ≥41. Comparar también directa (control).

## Notas

- No tocar el mecanismo de directa (1 intento) — es el baseline honesto.
- El sample de 60 es fijo (mismo set) para comparabilidad entre iteraciones.
- Si el ensamble da 5x, el siguiente paso es plan-then-code para subir aún más.
