//! Cliente MCP — registro de servidores externos (stub de discovery, Fase 1).

use serde::{Deserialize, Serialize};

/// Configuración de un servidor MCP externo que alx consume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
}

impl ClientConfig {
    /// Nuevo cliente con comando placeholder (aún sin lanzador real en Fase 1).
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: "unset".to_string(),
            args: Vec::new(),
            enabled: true,
        }
    }
}

/// Registro de clientes MCP del motor.
#[derive(Debug, Clone, Default)]
pub struct ClientRegistry {
    pub clients: Vec<ClientConfig>,
}

impl ClientRegistry {
    /// Registro vacío.
    pub fn new() -> Self {
        Self { clients: Vec::new() }
    }

    /// Los 5 MCP del plan 07 §3 que alx descubre en Fase 1.
    pub fn alexandria_default() -> Self {
        let clients = [
            "codebase-memory",
            "code-graph-rag",
            "notebooklm",
            "mcp-search",
            "chrome-devtools",
        ]
        .iter()
        .map(|name| ClientConfig::new(*name))
        .collect();
        Self { clients }
    }

    /// Clientes habilitados.
    pub fn enabled_clients(&self) -> Vec<&ClientConfig> {
        self.clients.iter().filter(|c| c.enabled).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_has_five_enabled_clients() {
        let registry = ClientRegistry::alexandria_default();
        assert_eq!(registry.clients.len(), 5);
        assert_eq!(registry.enabled_clients().len(), 5);
        let names: Vec<&str> = registry
            .enabled_clients()
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(
            names,
            vec![
                "codebase-memory",
                "code-graph-rag",
                "notebooklm",
                "mcp-search",
                "chrome-devtools"
            ]
        );
    }

    #[test]
    fn enabled_clients_filters_disabled() {
        let mut registry = ClientRegistry::alexandria_default();
        registry.clients[0].enabled = false;
        assert_eq!(registry.enabled_clients().len(), 4);
    }

    #[test]
    fn empty_registry_has_no_enabled() {
        let registry = ClientRegistry::new();
        assert!(registry.enabled_clients().is_empty());
    }
}
