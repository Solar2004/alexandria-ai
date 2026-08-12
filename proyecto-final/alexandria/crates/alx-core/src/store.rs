//! Store JSONL append-only. Fuente de verdad del estado en disco.
//!
//! Cada línea es un registro serializado. Escritura append-only = crash-safe:
//! un crash deja el estado en el último registro completo.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// Añade un registro (una línea) al archivo JSONL.
pub fn append_line<T: Serialize>(path: &Path, record: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    let line = serde_json::to_string(record)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    writeln!(f, "{line}")
}

/// Carga todos los registros válidos del archivo JSONL (ignora líneas corruptas).
pub fn load_lines<T: DeserializeOwned>(path: &Path) -> Vec<T> {
    if !path.exists() {
        return Vec::new();
    }
    let f = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    BufReader::new(f)
        .lines()
        .flatten()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(&l).ok())
        .collect()
}

/// Cuenta las líneas del archivo (0 si no existe).
pub fn count_lines(path: &Path) -> usize {
    load_lines::<serde_json::Value>(path).len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp_path(tag: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("alx-core-{tag}-{nanos}.jsonl"))
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Rec {
        id: u32,
        name: String,
    }

    #[test]
    fn append_then_load_roundtrip() {
        let p = tmp_path("roundtrip");
        let r1 = Rec { id: 1, name: "alpha".into() };
        let r2 = Rec { id: 2, name: "beta".into() };
        append_line(&p, &r1).unwrap();
        append_line(&p, &r2).unwrap();
        let loaded: Vec<Rec> = load_lines(&p);
        assert_eq!(loaded, vec![r1, r2]);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn missing_file_loads_empty() {
        let p = tmp_path("missing");
        let loaded: Vec<Rec> = load_lines(&p);
        assert!(loaded.is_empty());
    }

    #[test]
    fn corrupt_line_is_skipped() {
        let p = tmp_path("corrupt");
        let mut f = OpenOptions::new().create(true).append(true).open(&p).unwrap();
        writeln!(f, "not-json").unwrap();
        writeln!(f, r#"{{"id":1,"name":"ok"}}"#).unwrap();
        drop(f);
        let loaded: Vec<Rec> = load_lines(&p);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "ok");
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn count_lines_works() {
        let p = tmp_path("count");
        let r = Rec { id: 1, name: "x".into() };
        append_line(&p, &r).unwrap();
        append_line(&p, &r).unwrap();
        assert_eq!(count_lines(&p), 2);
        let _ = fs::remove_file(&p);
    }
}
