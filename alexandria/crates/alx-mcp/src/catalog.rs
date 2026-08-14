//! Catálogo central de tools MCP.

use serde::{Deserialize, Serialize};

/// Esquema de entrada por defecto: objeto sin propiedades.
pub fn default_input_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {}})
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

    /// Catálogo Fase 1: tools de ejemplo de los namespaces del plan 07 §2.
    pub fn alexandria_default() -> Self {
        let mut catalog = Self::new();
        for (name, description) in [
            ("task.list", "Lista las tareas del DAG de la fase actual."),
            ("task.create", "Crea una nueva tarea en el DAG."),
            ("harness.run", "Ejecuta el harness en una fase del pipeline."),
            ("agent.spawn", "Lanza un agente especialista."),
            ("memory.recall", "Recupera recuerdos relevantes de la memoria."),
            ("governor.cost_report", "Reporte de coste y presupuesto de la tarea."),
            ("gate.run", "Corre las compuertas de verificación."),
            ("bench.run", "Ejecuta un benchmark."),
            ("phalanx.status", "Estado del sistema phalanx."),
        ] {
            catalog.register(Tool::new(name, description));
        }
        catalog
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_catalog_has_nine_tools() {
        let catalog = ToolCatalog::alexandria_default();
        assert_eq!(catalog.list().len(), 9);
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
