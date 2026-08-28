//! Catálogo central de tools MCP.

use serde::{Deserialize, Serialize};

/// Esquema de entrada por defecto: objeto sin propiedades.
pub fn default_input_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {}})
}

/// Construye un `input_schema` objeto con propiedades string obligatorias.
fn schema_with(required: &[&str]) -> serde_json::Value {
    let mut props = serde_json::Map::new();
    for name in required {
        props.insert(
            (*name).to_string(),
            serde_json::json!({"type": "string", "description": name}),
        );
    }
    serde_json::json!({"type": "object", "properties": props, "required": required})
}

/// Una tool MCP expuesta por el servidor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl Tool {
    /// Construye una tool con el `input_schema` por defecto.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema: default_input_schema(),
        }
    }
}

/// Catálogo central de tools: todas las del motor más las de los clientes MCP.
#[derive(Debug, Clone, Default)]
pub struct ToolCatalog {
    tools: Vec<Tool>,
}

impl ToolCatalog {
    /// Catálogo vacío.
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// Registra una tool al final del catálogo.
    pub fn register(&mut self, tool: Tool) {
        self.tools.push(tool);
    }

    /// Vista de todas las tools registradas.
    pub fn list(&self) -> Vec<&Tool> {
        self.tools.iter().collect()
    }

    /// Busca una tool por nombre.
    pub fn by_name(&self, name: &str) -> Option<&Tool> {
        self.tools.iter().find(|t| t.name == name)
    }

    /// ¿Existe una tool con este nombre?
    pub fn has(&self, name: &str) -> bool {
        self.by_name(name).is_some()
    }

    /// Catálogo de tools del motor — todas con ejecución REAL en alx-cli.
    pub fn alexandria_default() -> Self {
        let mut catalog = Self::new();
        let mut with_schema = |name: &str, description: &str, schema: serde_json::Value| {
            let mut t = Tool::new(name, description);
            t.input_schema = schema;
            catalog.register(t);
        };
        with_schema("task.list", "Lista las tareas persistidas (state/tasks.jsonl).", default_input_schema());
        with_schema(
            "task.create",
            "Crea y persiste una tarea nueva (state/tasks.jsonl).",
            schema_with(&["title"]),
        );
        with_schema(
            "memory.recall",
            "Recupera los recuerdos más relevantes de la memoria (por peso).",
            serde_json::json!({
                "type": "object",
                "properties": {"n": {"type": "integer", "description": "cuántos recalls (default 8)"}}
            }),
        );
        with_schema(
            "memory.save",
            "Guarda una lección/aprendizaje en la memoria (comprime caveman y persiste).",
            schema_with(&["text"]),
        );
        with_schema(
            "harness.run",
            "Ciclo del watcher evolutivo: recarga harnesses del disco, promueve/retira.",
            default_input_schema(),
        );
        with_schema(
            "agent.spawn",
            "Lanza un agente especialista real contra la cadena.",
            schema_with(&["name", "task"]),
        );
        with_schema(
            "governor.cost_report",
            "Reporte de coste real desde el ledger persistido.",
            default_input_schema(),
        );
        with_schema(
            "gate.run",
            "Compuerta de verificación real: corre un comando y devuelve evidencia.",
            serde_json::json!({
                "type": "object",
                "properties": {"command": {"type": "string", "description": "comando a verificar (default: build del motor)"}}
            }),
        );
        with_schema(
            "bench.run",
            "Resumen de benchmarks ejecutados + métricas por crate.",
            default_input_schema(),
        );
        with_schema("phalanx.status", "Estado del sistema phalanx (config + hooks).", default_input_schema());
        with_schema(
            "skill.harness",
            "Crea/activa el harness temporal de una skill con sus pasos (se reinyectan cada prompt).",
            schema_with(&["skill"]),
        );
        with_schema(
            "harness.step",
            "Marca un paso (1-indexed) del harness de skill como hecho.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "id del harness (hx-skill-...)"},
                    "step": {"type": "integer", "description": "número de paso"}
                },
                "required": ["id", "step"]
            }),
        );
        with_schema(
            "lsp.check",
            "Diagnostics LSP REALES (rust-analyzer/tsserver/pyright) sobre ficheros.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "files": {"type": "array", "items": {"type": "string"}, "description": "ficheros a verificar"}
                },
                "required": ["files"]
            }),
        );
        catalog
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_catalog_has_thirteen_tools() {
        let catalog = ToolCatalog::alexandria_default();
        assert_eq!(catalog.list().len(), 13);
    }

    #[test]
    fn parameterized_tools_declare_required_args() {
        let catalog = ToolCatalog::alexandria_default();
        let create = catalog.by_name("task.create").expect("task.create registrada");
        assert_eq!(create.input_schema["required"], serde_json::json!(["title"]));
        let save = catalog.by_name("memory.save").expect("memory.save registrada");
        assert_eq!(save.input_schema["required"], serde_json::json!(["text"]));
    }

    #[test]
    fn by_name_finds_tool() {
        let catalog = ToolCatalog::alexandria_default();
        let tool = catalog.by_name("task.list").expect("task.list registrada");
        assert_eq!(tool.name, "task.list");
        assert!(!tool.description.is_empty());
        assert_eq!(
            tool.input_schema,
            serde_json::json!({"type": "object", "properties": {}})
        );
    }

    #[test]
    fn by_name_missing_returns_none() {
        let catalog = ToolCatalog::new();
        assert!(catalog.by_name("nope").is_none());
    }

    #[test]
    fn has_reports_presence() {
        let catalog = ToolCatalog::alexandria_default();
        assert!(catalog.has("gate.run"));
        assert!(!catalog.has("gate.nope"));
    }

    #[test]
    fn register_adds_tool() {
        let mut catalog = ToolCatalog::new();
        catalog.register(Tool::new("foo.bar", "desc"));
        assert_eq!(catalog.list().len(), 1);
        assert!(catalog.has("foo.bar"));
    }
}
