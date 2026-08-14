//! alx-memory — Auto-recalls: capturar, comprimir caveman, inyectar.
//!
//! Motor de auto-memoria de ALEXANDRIA (spec 05-hooks-system, sección auto-memoria).
//! Elimina la repetición: cada aprendizaje que el dev repite a la AI se captura,
//! comprime a estilo caveman y se re-inyecta en sesiones futuras con peso.
//!
//! Piezas:
//! - [`compress`]: compresión determinista — quita relleno, conserva lo técnico/código.
//! - [`RecallStore`]: almacena, deduplica (por texto case-insensitive), refuerza,
//!   ordena por peso e inyecta con presupuesto de chars.
//!
//! El flujo esperado del hook `memory.capture`:
//! ```text
//! frase cruda → compress() → Recall{ text, ... } → store.add() → top_n_by_weight → inject_budget
//! ```

use serde::{Deserialize, Serialize};

pub use alx_core::types::{now_ms, Recall, RecallSource};

/// Palabras de relleno que `compress` elimina (comparación case-insensitive).
/// Solo se quitan cuando aparecen como palabra independiente; los términos
/// técnicos y fragmentos de código quedan intactos.
const STOPWORDS: &[&str] = &[
    // Español
    "el", "la", "los", "las", "un", "una", "unos", "unas", "de", "del", "en", "para", "con", "y",
    "o", "que", "es", "son", "está",
    // Inglés
    "the", "a", "an", "of", "in", "for", "with", "and", "or", "to", "is", "are", "it", "this",
    "that",
];

/// ¿Es esta palabra de relleno? Se comparan tras recortar puntuación de borde
/// (p.ej. `con,` → `con`), así el relleno se cae aunque lleve coma/paréntesis.
fn is_stopword(token: &str) -> bool {
    let trimmed = token.trim_matches(|c: char| !c.is_alphanumeric());
    STOPWORDS.contains(&trimmed.to_lowercase().as_str())
}

/// Compresión caveman determinista.
///
/// - Divide por cualquier run de whitespace (colapsa espacios, tabulaciones, saltos).
/// - Elimina palabras de relleno que aparecen como token independiente.
/// - Conserva el resto **verbatim**: ni minúsculas ni división de términos técnicos
///   o fragmentos de código (`UserService::create_user`, `<=`).
pub fn compress(text: &str) -> String {
    text.split_whitespace()
        .filter(|tok| !is_stopword(tok))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Almacén de auto-recalls.
///
/// Sin hilos ni IO: memoria viva que el hook `memory.inject` consulta al arrancar
/// la sesión. Persistible a JSON (serde) para el `memory.commit` de SessionStop.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecallStore {
    recalls: Vec<Recall>,
}

impl RecallStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.recalls.len()
    }

    pub fn is_empty(&self) -> bool {
        self.recalls.is_empty()
    }

    /// Añade un recall. Si ya existe texto idéntico (case-insensitive), **no duplica**:
    /// suma el `weight` del entrante al existente. Devuelve el id del recall efectivo
    /// (el existente si hubo dedup, si no el recién añadido).
    ///
    /// Recomendado comprimir antes: `store.add(Recall { text: compress(raw), .. })`.
    pub fn add(&mut self, recall: Recall) -> String {
        let key = recall.text.to_lowercase();
        if let Some(existing) = self
            .recalls
            .iter_mut()
            .find(|r| r.text.to_lowercase() == key)
        {
            existing.weight = existing.weight.saturating_add(recall.weight);
            existing.id.clone()
        } else {
            let id = recall.id.clone();
            self.recalls.push(recall);
            id
        }
    }

    /// Todos los recalls, en orden de inserción.
    pub fn all(&self) -> &[Recall] {
        &self.recalls
    }

    /// Recalls que tienen el tag dado (comparación case-insensitive).
    pub fn by_tag(&self, tag: &str) -> Vec<&Recall> {
        let needle = tag.to_lowercase();
        self.recalls
            .iter()
            .filter(|r| r.tags.iter().any(|t| t.to_lowercase() == needle))
            .collect()
    }

    /// Refuerza un recall (se llamó cuando ayudó a evitar un error): `weight += 1`.
    /// Devuelve `false` si el id no existe.
    pub fn reinforce(&mut self, id: &str) -> bool {
        match self.recalls.iter_mut().find(|r| r.id == id) {
            Some(r) => {
                r.weight = r.weight.saturating_add(1);
                true
            }
            None => false,
        }
    }

    /// Los `n` recalls con mayor peso, ordenados de mayor a menor (estable en empates).
    pub fn top_n_by_weight(&self, n: usize) -> Vec<&Recall> {
        let mut top: Vec<&Recall> = self.recalls.iter().collect();
        // Estable: empates conservan el orden de inserción.
        top.sort_by_key(|r| std::cmp::Reverse(r.weight));
        top.truncate(n);
        top
    }

    /// Filtra una lista ya ordenada (p.ej. salida de `top_n_by_weight`) para inyección
    /// en prompts: elige un prefijo cuya suma de chars de `text` no supere `max_chars`.
    /// Descarta (en orden) los recalls que individualmente desbordarían el presupuesto.
    pub fn inject_budget(top_n: Vec<&Recall>, max_chars: usize) -> Vec<&Recall> {
        let mut used = 0usize;
        let mut out = Vec::with_capacity(top_n.len());
        for recall in top_n {
            let cost = recall.text.chars().count();
            if used + cost > max_chars {
                continue;
            }
            used += cost;
            out.push(recall);
        }
        out
    }

    /// Elimina recalls caducados: los creados antes de `now - max_age_ms`.
    /// Devuelve cuántos se eliminaron.
    pub fn prune_older_than(&mut self, max_age_ms: u64, now: u64) -> usize {
        let cutoff = now.saturating_sub(max_age_ms);
        let before = self.recalls.len();
        self.recalls.retain(|r| r.created >= cutoff);
        before - self.recalls.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alx_core::types::RecallSource;

    fn recall(id: &str, text: &str, weight: u32, created: u64) -> Recall {
        Recall {
            id: id.to_string(),
            text: text.to_string(),
            source: RecallSource::Tool,
            tags: vec!["auth".to_string()],
            weight,
            created,
        }
    }

    #[test]
    fn compress_reduce_and_preserve_tech() {
        let raw = "el auth token expiry check usa < no <= para que el token no caduque antes de tiempo en el sistema";
        let out = compress(raw);
        // Contrato exacto: se caen los 7 rellenos, lo técnico queda verbatim.
        assert_eq!(out, "auth token expiry check usa < no <= token no caduque antes tiempo sistema");
        assert!(out.len() < raw.len());
    }

    #[test]
    fn compress_heavy_filler_reduces_significantly() {
        let raw = "el un la los las de del en para con y o que es son está the a an of in for with and or to is are it this that token";
        let out = compress(raw);
        // Solo sobrevive el contenido; ~35 tokens de relleno se caen.
        assert_eq!(out, "token");
        assert!(out.len() * 5 < raw.len());
    }

    #[test]
    fn compress_preserves_code_and_terms_verbatim() {
        let raw = "esto es el token con formato JWT y el parser usa serde_json::from_str";
        let out = compress(raw);
        assert_eq!(out, "esto token formato JWT parser usa serde_json::from_str");
        // Código intacto: ni minúsculas ni división del identificador.
        assert!(out.contains("serde_json::from_str"));
        assert!(out.contains("JWT"));
    }

    #[test]
    fn compress_collapses_whitespace() {
        assert_eq!(compress("el  token   usa\n\n  auth"), "token usa auth");
        assert_eq!(compress(""), "");
        // Solo relleno -> vacío.
        assert_eq!(compress("el de y que"), "");
    }

    #[test]
    fn add_dedup_no_duplicate_and_sums_weight() {
        let mut store = RecallStore::new();
        store.add(recall("r1", "auth token expiry check", 1, 0));
        // Texto idéntico en mayúsculas -> dedup, suma weight.
        store.add(recall("r2", "AUTH TOKEN EXPIRY CHECK", 2, 0));
        assert_eq!(store.len(), 1);
        assert_eq!(store.all()[0].weight, 3);
        // Y el id efectivo es el existente.
        assert_eq!(store.add(recall("r3", "auth token expiry check", 1, 0)), "r1");
        assert_eq!(store.len(), 1);
        assert_eq!(store.all()[0].weight, 4);
    }

    #[test]
    fn add_distinct_text_is_kept() {
        let mut store = RecallStore::new();
        store.add(recall("r1", "auth expiry <", 1, 0));
        store.add(recall("r2", "cache invalidation on write", 1, 0));
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn reinforce_increments_weight() {
        let mut store = RecallStore::new();
        store.add(recall("r1", "auth token expiry check", 1, 0));
        assert!(store.reinforce("r1"));
        assert_eq!(store.all()[0].weight, 2);
        // Id inexistente -> false, no cambia nada.
        assert!(!store.reinforce("nope"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn top_n_ordered_by_weight_desc() {
        let mut store = RecallStore::new();
        store.add(recall("a", "low", 1, 0));
        store.add(recall("b", "high", 5, 0));
        store.add(recall("c", "mid", 3, 0));

        let top2 = store.top_n_by_weight(2);
        assert_eq!(top2.len(), 2);
        assert_eq!(top2[0].id, "b");
        assert_eq!(top2[1].id, "c");

        let all = store.top_n_by_weight(10);
        let ids: Vec<&str> = all.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "c", "a"]);
    }

    #[test]
    fn inject_budget_respects_max_chars() {
        let mut store = RecallStore::new();
        store.add(recall("a", "tok", 3, 0));
        store.add(recall("b", "auth", 2, 0));
        store.add(recall("c", "cache", 1, 0));

        let top = store.top_n_by_weight(3);
        let filtered = RecallStore::inject_budget(top, 7); // "tok"=3 + "auth"=4; "cache"=5 no cabe.
        let total: usize = filtered.iter().map(|r| r.text.chars().count()).sum();
        assert!(total <= 7);
        assert_eq!(filtered.len(), 2);

        // Presupuesto 0 -> nada.
        let top = store.top_n_by_weight(3);
        assert!(RecallStore::inject_budget(top, 0).is_empty());
    }

    #[test]
    fn by_tag_filters_case_insensitive() {
        let mut store = RecallStore::new();
        store.add(recall("r1", "auth expiry <", 1, 0));
        store.add(Recall {
            tags: vec!["cache".to_string()],
            ..recall("r2", "write invalidation", 1, 0)
        });
        assert_eq!(store.by_tag("AUTH").len(), 1);
        assert_eq!(store.by_tag("cache").len(), 1);
        assert!(store.by_tag("missing").is_empty());
    }

    #[test]
    fn prune_removes_old_keeps_recent() {
        let now = 1_000u64;
        let mut store = RecallStore::new();
        store.add(recall("old", "expired", 1, 500)); // 500 < now-100 = 900 -> fuera
        store.add(recall("mid", "alive", 1, 950)); // dentro
        store.add(recall("new", "recent", 1, 2_000)); // dentro
        assert_eq!(store.prune_older_than(100, now), 1);
        assert_eq!(store.len(), 2);
        assert!(store.by_tag("auth").iter().all(|r| r.id != "old"));

        // now menor que max_age -> saturating, no borra nada.
        assert_eq!(store.prune_older_than(10_000, 100), 0);
    }

    #[test]
    fn serde_roundtrip() {
        let mut store = RecallStore::new();
        store.add(recall("r1", "auth token expiry check", 2, 100));
        store.add(recall("r2", "cache invalidation", 1, 200));
        let json = serde_json::to_string(&store).unwrap();
        let back: RecallStore = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back.all()[0].weight, 2);
        assert_eq!(back.by_tag("auth").len(), 2);
    }
}
