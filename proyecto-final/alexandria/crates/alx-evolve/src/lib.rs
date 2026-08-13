//! alx-evolve — harness evolutivo / self-evolución.
//!
//! La AI crea harnesses en tiempo real: detecta qué formalizar, los crea con
//! documentación mínima, los aplica, y un watcher de objetivos los retira
//! (temporales cumplidos) o los promueve (demostraron servir). Nada se escapa
//! sin doc-min. Spec: plan/16-evolve.md.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Write;

/// Vida de un harness: temporal (muere al cumplir objetivo) o permanente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HarnessKind {
    Temporal,
    Permanent,
}

/// Estado del lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HarnessState {
    Active,
    WaitingObjective,
    Retired,
    Promoted,
}

/// Cuándo corre el harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Trigger {
    Event(String),   // "PostToolUse", "PhasePassed", ...
    Phase(String),   // "Build", "Review", ...
    Manual,
}

/// Candidato a harness detectado sobre trabajo real (heurística determinista).
/// El `kind` es la recomendación del detector (qué merece formalizarse); el
/// harness materializado empieza siempre Temporal y se promueve con evidencia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessCandidate {
    pub suggested_name: String,
    pub kind: HarnessKind,
    pub trigger: Trigger,
    pub objective: String,
    pub doc: String,
}

/// Un harness del sistema evolutivo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Harness {
    pub id: String,          // "hx-<slug>"
    pub name: String,
    pub kind: HarnessKind,
    pub trigger: Trigger,
    pub objective: String,
    /// Documentación mínima obligatoria: qué, por qué, cuándo.
    pub doc: String,
    pub state: HarnessState,
    pub created_by: String,
    pub created: u64,
    pub uses: u32,
}

impl Harness {
    /// Crea un harness nuevo. `doc` vacía = inválido (regla doc-min).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        kind: HarnessKind,
        trigger: Trigger,
        objective: impl Into<String>,
        doc: impl Into<String>,
        created_by: impl Into<String>,
        created: u64,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            trigger,
            objective: objective.into(),
            doc: doc.into(),
            state: match kind {
                HarnessKind::Temporal => HarnessState::WaitingObjective,
                HarnessKind::Permanent => HarnessState::Active,
            },
            created_by: created_by.into(),
            created,
            uses: 0,
        }
    }

    /// Un harness temporal cumple su objetivo → se retira (autodestrucción).
    pub fn retire_if_goal_met(&mut self) -> bool {
        if self.kind == HarnessKind::Temporal && self.state == HarnessState::WaitingObjective {
            self.state = HarnessState::Retired;
            return true;
        }
        false
    }

    /// Temporal que demostró servir (usos >= umbral) → promueve a permanente.
    pub fn promote(&mut self, min_uses: u32) -> bool {
        if self.kind == HarnessKind::Temporal
            && self.state == HarnessState::WaitingObjective
            && self.uses >= min_uses
        {
            self.kind = HarnessKind::Permanent;
            self.state = HarnessState::Promoted;
            return true;
        }
        false
    }

    /// Registra una aplicación del harness.
    pub fn record_use(&mut self) {
        self.uses = self.uses.saturating_add(1);
    }
}

/// Registry de harnesses. Estado en memoria; el disco vive en
/// `proyecto-final/harnesses/` (active/*.toml + index.toml).
#[derive(Debug, Default, Clone)]
pub struct HarnessRegistry {
    harnesses: Vec<Harness>,
}

/// Resumen de un ciclo del watcher persistente (`watcher_cycle`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatcherSummary {
    pub loaded: usize,
    pub retired: Vec<String>,
    pub promoted: Vec<String>,
    pub live: usize,
}

impl HarnessRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, h: Harness) {
        self.harnesses.push(h);
    }

    /// Materializa un candidato como harness (id `hx-<suggested_name>`).
    /// Doc-min es compuerta: doc < 20 chars → None. También None si el id ya
    /// existe. El harness empieza Temporal por defecto (plan §11: "todo harness
    /// nuevo empieza temporal; solo se promueve con evidencia de utilidad").
    pub fn add_candidate(&mut self, c: HarnessCandidate, now: u64) -> Option<String> {
        let id = format!("hx-{}", c.suggested_name);
        if self.by_id(&id).is_some() {
            return None;
        }
        if c.doc.trim().chars().count() < 20 {
            return None;
        }
        let h = Harness::new(
            id.clone(),
            c.suggested_name,
            HarnessKind::Temporal,
            c.trigger,
            c.objective,
            c.doc,
            "alx-evolve",
            now,
        );
        self.add(h);
        Some(id)
    }

    pub fn all(&self) -> &[Harness] {
        &self.harnesses
    }

    pub fn by_id(&self, id: &str) -> Option<&Harness> {
        self.harnesses.iter().find(|h| h.id == id)
    }

    pub fn by_id_mut(&mut self, id: &str) -> Option<&mut Harness> {
        self.harnesses.iter_mut().find(|h| h.id == id)
    }

    /// Watcher de objetivos: retira temporales que cumplieron. Devuelve los
    /// ids retirados.
    pub fn run_watcher(&mut self, goal_met: &dyn Fn(&Harness) -> bool) -> Vec<String> {
        let mut retired = Vec::new();
        for h in self.harnesses.iter_mut() {
            if h.state == HarnessState::WaitingObjective && goal_met(h) {
                h.state = HarnessState::Retired;
                retired.push(h.id.clone());
            }
        }
        retired
    }

    /// Promueve temporales con suficiente uso.
    pub fn promote_used(&mut self, min_uses: u32) -> Vec<String> {
        let mut promoted = Vec::new();
        for h in self.harnesses.iter_mut() {
            if h.promote(min_uses) {
                promoted.push(h.id.clone());
            }
        }
        promoted
    }

    /// Nº de harnesses vivos (no Retired).
    pub fn live_count(&self) -> usize {
        self.harnesses.iter().filter(|h| h.state != HarnessState::Retired).count()
    }

    /// Persiste el registry como JSONL en `dir` (plan 16 §8): los activos
    /// (Active/WaitingObjective/Promoted) sobrescriben `active/harnesses.jsonl`
    /// y los retirados sobrescriben `archive/retired.jsonl`. Crea los dirs si
    /// faltan. Write (no append) para reflejar el estado actual.
    pub fn save_to(&self, dir: &std::path::Path) -> std::io::Result<()> {
        let active_dir = dir.join("active");
        let archive_dir = dir.join("archive");
        std::fs::create_dir_all(&active_dir)?;
        std::fs::create_dir_all(&archive_dir)?;

        let active_path = active_dir.join("harnesses.jsonl");
        let mut active_out = std::fs::File::create(&active_path)?;
        for h in self.harnesses.iter().filter(|h| h.state != HarnessState::Retired) {
            let line = serde_json::to_string(h).map_err(std::io::Error::other)?;
            writeln!(active_out, "{line}")?;
        }

        let retired_path = archive_dir.join("retired.jsonl");
        let mut retired_out = std::fs::File::create(&retired_path)?;
        for h in self.harnesses.iter().filter(|h| h.state == HarnessState::Retired) {
            let line = serde_json::to_string(h).map_err(std::io::Error::other)?;
            writeln!(retired_out, "{line}")?;
        }

        Ok(())
    }

    /// Carga el registry desde `dir`: lee `active/harnesses.jsonl` (y
    /// `archive/retired.jsonl` si existe) línea a línea; las corruptas se
    /// ignoran. Devuelve un registry con todos los harnesses leídos.
    pub fn load_from(dir: &std::path::Path) -> Self {
        let mut reg = Self::new();
        for h in read_jsonl(&dir.join("active").join("harnesses.jsonl")) {
            reg.add(h);
        }
        for h in read_jsonl(&dir.join("archive").join("retired.jsonl")) {
            reg.add(h);
        }
        reg
    }

    /// Nº de harnesses activos persistidos en disco (líneas válidas de
    /// `active/harnesses.jsonl`).
    pub fn count_in(dir: &std::path::Path) -> usize {
        read_jsonl(&dir.join("active").join("harnesses.jsonl")).len()
    }

    /// Ciclo completo del watcher persistente: carga el estado, retira los
    /// temporales que cumplieron objetivo, promueve los que demostraron uso,
    /// y persiste el resultado. Devuelve un resumen del ciclo. Un fallo de
    /// disco no aborta el ciclo (el estado en memoria ya quedó aplicado).
    pub fn watcher_cycle(
        dir: &std::path::Path,
        goal_met: &dyn Fn(&Harness) -> bool,
        min_uses: u32,
    ) -> WatcherSummary {
        let mut reg = Self::load_from(dir);
        let loaded = reg.all().len();
        let retired = reg.run_watcher(goal_met);
        let promoted = reg.promote_used(min_uses);
        let live = reg.live_count();
        let _ = reg.save_to(dir);
        WatcherSummary { loaded, retired, promoted, live }
    }
}

/// Lee un archivo JSONL línea a línea; líneas vacías o corruptas se ignoran.
/// Archivo ausente = lista vacía.
fn read_jsonl(path: &std::path::Path) -> Vec<Harness> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            serde_json::from_str(line).ok()
        })
        .collect()
}

/// doc-min: un harness sin documentación no puede registrarse.
pub fn validate_doc_min(h: &Harness) -> Result<(), String> {
    if h.doc.trim().is_empty() {
        return Err(format!("harness {} sin doc-min (obligatoria)", h.id));
    }
    if h.doc.trim().chars().count() < 20 {
        return Err(format!("harness {} doc-min demasiado corta (<20 chars)", h.id));
    }
    Ok(())
}

/// Detección heurística determinista de candidatos a harness a partir de
/// trabajo real (hook `evolve.detect`, PostToolUse). Solo señala patrones
/// verificables; el orden de salida es estable.
pub fn detect_candidates(work_text: &str) -> Vec<HarnessCandidate> {
    let mut out = Vec::new();

    // Colores hardcodeados → design-tokens (permanente, corre en Build).
    if contains_hex_color(work_text) {
        out.push(HarnessCandidate {
            suggested_name: "design-tokens".into(),
            kind: HarnessKind::Permanent,
            trigger: Trigger::Phase("Build".into()),
            objective: "consistencia visual".into(),
            doc: "Usa tokens de diseño, sin hex literales hardcodeados".into(),
        });
    }

    // Mención de regla/convención → regla-formalizada.
    if work_text.contains("siempre") || work_text.contains("regla") || work_text.contains("nunca")
    {
        out.push(HarnessCandidate {
            suggested_name: "regla-formalizada".into(),
            kind: HarnessKind::Permanent,
            trigger: Trigger::Manual,
            objective: "formalizar regla como check".into(),
            doc: "Convierte una convencion repetida en harness verificable".into(),
        });
    }

    // Repetición de término técnico (≥5 chars, ≥4 veces) → abstraer-<palabra>.
    for word in repeated_terms(work_text) {
        out.push(HarnessCandidate {
            suggested_name: format!("abstraer-{word}"),
            kind: HarnessKind::Temporal,
            trigger: Trigger::Manual,
            objective: format!("extraer {word} a util reutilizable"),
            doc: "Detecta termino repetido: extraer a abstraccion".into(),
        });
    }

    // Bloque repetido (≥50 chars, ≥2 veces) → extraer-util.
    if has_repeated_block(work_text) {
        out.push(HarnessCandidate {
            suggested_name: "extraer-util".into(),
            kind: HarnessKind::Temporal,
            trigger: Trigger::Manual,
            objective: "extraer fragmento duplicado a util compartido".into(),
            doc: "Detecta un bloque de codigo repetido para extraer a util".into(),
        });
    }

    // Dedup por suggested_name (Set), preservando el orden de detección.
    let mut seen: HashSet<String> = HashSet::new();
    out.retain(|c| seen.insert(c.suggested_name.clone()));
    out
}

/// `#RRGGBB`: '#' + 6 hex digits, no seguido de otro hex digit (excluye RGBA de 8).
fn contains_hex_color(text: &str) -> bool {
    let b = text.as_bytes();
    for i in 0..b.len() {
        if b[i] == b'#' {
            let end = i + 7;
            if end <= b.len()
                && b[i + 1..end].iter().all(|c| c.is_ascii_hexdigit())
                && (end == b.len() || !b[end].is_ascii_hexdigit())
            {
                return true;
            }
        }
    }
    false
}

/// Palabras alfanuméricas (≥5 chars) que aparecen ≥4 veces, ordenadas.
fn repeated_terms(text: &str) -> Vec<String> {
    let mut counts: HashMap<String, u32> = HashMap::new();
    let mut cur = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            cur.push(c);
        } else {
            if cur.chars().count() >= 5 {
                *counts.entry(cur.clone()).or_insert(0) += 1;
            }
            cur.clear();
        }
    }
    if cur.chars().count() >= 5 {
        *counts.entry(cur.clone()).or_insert(0) += 1;
    }
    let mut terms: Vec<String> = counts
        .iter()
        .filter(|(_, &n)| n >= 4)
        .map(|(w, _)| w.clone())
        .collect();
    terms.sort();
    terms
}

/// Un fragmento de ≥50 chars aparece ≥2 veces (ventana fija de 50 bytes).
fn has_repeated_block(text: &str) -> bool {
    const WIN: usize = 50;
    let b = text.as_bytes();
    if b.len() <= WIN {
        return false;
    }
    let mut seen: HashSet<&[u8]> = HashSet::with_capacity(b.len() - WIN);
    for i in 0..=(b.len() - WIN) {
        if !seen.insert(&b[i..i + WIN]) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn harness(id: &str, kind: HarnessKind, now: u64) -> Harness {
        Harness::new(
            id,
            id,
            kind,
            Trigger::Phase("Build".into()),
            "mantener consistencia",
            "Verifica que la salida de la fase cumple las reglas establecidas del proyecto.",
            "alx-evolve",
            now,
        )
    }

    #[test]
    fn temporal_starts_waiting_permanent_active() {
        let t = harness("t1", HarnessKind::Temporal, 1);
        let p = harness("p1", HarnessKind::Permanent, 1);
        assert_eq!(t.state, HarnessState::WaitingObjective);
        assert_eq!(p.state, HarnessState::Active);
    }

    #[test]
    fn retire_when_goal_met() {
        let mut h = harness("t1", HarnessKind::Temporal, 1);
        assert!(h.retire_if_goal_met());
        assert_eq!(h.state, HarnessState::Retired);
        // doble retiro no hace nada
        assert!(!h.retire_if_goal_met());
    }

    #[test]
    fn permanent_never_retires_by_goal() {
        let mut h = harness("p1", HarnessKind::Permanent, 1);
        assert!(!h.retire_if_goal_met());
        assert_eq!(h.state, HarnessState::Active);
    }

    #[test]
    fn promote_with_enough_uses() {
        let mut h = harness("t1", HarnessKind::Temporal, 1);
        h.record_use();
        h.record_use();
        assert!(h.promote(2));
        assert_eq!(h.kind, HarnessKind::Permanent);
        assert_eq!(h.state, HarnessState::Promoted);
        // no se promueve de nuevo
        assert!(!h.promote(2));
    }

    #[test]
    fn watcher_retires_only_matching_temporals() {
        let mut reg = HarnessRegistry::new();
        reg.add(harness("t1", HarnessKind::Temporal, 1));
        reg.add(harness("p1", HarnessKind::Permanent, 1));
        let retired = reg.run_watcher(&|_| true);
        assert_eq!(retired, vec!["t1".to_string()]);
        assert_eq!(reg.live_count(), 1);
    }

    #[test]
    fn doc_min_rejects_empty() {
        let mut h = harness("t1", HarnessKind::Temporal, 1);
        h.doc = String::new();
        assert!(validate_doc_min(&h).is_err());
        h.doc = "corta".into();
        assert!(validate_doc_min(&h).is_err());
        h.doc = "Verifica que la salida de la fase cumple las reglas establecidas.".into();
        assert!(validate_doc_min(&h).is_ok());
    }

    fn candidate(name: &str, kind: HarnessKind, doc: &str) -> HarnessCandidate {
        HarnessCandidate {
            suggested_name: name.into(),
            kind,
            trigger: Trigger::Manual,
            objective: "objetivo".into(),
            doc: doc.into(),
        }
    }

    #[test]
    fn detect_hex_color_creates_design_tokens() {
        let cands = detect_candidates("usa #FF0000 para el boton y #00FF00 para el fondo");
        let dt = cands
            .iter()
            .find(|c| c.suggested_name == "design-tokens")
            .expect("design-tokens");
        assert_eq!(dt.kind, HarnessKind::Permanent);
        assert_eq!(dt.trigger, Trigger::Phase("Build".into()));
        // 8-digit RGBA no cuenta como #RRGGBB → sin candidato design-tokens
        assert!(detect_candidates("color #FF0000ff con alpha").is_empty());
    }

    #[test]
    fn detect_repeated_term_creates_abstraer() {
        let cands = detect_candidates("cache cache cache cache al final");
        let a = cands
            .iter()
            .find(|c| c.suggested_name == "abstraer-cache")
            .expect("abstraer-cache");
        assert_eq!(a.kind, HarnessKind::Temporal);
        assert!(a.objective.contains("cache"));
    }

    #[test]
    fn detect_rule_word_creates_regla_formalizada() {
        let cands = detect_candidates("regla de negocio");
        let r = cands
            .iter()
            .find(|c| c.suggested_name == "regla-formalizada")
            .expect("regla-formalizada");
        assert_eq!(r.kind, HarnessKind::Permanent);
    }

    #[test]
    fn detect_repeated_block_creates_extraer_util() {
        let block = "let resultado = calcularValorComplejo(inputA, inputB, contexto); ";
        let text = format!("{block} luego {block}");
        let cands = detect_candidates(&text);
        assert!(
            cands.iter().any(|c| c.suggested_name == "extraer-util"),
            "bloque repetido deberia dar extraer-util: {cands:?}"
        );
    }

    #[test]
    fn detect_candidates_dedupe_by_name() {
        // #FF0000, "regla"+"siempre", y "cache"×4: tres candidatos distintos.
        let cands = detect_candidates("#FF0000 #FF0000 regla siempre cache cache cache cache");
        let names: Vec<&str> = cands.iter().map(|c| c.suggested_name.as_str()).collect();
        let unique: HashSet<&str> = names.iter().copied().collect();
        assert_eq!(names.len(), unique.len(), "sin duplicados: {names:?}");
        assert!(names.contains(&"design-tokens"));
        assert!(names.contains(&"regla-formalizada"));
        assert!(names.contains(&"abstraer-cache"));
    }

    #[test]
    fn add_candidate_registers_and_rejects_duplicate() {
        let mut reg = HarnessRegistry::new();
        let doc = "Usa tokens de diseno, sin hex literales hardcodeados en el codigo";
        let id = reg.add_candidate(candidate("design-tokens", HarnessKind::Permanent, doc), 100);
        assert_eq!(id.as_deref(), Some("hx-design-tokens"));
        assert!(reg.by_id("hx-design-tokens").is_some());

        let dup = candidate("design-tokens", HarnessKind::Permanent, doc);
        assert_eq!(reg.add_candidate(dup, 100), None);
        assert_eq!(reg.live_count(), 1);
    }

    #[test]
    fn add_candidate_starts_temporal_by_default() {
        // Plan §11: todo harness nuevo empieza temporal aunque el detector
        // recomiende permanente; se promueve solo con evidencia de utilidad.
        let mut reg = HarnessRegistry::new();
        let doc = "Usa tokens de diseno, sin hex literales hardcodeados en el codigo";
        reg.add_candidate(candidate("design-tokens", HarnessKind::Permanent, doc), 1);
        let h = reg.by_id("hx-design-tokens").unwrap();
        assert_eq!(h.kind, HarnessKind::Temporal);
        assert_eq!(h.state, HarnessState::WaitingObjective);
        assert_eq!(h.created_by, "alx-evolve");
    }

    #[test]
    fn add_candidate_rejects_short_doc() {
        let mut reg = HarnessRegistry::new();
        assert_eq!(reg.add_candidate(candidate("corto", HarnessKind::Temporal, "corta"), 1), None);
        assert!(reg.by_id("hx-corto").is_none());
    }

    /// Directorio temporal único (pid + nanos), limpiado al final del test.
    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("alx-evolve-{tag}-{pid}-{nanos}"))
    }

    #[test]
    fn save_load_roundtrip_preserves_ids_and_states() {
        let dir = temp_dir("roundtrip");
        let mut reg = HarnessRegistry::new();
        reg.add(harness("t1", HarnessKind::Temporal, 1));  // WaitingObjective
        reg.add(harness("p1", HarnessKind::Permanent, 1)); // Active
        reg.save_to(&dir).expect("save_to");

        let loaded = HarnessRegistry::load_from(&dir);
        let ids: Vec<&str> = loaded.all().iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, vec!["t1", "p1"]);
        assert_eq!(loaded.by_id("t1").unwrap().state, HarnessState::WaitingObjective);
        assert_eq!(loaded.by_id("p1").unwrap().state, HarnessState::Active);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn watcher_cycle_retires_promotes_and_persists() {
        let dir = temp_dir("cycle");
        let mut reg = HarnessRegistry::new();
        reg.add(harness("t1", HarnessKind::Temporal, 1)); // cumple objetivo -> retira
        let mut t2 = harness("t2", HarnessKind::Temporal, 1);
        t2.record_use();
        t2.record_use();
        reg.add(t2);
        reg.save_to(&dir).expect("save inicial");

        let summary = HarnessRegistry::watcher_cycle(&dir, &|h| h.id == "t1", 2);
        assert_eq!(summary.loaded, 2);
        assert_eq!(summary.retired, vec!["t1".to_string()]);
        assert_eq!(summary.promoted, vec!["t2".to_string()]);
        assert_eq!(summary.live, 1);

        // el disco refleja los cambios
        let after = HarnessRegistry::load_from(&dir);
        assert_eq!(after.by_id("t1").unwrap().state, HarnessState::Retired);
        assert_eq!(after.by_id("t2").unwrap().state, HarnessState::Promoted);
        assert_eq!(after.by_id("t2").unwrap().kind, HarnessKind::Permanent);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn count_in_counts_active_persisted() {
        let dir = temp_dir("count");
        let mut reg = HarnessRegistry::new();
        reg.add(harness("t1", HarnessKind::Temporal, 1));
        reg.add(harness("p1", HarnessKind::Permanent, 1));
        reg.save_to(&dir).expect("save_to");
        assert_eq!(HarnessRegistry::count_in(&dir), 2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_from_empty_dir_is_empty() {
        let dir = temp_dir("empty");
        let reg = HarnessRegistry::load_from(&dir);
        assert_eq!(reg.all().len(), 0);
        assert_eq!(reg.live_count(), 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_from_ignores_corrupt_lines() {
        let dir = temp_dir("corrupt");
        std::fs::create_dir_all(dir.join("active")).expect("mkdir");
        let h = harness("t1", HarnessKind::Temporal, 1);
        let line = serde_json::to_string(&h).expect("serialize");
        let content = format!("{line}\nesto no es json\n");
        std::fs::write(dir.join("active").join("harnesses.jsonl"), content).expect("write");

        let reg = HarnessRegistry::load_from(&dir);
        assert_eq!(reg.all().len(), 1);
        assert_eq!(reg.by_id("t1").unwrap().state, HarnessState::WaitingObjective);

        std::fs::remove_dir_all(&dir).ok();
    }
}
