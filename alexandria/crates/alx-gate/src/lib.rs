//! alx-gate — Verificación: build/test/lint, LSP discovery, evidencia.
//!
//! Runner de comandos reales con timeout y captura de salida (`run_command`),
//! verificación de fase (`verify_build`/`verify_tests`/`verify_lint`) que
//! produce [`Evidence`] para anexar al Task, y descubrimiento básico de
//! lenguajes del proyecto (`discover_langs`) con comandos de lint sugeridos.
//!
//! Contrato de `exit_code`: `-1` significa que el comando no completó
//! normalmente — fallo de spawn, comando no encontrado (el shell responde
//! 127), muerto por señal, o timeout (proceso matado).

pub mod lsp;

use alx_core::types::{Evidence, EvidenceKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Tamaño máximo de la cabecera de salida capturada en `stdout_head`.
const STDOUT_HEAD_MAX: usize = 4000;

/// Resultado de un comando real: código de salida, salida capturada y duración.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandOutcome {
    pub exit_code: i32,
    pub stdout_head: String,
    pub duration_ms: u128,
}

/// Lenguajes detectados en el directorio del proyecto.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectLangs {
    pub rust: bool,
    pub js: bool,
    pub python: bool,
}

/// Ejecuta `cmd` real (`sh -c cmd`), captura stdout+stderr (primeros ~4000
/// chars como `stdout_head`) y mide la duración. Si supera `timeout_ms`, mata
/// el proceso. Fallo de spawn / comando no encontrado / señal / timeout →
/// `exit_code = -1`.
pub fn run_command(cmd: &str, timeout_ms: u64) -> CommandOutcome {
    let start = Instant::now();

    let mut child = match spawn_shell(cmd) {
        Ok(c) => c,
        Err(_) => {
            return CommandOutcome {
                exit_code: -1,
                stdout_head: String::new(),
                duration_ms: start.elapsed().as_millis(),
            };
        }
    };

    // Drenar stdout y stderr en un thread: evita que el pipe lleno bloquee al
    // hijo (deadlock) y nos da la salida completa como evidencia.
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let reader = thread::spawn(move || {
        let mut out = Vec::new();
        let mut err = Vec::new();
        if let Some(mut s) = stdout {
            let _ = s.read_to_end(&mut out);
        }
        if let Some(mut s) = stderr {
            let _ = s.read_to_end(&mut err);
        }
        out.extend_from_slice(&err);
        out
    });

    let deadline = start + Duration::from_millis(timeout_ms);
    let exit_code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break normalize_status(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break -1;
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break -1,
        }
    };

    let merged: Vec<u8> = reader.join().unwrap_or_default();
    let stdout_head = truncate_head(&String::from_utf8_lossy(&merged));

    CommandOutcome {
        exit_code,
        stdout_head,
        duration_ms: start.elapsed().as_millis(),
    }
}

/// Corre el comando y devuelve `Evidence` con `kind CommandOutput` y
/// `passed = (exit_code == expected_exit)`.
pub fn gate_passed(cmd: &str, timeout_ms: u64, expected_exit: i32) -> Evidence {
    let out = run_command(cmd, timeout_ms);
    let passed = out.exit_code == expected_exit;
    let mut ev = Evidence::command_output(cmd, out.exit_code, &out.stdout_head, passed);
    ev.metrics.insert("duration_ms".into(), out.duration_ms as f64);
    ev
}

/// Verifica build: `kind BuildOutput`, pasa con exit 0.
pub fn verify_build(cmd: &str, timeout_ms: u64) -> Evidence {
    verify(cmd, timeout_ms, EvidenceKind::BuildOutput)
}

/// Verifica tests: `kind TestSummary`, pasa con exit 0.
pub fn verify_tests(cmd: &str, timeout_ms: u64) -> Evidence {
    verify(cmd, timeout_ms, EvidenceKind::TestSummary)
}

/// Verifica lint: `kind LintReport`, pasa con exit 0. Si falla, añade una
/// métrica `warnings` estimada contando líneas con "warning" en la salida.
pub fn verify_lint(cmd: &str, timeout_ms: u64) -> Evidence {
    let out = run_command(cmd, timeout_ms);
    let passed = out.exit_code == 0;
    let mut ev = Evidence {
        kind: EvidenceKind::LintReport,
        command: cmd.to_string(),
        exit_code: out.exit_code,
        stdout_head: out.stdout_head,
        passed,
        metrics: HashMap::new(),
    };
    ev.metrics.insert("duration_ms".into(), out.duration_ms as f64);
    if !passed {
        ev.metrics.insert("warnings".into(), estimate_warnings(&ev.stdout_head) as f64);
    }
    ev
}

/// Detecta lenguajes por presencia de archivos de manifiesto en `project_dir`:
/// Rust (Cargo.toml), JS (package.json), Python (pyproject.toml o requirements.txt).
pub fn discover_langs(project_dir: &str) -> ProjectLangs {
    let base = std::path::Path::new(project_dir);
    ProjectLangs {
        rust: base.join("Cargo.toml").is_file(),
        js: base.join("package.json").is_file(),
        python: base.join("pyproject.toml").is_file()
            || base.join("requirements.txt").is_file(),
    }
}

/// Comandos de lint por lenguaje detectado. No los ejecuta.
pub fn suggested_lints(langs: &ProjectLangs) -> Vec<String> {
    let mut lints = Vec::new();
    if langs.rust {
        lints.push("cargo clippy -- -D warnings".to_string());
    }
    if langs.js {
        lints.push("npx eslint .".to_string());
    }
    if langs.python {
        lints.push("uv run ruff check .".to_string());
    }
    lints
}

fn spawn_shell(cmd: &str) -> std::io::Result<Child> {
    Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

fn normalize_status(status: std::process::ExitStatus) -> i32 {
    match status.code() {
        // El shell responde 127 cuando el comando no existe → mismo sentinel
        // que "no ejecutó" (spawn fallido / matado por señal / timeout).
        Some(127) => -1,
        Some(code) => code,
        None => -1, // muerto por señal
    }
}

fn truncate_head(s: &str) -> String {
    s.chars().take(STDOUT_HEAD_MAX).collect()
}

fn estimate_warnings(output: &str) -> usize {
    output
        .lines()
        .filter(|l| l.to_ascii_lowercase().contains("warning"))
        .count()
}

fn verify(cmd: &str, timeout_ms: u64, kind: EvidenceKind) -> Evidence {
    let out = run_command(cmd, timeout_ms);
    let passed = out.exit_code == 0;
    let mut ev = Evidence {
        kind,
        command: cmd.to_string(),
        exit_code: out.exit_code,
        stdout_head: out.stdout_head,
        passed,
        metrics: HashMap::new(),
    };
    ev.metrics.insert("duration_ms".into(), out.duration_ms as f64);
    ev
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let unique = format!("alx-gate-{tag}-{}", alx_core::types::now_ms());
        std::env::temp_dir().join(unique)
    }

    #[test]
    fn gate_passed_echo_succeeds() {
        let ev = gate_passed("echo hello gate", 5000, 0);
        assert!(ev.passed);
        assert_eq!(ev.exit_code, 0);
        assert_eq!(ev.kind, EvidenceKind::CommandOutput);
        assert!(ev.metrics.contains_key("duration_ms"));
    }

    #[test]
    fn gate_passed_failing_exit_is_false() {
        let ev = gate_passed("exit 1", 5000, 0);
        assert!(!ev.passed);
        assert_eq!(ev.exit_code, 1);
    }

    #[test]
    fn unknown_command_yields_minus_one() {
        let out = run_command("alx_no_such_command_xyz_12345", 5000);
        assert_eq!(out.exit_code, -1);
    }

    #[test]
    fn timeout_kills_command() {
        // `:` es builtin de sh: el bucle no deja procesos huérfanos al matar.
        let out = run_command("while :; do :; done", 300);
        assert!(
            out.duration_ms < 5000,
            "se corto en {}ms (esperado < 5000)",
            out.duration_ms
        );
        assert_eq!(out.exit_code, -1);
    }

    #[test]
    fn verify_build_kind_and_pass() {
        let ev = verify_build("true", 5000);
        assert_eq!(ev.kind, EvidenceKind::BuildOutput);
        assert!(ev.passed);
        assert!(ev.metrics.contains_key("duration_ms"));
    }

    #[test]
    fn verify_tests_success() {
        let ev = verify_tests("true", 5000);
        assert_eq!(ev.kind, EvidenceKind::TestSummary);
        assert!(ev.passed);
    }

    #[test]
    fn verify_lint_failure_adds_warnings_metric() {
        let ev = verify_lint("echo 'warning: found a lint issue'; exit 1", 5000);
        assert_eq!(ev.kind, EvidenceKind::LintReport);
        assert!(!ev.passed);
        assert_eq!(ev.metrics.get("warnings"), Some(&1.0));
    }

    #[test]
    fn discover_langs_detects_rust() {
        let dir = temp_dir("rust");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        let langs = discover_langs(&dir.to_string_lossy());
        assert!(langs.rust);
        assert!(!langs.js);
        assert!(!langs.python);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn discover_langs_detects_js_and_python() {
        let dir = temp_dir("pyjs");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("package.json"), "{}").unwrap();
        std::fs::write(dir.join("requirements.txt"), "").unwrap();
        let langs = discover_langs(&dir.to_string_lossy());
        assert!(langs.js);
        assert!(langs.python);
        assert!(!langs.rust);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn discover_langs_detects_python_via_pyproject() {
        let dir = temp_dir("pypy");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pyproject.toml"), "").unwrap();
        let langs = discover_langs(&dir.to_string_lossy());
        assert!(langs.python);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn suggested_lints_cover_all_langs() {
        let langs = ProjectLangs { rust: true, js: true, python: true };
        let lints = suggested_lints(&langs);
        assert!(lints.iter().any(|l| l.contains("cargo clippy")));
        assert!(lints.iter().any(|l| l.contains("eslint")));
        assert!(lints.iter().any(|l| l.contains("ruff")));
    }

    #[test]
    fn suggested_lints_empty_for_no_langs() {
        let langs = ProjectLangs::default();
        assert!(suggested_lints(&langs).is_empty());
    }

    #[test]
    fn project_langs_json_roundtrip() {
        let langs = ProjectLangs { rust: true, js: false, python: true };
        let s = serde_json::to_string(&langs).unwrap();
        let back: ProjectLangs = serde_json::from_str(&s).unwrap();
        assert_eq!(back, langs);
    }
}
