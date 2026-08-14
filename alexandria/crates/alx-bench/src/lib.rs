//! alx-bench — métricas de performance, umbrales y diff bench.
//!
//! El "aprobado matemáticamente" = métricas + umbrales, no opiniones (plan 10-testing §4).
//! Fase 1: umbrales desde bench.toml (parse manual línea por línea, sin la crate toml),
//! medición de runtime/memoria y verificación contra los umbrales.

use std::collections::HashMap;
use std::fs;
use std::time::Instant;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Umbrales
// ---------------------------------------------------------------------------

/// Un umbral numérico sobre una métrica.
///
/// `max` es el valor límite. Con `is_min == true` la métrica debe ser `>= max`
/// (p. ej. `min_test_coverage`); en caso contrario debe ser `<= max`
/// (p. ej. `max_runtime_s`, `max_memory_mb`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Threshold {
    /// Nombre de la métrica, tal cual en bench.toml (`max_runtime_s`, `min_test_coverage`, ...).
    pub metric: String,
    /// Valor límite.
    pub max: f64,
    /// `true` si el umbral es un mínimo (clave con prefijo `min_`).
    pub is_min: bool,
}

/// Conjunto de umbrales que define el presupuesto de una fase.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BenchSpec {
    pub thresholds: Vec<Threshold>,
}

impl BenchSpec {
    /// Parsea un mini-formato TOML simple, línea por línea (`clave = valor`).
    ///
    /// Las claves `min_*` se marcan como mínimos; el resto como máximos.
    /// Se ignoran líneas vacías, comentarios (`#`) y cabeceras de tabla (`[bench.x]`).
    /// Cualquier línea que no siga `clave = valor` es un error.
    pub fn from_toml_str(s: &str) -> Result<Self, String> {
        let mut thresholds = Vec::new();
        for (i, raw) in s.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
                continue;
            }
            let Some(eq) = line.find('=') else {
                return Err(format!(
                    "línea {}: se esperaba `clave = valor`, se encontró `{line}`",
                    i + 1
                ));
            };
            let key = line[..eq].trim();
            let val = line[eq + 1..].trim();
            let value: f64 = val
                .parse()
                .map_err(|_| format!("línea {}: valor inválido para `{key}`: `{val}`", i + 1))?;
            thresholds.push(Threshold {
                metric: key.to_string(),
                max: value,
                is_min: key.starts_with("min_"),
            });
        }
        Ok(Self { thresholds })
    }
}

// ---------------------------------------------------------------------------
// Métricas
// ---------------------------------------------------------------------------

/// Métricas medidas de una fase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    /// Tiempo de ejecución en milisegundos.
    pub runtime_ms: u128,
    /// Memoria estimada en MiB.
    pub memory_mb: f64,
    /// Cobertura de tests en porcentaje.
    pub test_coverage: f64,
}

impl Metrics {
    /// Vista como mapa `clave de bench.toml → valor`, para `check` y `BenchResult`.
    fn as_map(&self) -> HashMap<String, f64> {
        let mut m = HashMap::new();
        m.insert("max_runtime_s".to_string(), self.runtime_ms as f64 / 1000.0);
        m.insert("max_memory_mb".to_string(), self.memory_mb);
        m.insert("min_test_coverage".to_string(), self.test_coverage);
        m
    }
}

/// Mide el tiempo de ejecución de un closure, en milisegundos.
pub fn measure_runtime(f: impl FnOnce()) -> u128 {
    let start = Instant::now();
    f();
    start.elapsed().as_millis()
}

/// Estimación simple de la memoria del proceso actual en MiB.
///
/// Lee `/proc/self/statm` y usa las páginas residentes (campo 2) * 4 KiB.
/// Si el archivo no existe o no se puede leer, devuelve `0.0`.
pub fn measure_memory_mb() -> f64 {
    let Ok(statm) = fs::read_to_string("/proc/self/statm") else {
        return 0.0;
    };
    match statm.split_whitespace().nth(1) {
        Some(pages) => pages.parse::<f64>().map(|p| p * 4.0 / 1024.0).unwrap_or(0.0),
        None => 0.0,
    }
}

// ---------------------------------------------------------------------------
// Verificación contra umbrales
// ---------------------------------------------------------------------------

/// Resultado de verificar métricas contra un `BenchSpec`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchResult {
    pub passed: bool,
    /// Fallos descriptivos, uno por umbral violado.
    pub failures: Vec<String>,
    /// Métricas como mapa `clave → valor` (claves de bench.toml).
    pub metrics: HashMap<String, f64>,
}

/// Verifica `metrics` contra los umbrales de `spec`.
///
/// Reglas: umbral máximo → `valor <= max`; umbral mínimo (`is_min`) → `valor >= max`.
/// Un umbral cuya métrica no se midió cuenta como fallo (no verificable = no pasa).
pub fn check(spec: &BenchSpec, metrics: &Metrics) -> BenchResult {
    let map = metrics.as_map();
    let mut failures = Vec::new();
    for t in &spec.thresholds {
        match map.get(&t.metric) {
            None => failures.push(format!("{}: métrica no medida", t.metric)),
            Some(&value) => {
                let ok = if t.is_min { value >= t.max } else { value <= t.max };
                if !ok {
                    let op = if t.is_min { "<" } else { ">" };
                    failures.push(format!("{}: {} {} {}", t.metric, value, op, t.max));
                }
            }
        }
    }
    BenchResult {
        passed: failures.is_empty(),
        failures,
        metrics: map,
    }
}

/// Informe legible de un `BenchResult`, para stdout / gate.
pub struct BenchReport;

impl BenchReport {
    /// Una línea por umbral: `✓ <métrica>: <valor>` si pasa, o el fallo descriptivo.
    pub fn from_result(result: &BenchResult) -> String {
        let mut lines: Vec<String> = Vec::new();
        let mut failed = std::collections::HashSet::new();
        for f in &result.failures {
            lines.push(f.clone());
            if let Some(metric) = f.split(':').next() {
                failed.insert(metric.trim().to_string());
            }
        }
        let mut keys: Vec<String> = result.metrics.keys().cloned().collect();
        keys.sort();
        for k in keys {
            if !failed.contains(&k) {
                lines.push(format!("✓ {k}: {}", result.metrics[&k]));
            }
        }
        lines.join("\n")
    }
}

// ---------------------------------------------------------------------------
// Diff bench
// ---------------------------------------------------------------------------

/// Porcentaje de cambio relativo de `old` a `new`: `((new - old) / old) * 100`.
///
/// Si `old == 0` no hay base para un porcentaje finito y devuelve `0.0`.
pub fn regression(new: f64, old: f64) -> f64 {
    if old == 0.0 {
        return 0.0;
    }
    ((new - old) / old) * 100.0
}

/// `true` si el porcentaje representa una regresión mayor que `max_pct`.
pub fn is_regression(pct: f64, max_pct: f64) -> bool {
    pct > max_pct
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_thresholds_and_marks_min() {
        let spec =
            BenchSpec::from_toml_str("max_runtime_s = 60\nmax_memory_mb = 512\nmin_test_coverage = 80\n")
                .unwrap();
        assert_eq!(spec.thresholds.len(), 3);
        assert_eq!(spec.thresholds[0].metric, "max_runtime_s");
        assert_eq!(spec.thresholds[0].max, 60.0);
        assert!(!spec.thresholds[0].is_min);
        assert_eq!(spec.thresholds[1].metric, "max_memory_mb");
        assert_eq!(spec.thresholds[1].max, 512.0);
        assert!(!spec.thresholds[1].is_min);
        assert_eq!(spec.thresholds[2].metric, "min_test_coverage");
        assert_eq!(spec.thresholds[2].max, 80.0);
        assert!(spec.thresholds[2].is_min);
    }

    #[test]
    fn ignores_comments_and_table_headers() {
        let spec =
            BenchSpec::from_toml_str("# comentario\n[bench.Build]\nmax_runtime_s = 180\n\n").unwrap();
        assert_eq!(spec.thresholds.len(), 1);
        assert_eq!(spec.thresholds[0].max, 180.0);
    }

    #[test]
    fn invalid_value_is_an_error() {
        assert!(BenchSpec::from_toml_str("max_runtime_s = abc").is_err());
        assert!(BenchSpec::from_toml_str("sin_igual").is_err());
    }

    #[test]
    fn check_passes_within_limits() {
        let spec =
            BenchSpec::from_toml_str("max_runtime_s = 60\nmax_memory_mb = 512\nmin_test_coverage = 80\n")
                .unwrap();
        let m = Metrics {
            runtime_ms: 30_000,
            memory_mb: 100.0,
            test_coverage: 90.0,
        };
        let r = check(&spec, &m);
        assert!(r.passed, "fallos inesperados: {:?}", r.failures);
        assert!(r.failures.is_empty());
    }

    #[test]
    fn check_fails_when_exceeds_max() {
        let spec = BenchSpec::from_toml_str("max_runtime_s = 60\n").unwrap();
        let m = Metrics {
            runtime_ms: 70_000,
            memory_mb: 0.0,
            test_coverage: 0.0,
        };
        let r = check(&spec, &m);
        assert!(!r.passed);
        assert_eq!(r.failures, vec!["max_runtime_s: 70 > 60".to_string()]);
    }

    #[test]
    fn check_fails_on_low_min_coverage() {
        let spec = BenchSpec::from_toml_str("min_test_coverage = 80\n").unwrap();
        let m = Metrics {
            runtime_ms: 0,
            memory_mb: 0.0,
            test_coverage: 60.0,
        };
        let r = check(&spec, &m);
        assert!(!r.passed);
        assert!(r.failures.iter().any(|f| f.contains("min_test_coverage")));
    }

    #[test]
    fn check_equality_at_limit_passes() {
        let spec = BenchSpec::from_toml_str("max_runtime_s = 60\n").unwrap();
        let m = Metrics {
            runtime_ms: 60_000,
            memory_mb: 0.0,
            test_coverage: 0.0,
        };
        assert!(check(&spec, &m).passed);
    }

    #[test]
    fn measure_runtime_positive_for_sleep() {
        let ms = measure_runtime(|| std::thread::sleep(std::time::Duration::from_millis(10)));
        assert!(ms > 0);
    }

    #[test]
    fn regression_percentage() {
        assert_eq!(regression(110.0, 100.0), 10.0);
        assert_eq!(regression(90.0, 100.0), -10.0);
        assert_eq!(regression(100.0, 100.0), 0.0);
        assert_eq!(regression(5.0, 0.0), 0.0);
    }

    #[test]
    fn regression_detects_over_threshold() {
        assert!(is_regression(11.0, 10.0));
        assert!(!is_regression(10.0, 10.0));
        assert!(!is_regression(5.0, 10.0));
    }

    #[test]
    fn memory_does_not_panic() {
        let mb = measure_memory_mb();
        assert!(mb >= 0.0);
    }

    #[test]
    fn report_is_readable() {
        let spec =
            BenchSpec::from_toml_str("max_runtime_s = 60\nmin_test_coverage = 80\n").unwrap();
        let ok = check(
            &spec,
            &Metrics {
                runtime_ms: 1_000,
                memory_mb: 10.0,
                test_coverage: 99.0,
            },
        );
        let report = BenchReport::from_result(&ok);
        assert!(report.contains('✓'), "reporte: {report}");

        let bad = check(
            &spec,
            &Metrics {
                runtime_ms: 1_000,
                memory_mb: 10.0,
                test_coverage: 50.0,
            },
        );
        let report = BenchReport::from_result(&bad);
        assert!(report.contains("min_test_coverage: 50 < 80"), "reporte: {report}");
        assert!(report.contains('✓'), "el umbral que pasa también aparece: {report}");
    }
}
