//! alx-agents — Registry, router, spawn, headless sessions.
//!
//! Carga agentes desde markdown con frontmatter YAML simple (parse manual, sin crate
//! yaml — Fase 1), los valida, los rutea a fases del pipeline y ensambla el envelope
//! mínimo que necesita el agente (spec 06-agents-system).
//!
//! Piezas:
//! - [`AgentSpec`]: schema serde de un agente, con [`AgentSpec::from_frontmatter`].
//! - [`AgentRegistry`]: almacena, consulta por nombre/fase y valida los specs.
//! - [`route`] / [`fallback`]: router de fase → agentes, con general-purpose de respaldo.
//! - [`build_envelope`]: ensambla el [`AgentEnvelope`] (system + task + memory + tools + budget).

use alx_core::types::{ModelTier, PhaseId, Recall};
use serde::{Deserialize, Serialize};

/// Schema de un agente. Serializa/deserializa a JSON (índice `state/agents.index.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSpec {
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,
    pub tier: ModelTier,
    pub phase: Option<PhaseId>,
    pub tags: Vec<String>,
}

impl Default for AgentSpec {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            tools: Vec::new(),
            tier: ModelTier::T2Medium,
            phase: None,
            tags: Vec::new(),
        }
    }
}

impl AgentSpec {
    /// Parsea el frontmatter YAML simple entre líneas `---`.
    ///
    /// Soporta `clave: valor` y listas en dos formas: `clave: [a, b]` o un bloque
    /// de líneas `- item` tras `clave:`. Valores con comillas (`"..."` / `'...'`)
    /// se des-citan. Campos desconocidos se ignoran (p.ej. `skip_if`).
    ///
    /// Error si falta `name` o `description`, si no hay frontmatter o si `tier`/
    /// `phase` no son valores conocidos.
    pub fn from_frontmatter(md: &str) -> Result<AgentSpec, String> {
        let lines: Vec<&str> = md.lines().collect();
        if lines.first().map(|l| l.trim()) != Some("---") {
            return Err("no frontmatter: el archivo debe empezar por '---'".to_string());
        }
        let end = lines
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, l)| l.trim() == "---")
            .map(|(i, _)| i)
            .ok_or_else(|| "frontmatter sin cierre '---'".to_string())?;
        let body = &lines[1..end];

        let mut spec = AgentSpec::default();
        // Acumulador para listas multilinea ("tools:" seguido de "- item").
        let mut list_key: Option<String> = None;
        let mut pending: Vec<String> = Vec::new();

        for raw in body {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("- ") {
                let item = strip_quotes(rest.trim());
                match &list_key {
                    Some(_) => pending.push(item),
                    None => return Err(format!("línea '- item' sin lista previa: {line}")),
                }
                continue;
            }
            // Línea "clave: valor": primero cierra la lista pendiente.
            if let Some(k) = list_key.take() {
                assign_list(&mut spec, &k, std::mem::take(&mut pending));
            }
            let Some((key, val)) = line.split_once(':') else {
                return Err(format!("línea sin 'clave: valor': {line}"));
            };
            let (key, val) = (key.trim(), val.trim());
            match key {
                "name" => spec.name = strip_quotes(val),
                "description" => spec.description = strip_quotes(val),
                "tier" => spec.tier = parse_tier(val)?,
                "phase" => spec.phase = Some(parse_phase(val)?),
                "tools" | "tags" => {
                    if val.is_empty() {
                        // Lista multilinea por "- item".
                        list_key = Some(key.to_string());
                    } else if val.starts_with('[') && val.ends_with(']') {
                        assign_list(&mut spec, key, split_csv(&val[1..val.len() - 1]));
                    } else {
                        assign_list(&mut spec, key, vec![strip_quotes(val)]);
                    }
                }
                _ => {} // Campos desconocidos se ignoran.
            }
        }
        // Lista pendiente al final del bloque.
        if let Some(k) = list_key.take() {
            assign_list(&mut spec, &k, std::mem::take(&mut pending));
        }

        if spec.name.is_empty() {
            return Err("falta el campo obligatorio: name".to_string());
        }
        if spec.description.is_empty() {
            return Err("falta el campo obligatorio: description".to_string());
        }
        Ok(spec)
    }
}

/// Des-cita un valor si empieza y termina con el mismo tipo de comilla.
fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    let bytes = s.as_bytes();
    if bytes.len() >= 2 {
        let (first, last) = (bytes[0], bytes[bytes.len() - 1]);
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return s[1..s.len() - 1].trim().to_string();
        }
    }
    s.to_string()
}

/// Divide una lista `[a, b, c]` por comas, recorta y des-cita cada item.
fn split_csv(inner: &str) -> Vec<String> {
    inner
        .split(',')
        .map(|s| strip_quotes(s.trim()))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Vuelca una lista parseada en el campo del spec que corresponda.
fn assign_list(spec: &mut AgentSpec, key: &str, vals: Vec<String>) {
    match key {
        "tools" => spec.tools = vals,
        "tags" => spec.tags = vals,
        _ => {}
    }
}

/// `"T1Cheap" | "T2Medium" | "T3Premium"` → [`ModelTier`].
fn parse_tier(s: &str) -> Result<ModelTier, String> {
    match s.trim() {
        "T1Cheap" => Ok(ModelTier::T1Cheap),
        "T2Medium" => Ok(ModelTier::T2Medium),
        "T3Premium" => Ok(ModelTier::T3Premium),
        other => Err(format!("tier desconocido: {other}")),
    }
}

/// String de fase (`PhaseId::as_str`) → [`PhaseId`].
fn parse_phase(s: &str) -> Result<PhaseId, String> {
    match s.trim() {
        "Ingest" => Ok(PhaseId::Ingest),
        "Spec" => Ok(PhaseId::Spec),
        "Plan" => Ok(PhaseId::Plan),
        "Build" => Ok(PhaseId::Build),
        "Test" => Ok(PhaseId::Test),
        "Review" => Ok(PhaseId::Review),
        "Docs" => Ok(PhaseId::Docs),
        "Ship" => Ok(PhaseId::Ship),
        other => Err(format!("fase desconocida: {other}")),
    }
}

/// Presupuesto de tokens por tier (T1=2000, T2=15000, T3=60000).
pub fn budget_for_tier(tier: ModelTier) -> u32 {
    match tier {
        ModelTier::T1Cheap => 2_000,
        ModelTier::T2Medium => 15_000,
        ModelTier::T3Premium => 60_000,
    }
}

/// Registry de agentes: almacena specs en orden de inserción y los consulta.
#[derive(Debug, Clone, Default)]
pub struct AgentRegistry {
    agents: Vec<AgentSpec>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Añade un spec al final del registro.
    pub fn add(&mut self, spec: AgentSpec) {
        self.agents.push(spec);
    }

    /// Todos los specs, en orden de inserción.
    pub fn all(&self) -> &[AgentSpec] {
        &self.agents
    }

    /// Primer agente con el nombre exacto dado.
    pub fn by_name(&self, name: &str) -> Option<&AgentSpec> {
        // Búsqueda tolerante: exacta, y si no, normalizada (case/slug).
        // El frontmatter dice "Agents Orchestrator" y el usuario escribe
        // "agents-orchestrator": ambos deben funcionar.
        if let Some(a) = self.agents.iter().find(|a| a.name == name) {
            return Some(a);
        }
        let wanted = slugify_name(name);
        self.agents
            .iter()
            .find(|a| slugify_name(&a.name) == wanted)
    }

    /// Agentes cuya fase declarada coincide, en orden estable de inserción.
    pub fn by_phase(&self, phase: PhaseId) -> Vec<&AgentSpec> {
        self.agents
            .iter()
            .filter(|a| a.phase == Some(phase))
            .collect()
    }

    /// Valida el registro y devuelve los errores encontrados:
    /// descripción < 20 chars, tools vacías y nombre duplicado.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for spec in &self.agents {
            let len = spec.description.chars().count();
            if len < 20 {
                errors.push(format!(
                    "agent '{}': description too short ({} chars)",
                    spec.name, len
                ));
            }
            if spec.tools.is_empty() {
                errors.push(format!("agent '{}': no tools", spec.name));
            }
        }
        let mut counts: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
        for spec in &self.agents {
            *counts.entry(spec.name.as_str()).or_insert(0) += 1;
        }
        let mut dupes: Vec<&str> = counts
            .iter()
            .filter(|(_, &c)| c > 1)
            .map(|(n, _)| *n)
            .collect();
        dupes.sort_unstable();
        for name in dupes {
            errors.push(format!("duplicate agent name: {name}"));
        }
        errors
    }

    /// Lee cada archivo, y si su contenido empieza por `---` lo parsea y registra.
    /// Los archivos sin frontmatter se ignoran (no son agentes). Un archivo ilegible
    /// o con frontmatter inválido produce `Err` para esa entrada.
    pub fn register_from_markdowns(&mut self, files: &[&str]) -> Vec<Result<(), String>> {
        files
            .iter()
            .map(|path| {
                let content = std::fs::read_to_string(path)
                    .map_err(|e| format!("cannot read {path}: {e}"))?;
                if !content.starts_with("---") {
                    return Ok(());
                }
                let spec =
                    AgentSpec::from_frontmatter(&content).map_err(|e| format!("{path}: {e}"))?;
                self.add(spec);
                Ok(())
            })
            .collect()
    }
}

/// Agentes candidatos para una fase: los que declaran `phase == Some(phase)`,
/// en orden estable de inserción.
pub fn route(phase: PhaseId, registry: &AgentRegistry) -> Vec<&AgentSpec> {
    registry.by_phase(phase)
}

/// Agente de respaldo universal: `general-purpose` si está registrado.
pub fn fallback(registry: &AgentRegistry) -> Option<&AgentSpec> {
    registry.by_name("general-purpose")
}

/// Envelope mínimo que recibe un agente al hacer spawn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEnvelope {
    /// Prompt de sistema: descripción del agente + reglas globales.
    pub system: String,
    /// La tarea concreta de la fase.
    pub task: String,
    /// Recalls inyectados, recortados al presupuesto de chars.
    pub memory: Vec<Recall>,
    /// Tools permitidas al agente.
    pub tools: Vec<String>,
    /// Presupuesto de tokens según el tier.
    pub budget_tokens: u32,
}

/// Ensambla el envelope mínimo para un agente.
///
/// `system` = descripción del spec + reglas (caveman, evidencia). `memory` conserva
/// el orden de `recalls` pero solo los que quepan (por chars de `text`) en
/// `max_memory_chars`; un recall que desborde el presupuesto se descarta entero.
/// `budget_tokens` depende del tier (T1=2000, T2=15000, T3=60000).
pub fn build_envelope(
    spec: &AgentSpec,
    task: &str,
    recalls: Vec<Recall>,
    max_memory_chars: usize,
) -> AgentEnvelope {
    let system = format!(
        "{}\n\nREGLAS:\n- Estilo caveman: técnico, sin relleno.\n- Toda afirmación con evidencia: comandos reales ejecutados, no 'debería funcionar'.",
        spec.description
    );
    let mut used = 0usize;
    let mut memory = Vec::with_capacity(recalls.len());
    for recall in recalls {
        let cost = recall.text.chars().count();
        if used + cost > max_memory_chars {
            continue;
        }
        used += cost;
        memory.push(recall);
    }
    AgentEnvelope {
        system,
        task: task.to_string(),
        memory,
        tools: spec.tools.clone(),
        budget_tokens: budget_for_tier(spec.tier),
    }
}

/// Normaliza nombres de agente para búsquedas: minúsculas, no alfanuméricos
/// → '-'. "Agents Orchestrator" == "agents-orchestrator" == "AgentsOrchestrator".
pub fn slugify_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alx_core::types::RecallSource;

    fn recall(id: &str, text: &str) -> Recall {
        Recall {
            id: id.to_string(),
            text: text.to_string(),
            source: RecallSource::Session,
            tags: Vec::new(),
            weight: 1,
            created: 0,
        }
    }

    const FULL_FRONTMATTER: &str = r#"---
name: incremental-implementation
description: "Implementa cambios en pasos pequeños y verificables con evidencia"
tools: [Read, Write, "Bash", Test]
tier: T2Medium
phase: Build
tags: [quality, incremental]
---
# Cuerpo del agente
"#;

    #[test]
    fn from_frontmatter_parses_full() {
        let spec = AgentSpec::from_frontmatter(FULL_FRONTMATTER).unwrap();
        assert_eq!(spec.name, "incremental-implementation");
        assert_eq!(spec.description, "Implementa cambios en pasos pequeños y verificables con evidencia");
        assert_eq!(spec.tools, vec!["Read", "Write", "Bash", "Test"]);
        assert_eq!(spec.tier, ModelTier::T2Medium);
        assert_eq!(spec.phase, Some(PhaseId::Build));
        assert_eq!(spec.tags, vec!["quality", "incremental"]);
    }

    #[test]
    fn from_frontmatter_multiline_lists() {
        let md = r#"---
name: multi
description: "Agente con listas multilinea"
tools:
  - Read
  - Grep
  - Bash
tags:
  - a
  - b
tier: T1Cheap
phase: Test
---
"#;
        let spec = AgentSpec::from_frontmatter(md).unwrap();
        assert_eq!(spec.tools, vec!["Read", "Grep", "Bash"]);
        assert_eq!(spec.tags, vec!["a", "b"]);
        assert_eq!(spec.tier, ModelTier::T1Cheap);
        assert_eq!(spec.phase, Some(PhaseId::Test));
    }

    #[test]
    fn from_frontmatter_single_value_list_and_defaults() {
        let md = r#"---
name: minimo
description: "Descripcion suficientemente larga para pasar la validacion"
tools: Read
---
"#;
        let spec = AgentSpec::from_frontmatter(md).unwrap();
        assert_eq!(spec.tools, vec!["Read"]);
        assert_eq!(spec.tier, ModelTier::T2Medium); // default
        assert_eq!(spec.phase, None); // default
        assert!(spec.tags.is_empty()); // default
    }

    #[test]
    fn from_frontmatter_missing_name_errors() {
        let md = r#"---
description: "Descripcion sin nombre"
---
"#;
        let err = AgentSpec::from_frontmatter(md).unwrap_err();
        assert!(err.contains("name"), "error debería mencionar name: {err}");
    }

    #[test]
    fn from_frontmatter_missing_description_errors() {
        let md = r#"---
name: solo-nombre
---
"#;
        let err = AgentSpec::from_frontmatter(md).unwrap_err();
        assert!(err.contains("description"), "error debería mencionar description: {err}");
    }

    #[test]
    fn from_frontmatter_bad_tier_errors() {
        let md = r#"---
name: malo
description: "Descripcion con tier invalido que supera el minimo de longitud"
tier: T9Ultra
---
"#;
        assert!(AgentSpec::from_frontmatter(md).is_err());
    }

    #[test]
    fn from_frontmatter_without_frontmatter_errors() {
        assert!(AgentSpec::from_frontmatter("# solo cuerpo\nsin frontmatter").is_err());
    }

    #[test]
    fn registry_validate_detects_short_description() {
        let mut registry = AgentRegistry::new();
        registry.add(AgentSpec {
            name: "corto".to_string(),
            description: "muy corta".to_string(),
            ..Default::default()
        });
        let errors = registry.validate();
        assert!(
            errors.iter().any(|e| e.contains("description too short")),
            "errores: {errors:?}"
        );
    }

    #[test]
    fn registry_validate_detects_no_tools_and_duplicates() {
        let mut registry = AgentRegistry::new();
        let spec = |name: &str| AgentSpec {
            name: name.to_string(),
            description: "Descripcion larga de sobra para pasar el minimo de veinte caracteres"
                .to_string(),
            tools: vec!["Read".to_string()],
            ..Default::default()
        };
        // Sin tools.
        registry.add(AgentSpec {
            name: "sin-tools".to_string(),
            description: "Descripcion larga de sobra para pasar el minimo de veinte caracteres"
                .to_string(),
            ..Default::default()
        });
        // Duplicado.
        registry.add(spec("dup"));
        registry.add(spec("dup"));
        registry.add(spec("ok"));

        let errors = registry.validate();
        assert!(errors.iter().any(|e| e.contains("no tools")));
        assert!(errors.iter().any(|e| e.contains("duplicate agent name: dup")));
        // 'ok' está bien formado.
        assert!(errors.iter().all(|e| !e.contains("'ok'")));
    }

    #[test]
    fn register_from_markdowns_ignores_without_and_adds_with_frontmatter() {
        let plain = temp_md(
            "plain",
            "# Nota de documentacion\n\nEsto no es un agente, no empieza por '---'.",
        );
        let agent = temp_md(
            "agent",
            r#"---
name: from-file
description: "Agente cargado desde un archivo temporal con frontmatter"
tools: [Read, Bash]
tier: T3Premium
phase: Review
---
"#,
        );
        let paths = [plain.to_str().unwrap().to_string(), agent.to_str().unwrap().to_string()];
        let file_refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();

        let mut registry = AgentRegistry::new();
        let results = registry.register_from_markdowns(&file_refs);

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.is_ok()), "resultados: {results:?}");
        // El archivo sin '---' no registró; el de frontmatter sí.
        assert_eq!(registry.all().len(), 1);
        let added = registry.by_name("from-file").expect("agente añadido");
        assert_eq!(added.tier, ModelTier::T3Premium);
        assert_eq!(added.phase, Some(PhaseId::Review));

        // by_name tolerante: slug y mayúsculas encuentran al mismo agente.
        assert!(registry.by_name("from-file").is_some());
        assert!(registry.by_name("From File").is_some());
        assert!(registry.by_name("FROM_FILE").is_some());

        std::fs::remove_file(&plain).ok();
        std::fs::remove_file(&agent).ok();
    }

    #[test]
    fn register_from_markdowns_reports_unreadable() {
        let mut registry = AgentRegistry::new();
        let results = registry.register_from_markdowns(&["/no/existe/este/archivo.md"]);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_err());
    }

    #[test]
    fn route_returns_only_phase_matches_in_order() {
        let mut registry = AgentRegistry::new();
        let spec = |name: &str, phase: Option<PhaseId>| AgentSpec {
            name: name.to_string(),
            description: "Descripcion larga de sobra para pasar el minimo de veinte caracteres"
                .to_string(),
            tools: vec!["Read".to_string()],
            phase,
            ..Default::default()
        };
        registry.add(spec("b1", Some(PhaseId::Build)));
        registry.add(spec("reviewer", Some(PhaseId::Review)));
        registry.add(spec("b2", Some(PhaseId::Build)));
        registry.add(spec("sin-fase", None));

        let build = route(PhaseId::Build, &registry);
        let names: Vec<&str> = build.iter().map(|a| a.name.as_str()).collect();
        // Orden estable de inserción, solo la fase Build.
        assert_eq!(names, vec!["b1", "b2"]);

        assert!(route(PhaseId::Ship, &registry).is_empty());
    }

    #[test]
    fn fallback_finds_general_purpose() {
        let mut registry = AgentRegistry::new();
        assert!(fallback(&registry).is_none());
        registry.add(AgentSpec {
            name: "general-purpose".to_string(),
            description: "Agente general de proposito para tareas que no calzan en un especialista"
                .to_string(),
            tools: vec!["*".to_string()],
            ..Default::default()
        });
        assert_eq!(fallback(&registry).unwrap().name, "general-purpose");
    }

    #[test]
    fn build_envelope_budget_by_tier_and_memory_limit() {
        let mk_spec = |tier: ModelTier| AgentSpec {
            name: "x".to_string(),
            description: "Descripcion larga de sobra para pasar el minimo de veinte caracteres"
                .to_string(),
            tools: vec!["Read".to_string(), "Bash".to_string()],
            tier,
            ..Default::default()
        };
        assert_eq!(build_envelope(&mk_spec(ModelTier::T1Cheap), "", vec![], 0).budget_tokens, 2_000);
        assert_eq!(build_envelope(&mk_spec(ModelTier::T2Medium), "", vec![], 0).budget_tokens, 15_000);
        assert_eq!(build_envelope(&mk_spec(ModelTier::T3Premium), "", vec![], 0).budget_tokens, 60_000);

        // Memory: "tok" (3) + "auth" (4) = 7 cabe; "cache-inval" (11) no.
        let recalls = vec![recall("a", "tok"), recall("b", "auth"), recall("c", "cache-inval")];
        let envelope = build_envelope(&mk_spec(ModelTier::T1Cheap), "haz algo", recalls, 7);
        assert_eq!(envelope.memory.len(), 2);
        let total: usize = envelope.memory.iter().map(|r| r.text.chars().count()).sum();
        assert!(total <= 7);
        // Orden preservado.
        assert_eq!(envelope.memory[0].id, "a");
        assert_eq!(envelope.memory[1].id, "b");
        // Tools clonadas del spec.
        assert_eq!(envelope.tools, vec!["Read", "Bash"]);
        // System embebe la descripción.
        assert!(envelope.system.contains("Descripcion larga"));
        assert!(envelope.system.contains("caveman"));
    }

    #[test]
    fn build_envelope_skips_single_recall_over_budget() {
        let spec = AgentSpec {
            name: "x".to_string(),
            description: "Descripcion larga de sobra para pasar el minimo de veinte caracteres"
                .to_string(),
            ..Default::default()
        };
        let recalls = vec![recall("big", "texto-muy-largo-que-no-cabe-en-el-presupuesto")];
        let envelope = build_envelope(&spec, "t", recalls, 5);
        // El recall entero se descarta, no se trunca.
        assert!(envelope.memory.is_empty());
    }

    #[test]
    fn serde_roundtrip_envelope() {
        let spec = AgentSpec {
            name: "x".to_string(),
            description: "Descripcion larga de sobra para pasar el minimo de veinte caracteres"
                .to_string(),
            tier: ModelTier::T3Premium,
            tools: vec!["Bash".to_string()],
            ..Default::default()
        };
        let env = build_envelope(&spec, "task", vec![recall("r", "mem")], 100);
        let json = serde_json::to_string(&env).unwrap();
        let back: AgentEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(back.budget_tokens, 60_000);
        assert_eq!(back.memory.len(), 1);
    }

    /// Crea un archivo markdown temporal con nombre único por proceso.
    fn temp_md(suffix: &str, content: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "alx-agents-test-{}-{}.md",
            std::process::id(),
            suffix
        ));
        std::fs::write(&path, content).expect("escribir archivo temporal");
        path
    }
}
