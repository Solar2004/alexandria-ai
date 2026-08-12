//! alx-audit — Registry dedup: skills/agents/plugins/hooks, doctor.
//!
//! Inventario de TODO lo que existe en el ecosistema (global + repo + MCP + red)
//! para conectar todo, no perder nada y eliminar duplicados (plan 14).
//!
//! # Modelo
//!
//! Un [`AuditItem`] es dato puro (serde-serializable): un `id` único, `name`,
//! `kind` (qué tipo de componente es), `path` donde vive, `source` (global /
//! repo / plugin / …) y `description`.
//!
//! [`AuditIndex`] es el registry dedup: indexa items por `name` + `kind`. Dos
//! items con el mismo `name` y `kind` cuentan como el mismo componente
//! duplicado — `add_dedup` no lo re-añade y `duplicates` lo reporta para el
//! informe.
//!
//! [`Doctor`] valida items (id/name/path no vacíos, descripción ≥ 20 chars —
//! plan 06 §2: sin descripción el router no puede elegir) y produce un informe
//! legible: total por kind, inválidos y duplicados.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Tipo de componente auditado en el ecosistema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemKind {
    Skill,
    Agent,
    Plugin,
    Hook,
    McpServer,
    Harness,
}

impl ItemKind {
    /// Nombre legible del kind (para tablas del informe).
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemKind::Skill => "Skill",
            ItemKind::Agent => "Agent",
            ItemKind::Plugin => "Plugin",
            ItemKind::Hook => "Hook",
            ItemKind::McpServer => "McpServer",
            ItemKind::Harness => "Harness",
        }
    }
}

/// Un componente del inventario del ecosistema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditItem {
    pub id: String,
    pub name: String,
    pub kind: ItemKind,
    pub path: String,
    pub source: String,
    pub description: String,
}

impl AuditItem {
    /// Constructor canónico.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        kind: ItemKind,
        path: impl Into<String>,
        source: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            path: path.into(),
            source: source.into(),
            description: description.into(),
        }
    }
}

/// Registry dedup del inventario: indexa por `name` + `kind`.
#[derive(Debug, Default, Clone)]
pub struct AuditIndex {
    items: Vec<AuditItem>,
}

impl AuditIndex {
    /// Índice vacío.
    pub fn new() -> Self {
        Self::default()
    }

    /// Añade un item sin dedup. Devuelve `true` siempre.
    pub fn add(&mut self, item: AuditItem) -> bool {
        self.items.push(item);
        true
    }

    /// Añade con dedup por `name` + `kind`. Si ya existe un item con el mismo
    /// `name` y `kind` no añade nada y devuelve `false`; si es nuevo lo añade y
    /// devuelve `true`.
    pub fn add_dedup(&mut self, item: AuditItem) -> bool {
        if self.items.iter().any(|i| i.name == item.name && i.kind == item.kind) {
            return false;
        }
        self.items.push(item);
        true
    }

    /// Todos los items, en orden de inserción.
    pub fn all(&self) -> &[AuditItem] {
        &self.items
    }

    /// Items cuyo `kind` coincide (en orden de inserción).
    pub fn by_kind(&self, kind: ItemKind) -> Vec<&AuditItem> {
        self.items.iter().filter(|i| i.kind == kind).collect()
    }

    /// Items cuyo `name` coincide (en orden de inserción).
    pub fn by_name(&self, name: &str) -> Vec<&AuditItem> {
        self.items.iter().filter(|i| i.name == name).collect()
    }

    /// Nº total de items en el índice.
    pub fn count(&self) -> usize {
        self.items.len()
    }

    /// Items cuyo `name` + `kind` aparece más de una vez en el índice — los
    /// solapados que el informe de duplicados debe resolver. Devuelve TODAS las
    /// ocurrencias de cada grupo duplicado.
    pub fn duplicates(&self) -> Vec<&AuditItem> {
        let mut groups: HashMap<(&str, ItemKind), usize> = HashMap::new();
        for i in &self.items {
            *groups.entry((i.name.as_str(), i.kind)).or_insert(0) += 1;
        }
        self.items
            .iter()
            .filter(|i| groups.get(&(i.name.as_str(), i.kind)).copied().unwrap_or(0) > 1)
            .collect()
    }

    /// Índice de ejemplo construido sobre la auditoría real (plan 14 §5):
    /// `code-reviewer` aparece 3 veces (Skill, Agent, Plugin), `night-ops` 2,
    /// `fable` 2, `design-system` 1 y `doc-min` 1. `duplicates()` devuelve los
    /// solapados.
    pub fn from_example_ecosystem() -> Self {
        let mut index = Self::new();
        index.add(AuditItem::new(
            "skill-code-reviewer",
            "code-reviewer",
            ItemKind::Skill,
            "~/.claude/skills/code-review/SKILL.md",
            "global",
            "Skill de revisión de código: evalúa correctness, legibilidad, arquitectura y seguridad.",
        ));
        index.add(AuditItem::new(
            "agent-code-reviewer",
            "code-reviewer",
            ItemKind::Agent,
            "~/.claude/agents/code-reviewer.md",
            "global",
            "Agente que revisa diffs por bugs reales, fallos concretos y complejidad innecesaria.",
        ));
        index.add(AuditItem::new(
            "plugin-code-reviewer",
            "code-reviewer",
            ItemKind::Plugin,
            "plugins/ecc/agents/code-reviewer.md",
            "plugin-ecc",
            "Plugin ecc que aporta el agente de revisión de código para la fase Review del harness.",
        ));
        index.add(AuditItem::new(
            "skill-night-ops",
            "night-ops",
            ItemKind::Skill,
            "repo/skills/night-ops/SKILL.md",
            "repo",
            "Agente nocturno autónomo: trabaja por etapas con verificación real y commit atómico.",
        ));
        index.add(AuditItem::new(
            "skill-night-ops-global",
            "night-ops",
            ItemKind::Skill,
            "~/.claude/skills/night-ops/SKILL.md",
            "global",
            "Agente nocturno autónomo global: trabaja por etapas con verificación real y commit atómico.",
        ));
        index.add(AuditItem::new(
            "skill-fable",
            "fable",
            ItemKind::Skill,
            "repo/skills/fable/SKILL.md",
            "repo",
            "Fable-mode: disciplina de ejecución por etapas con plan escrito y verificación failable.",
        ));
        index.add(AuditItem::new(
            "skill-fable-global",
            "fable",
            ItemKind::Skill,
            "~/.claude/skills/fable-mode/SKILL.md",
            "global",
            "Fable-mode global: disciplina de ejecución por etapas con plan escrito y verificación failable.",
        ));
        index.add(AuditItem::new(
            "skill-design-system",
            "design-system",
            ItemKind::Skill,
            "~/.claude/skills/design-system/SKILL.md",
            "global",
            "Audita, documenta y extiende el design system: naming, tokens y variantes consistentes.",
        ));
        index.add(AuditItem::new(
            "agent-doc-min",
            "doc-min",
            ItemKind::Agent,
            "agents/doc-min.md",
            "repo",
            "Agente que mantiene documentación mínima: cabecera, decisiones y enlaces a planes.",
        ));
        index
    }
}

/// Doctor: validación e informe de salud del registry.
pub struct Doctor;

impl Doctor {
    /// Valida un item: `id` no vacío, `name` no vacío, `description` ≥ 20 chars
    /// y `path` no vacío. Devuelve `Err` con la primera regla que falla.
    pub fn validate(item: &AuditItem) -> Result<(), String> {
        if item.id.trim().is_empty() {
            return Err(format!("id vacío en item '{}'", item.name));
        }
        if item.name.trim().is_empty() {
            return Err("name vacío".to_string());
        }
        if item.description.trim().chars().count() < 20 {
            return Err(format!(
                "description de '{}' demasiado corta ({} chars, mínimo 20)",
                item.name,
                item.description.trim().chars().count()
            ));
        }
        if item.path.trim().is_empty() {
            return Err(format!("path vacío en item '{}'", item.name));
        }
        Ok(())
    }

    /// Informe legible del registry: total de items por kind, items inválidos
    /// (con el error de validación) y duplicados detectados.
    pub fn doctor_report(index: &AuditIndex) -> String {
        let mut out = String::new();
        out.push_str("== Informe Doctor del Registry ==\n");
        out.push_str(&format!("Total items: {}\n\n", index.count()));

        // Total por kind.
        out.push_str("== Items por kind ==\n");
        out.push_str(&format!("{:<10} {}\n", "Kind", "Count"));
        out.push_str(&format!("{:<10} {}\n", "-----", "-----"));
        for kind in [
            ItemKind::Skill,
            ItemKind::Agent,
            ItemKind::Plugin,
            ItemKind::Hook,
            ItemKind::McpServer,
            ItemKind::Harness,
        ] {
            out.push_str(&format!("{:<10} {}\n", kind.as_str(), index.by_kind(kind).len()));
        }

        // Items inválidos.
        out.push_str("\n== Items inválidos ==\n");
        let invalid: Vec<(&AuditItem, String)> = index
            .all()
            .iter()
            .filter_map(|i| match Self::validate(i) {
                Ok(()) => None,
                Err(e) => Some((i, e)),
            })
            .collect();
        if invalid.is_empty() {
            out.push_str("(ninguno)\n");
        } else {
            for (i, err) in &invalid {
                out.push_str(&format!("- [{}] {}: {}\n", i.kind.as_str(), i.name, err));
            }
        }

        // Duplicados.
        out.push_str("\n== Duplicados detectados ==\n");
        let dupes = index.duplicates();
        if dupes.is_empty() {
            out.push_str("(ninguno)\n");
        } else {
            let mut seen: Vec<(String, ItemKind)> = Vec::new();
            for i in &dupes {
                if seen.contains(&(i.name.clone(), i.kind)) {
                    continue;
                }
                seen.push((i.name.clone(), i.kind));
                let group: Vec<&AuditItem> = dupes
                    .iter()
                    .copied()
                    .filter(|d| d.name == i.name && d.kind == i.kind)
                    .collect();
                out.push_str(&format!(
                    "- {} [{}] x{} -> ",
                    i.name,
                    i.kind.as_str(),
                    group.len()
                ));
                out.push_str(
                    &group
                        .iter()
                        .map(|d| d.path.as_str())
                        .collect::<Vec<_>>()
                        .join(" | "),
                );
                out.push('\n');
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_item(name: &str, kind: ItemKind) -> AuditItem {
        AuditItem::new(
            format!("id-{}", name),
            name,
            kind,
            format!("path/{}.md", name),
            "repo",
            "Descripción de ejemplo suficientemente larga para pasar la validación del doctor.",
        )
    }

    #[test]
    fn add_simple_increments_count() {
        let mut index = AuditIndex::new();
        assert_eq!(index.count(), 0);
        index.add(sample_item("a", ItemKind::Skill));
        index.add(sample_item("b", ItemKind::Agent));
        assert_eq!(index.count(), 2);
    }

    #[test]
    fn add_dedup_rejects_same_name_and_kind() {
        let mut index = AuditIndex::new();
        assert!(index.add_dedup(sample_item("code-reviewer", ItemKind::Agent)));
        assert!(!index.add_dedup(sample_item("code-reviewer", ItemKind::Agent)));
        // Mismo name, distinto kind: SÍ se añade.
        assert!(index.add_dedup(sample_item("code-reviewer", ItemKind::Skill)));
        // Distinto name, mismo kind: SÍ se añade.
        assert!(index.add_dedup(sample_item("night-ops", ItemKind::Agent)));
        assert_eq!(index.count(), 3);
    }

    #[test]
    fn duplicates_detects_example_overlaps() {
        let index = AuditIndex::from_example_ecosystem();
        let dupes = index.duplicates();
        // Duplicados reales por name+kind: night-ops (skill x2) y fable (skill x2).
        assert_eq!(dupes.len(), 4);
        assert_eq!(index.by_name("code-reviewer").len(), 3); // 3 kinds distintos -> NO dup name+kind
        assert_eq!(index.by_name("night-ops").len(), 2);
        assert_eq!(index.by_name("fable").len(), 2);
        assert_eq!(index.by_name("design-system").len(), 1);
        assert!(dupes.iter().all(|i| i.name == "night-ops" || i.name == "fable"));
    }

    #[test]
    fn validate_rejects_short_description() {
        let item = AuditItem::new(
            "id",
            "short",
            ItemKind::Skill,
            "path/x.md",
            "repo",
            "corta",
        );
        let err = Doctor::validate(&item).unwrap_err();
        assert!(err.contains("corta"), "error: {err}");
    }

    #[test]
    fn validate_rejects_empty_id_and_path() {
        let item = AuditItem::new(
            "",
            "name",
            ItemKind::Skill,
            "path/x.md",
            "repo",
            "Descripción con la longitud mínima exigida por el doctor.",
        );
        assert!(Doctor::validate(&item).is_err());

        let item = AuditItem::new(
            "id",
            "name",
            ItemKind::Skill,
            "",
            "repo",
            "Descripción con la longitud mínima exigida por el doctor.",
        );
        assert!(Doctor::validate(&item).is_err());
    }

    #[test]
    fn validate_accepts_good_item() {
        let item = sample_item("ok", ItemKind::Hook);
        assert!(Doctor::validate(&item).is_ok());
    }

    #[test]
    fn doctor_report_mentions_duplicates() {
        let index = AuditIndex::from_example_ecosystem();
        let report = Doctor::doctor_report(&index);
        assert!(report.contains("Duplicados detectados"));
        assert!(report.contains("night-ops"));
        assert!(report.contains("fable"));
        assert!(report.contains("Total items: 9"));
        assert!(report.contains("Skill"));
        assert!(report.contains("Agent"));
        assert!(report.contains("(ninguno)")); // sin inválidos en el ejemplo
    }

    #[test]
    fn by_kind_filters() {
        let index = AuditIndex::from_example_ecosystem();
        let skills = index.by_kind(ItemKind::Skill);
        assert_eq!(skills.len(), 6); // code-reviewer + night-ops x2 + fable x2 + design-system
        assert!(skills.iter().all(|i| i.kind == ItemKind::Skill));
        assert_eq!(index.by_kind(ItemKind::Agent).len(), 2);
        assert_eq!(index.by_kind(ItemKind::Plugin).len(), 1);
        assert!(index.by_kind(ItemKind::McpServer).is_empty());
    }

    #[test]
    fn doctor_report_lists_invalid_items() {
        let mut index = AuditIndex::new();
        index.add(AuditItem::new(
            "id-bad",
            "bad-item",
            ItemKind::Hook,
            "path/bad.md",
            "repo",
            "desc corta",
        ));
        let report = Doctor::doctor_report(&index);
        assert!(report.contains("Items inválidos"));
        assert!(report.contains("bad-item"));
        assert!(report.contains("corta"));
    }
}
