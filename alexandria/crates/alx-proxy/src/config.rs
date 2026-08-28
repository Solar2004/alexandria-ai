//! Config del proxy: TOML con proveedores declarativos.
//!
//! Orden de resolución: `$ALX_PROXY_CONFIG` → `~/.config/alexandria/proxy.toml`
//! → `<repo>/alexandria/config/proxy.toml` → defaults (routatic local, sin keys).
//!
//! Las api-keys soportan la forma `env:VAR` para no meter secretos en disco.

use serde::Deserialize;

/// Proveedor upstream declarado en el TOML (array-of-tables: el orden del
/// fichero se preserva y es el orden de preferencia a igualdad de tier).
#[derive(Debug, Clone, Deserialize)]
pub struct Provider {
    pub name: String,
    /// Protocolo de conversación que habla el upstream: "anthropic" | "openai".
    #[serde(default = "default_protocol")]
    pub protocol: String,
    pub base_url: String,
    /// Pool de keys; `env:VAR` se resuelve al arrancar. Vacío = sin auth.
    #[serde(default)]
    pub api_keys: Vec<String>,
    /// Modelos que rota este proveedor (failover intra-proveedor).
    pub models: Vec<String>,
    /// Nivel de potencia: 0 = barato/siempre, 3 = premium.
    #[serde(default)]
    pub tier: u8,
    /// Desempate entre proveedores del mismo tier (mayor gana).
    #[serde(default = "default_weight")]
    pub weight: u8,
    /// Timeout por request (s).
    #[serde(default = "default_timeout")]
    pub timeout_s: u64,
}

fn default_protocol() -> String {
    "anthropic".into()
}
fn default_weight() -> u8 {
    5
}
fn default_timeout() -> u64 {
    300
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProxySettings {
    #[serde(default = "default_port")]
    pub port: u16,
    /// Modelo que ve el cliente (máscara).
    #[serde(default = "default_visible")]
    pub visible_model: String,
    /// Semáforo global de concurrencia hacia upstream (entropía).
    #[serde(default = "default_conc")]
    pub max_concurrency: usize,
    /// Cuánto espera un request en la cola del semáforo (s).
    #[serde(default = "default_queue")]
    pub queue_timeout_s: u64,
}

fn default_port() -> u16 {
    8797
}
fn default_visible() -> String {
    "claude-opus-4-6[1m]".into()
}
fn default_conc() -> usize {
    6
}
fn default_queue() -> u64 {
    120
}
impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            port: default_port(),
            visible_model: default_visible(),
            max_concurrency: default_conc(),
            queue_timeout_s: default_queue(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RoutingSettings {
    /// Desactiva la clasificación por tarea (todo va en orden declarado).
    #[serde(default)]
    pub dumb: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProxyConfig {
    #[serde(default)]
    pub proxy: ProxySettings,
    #[serde(default)]
    pub routing: RoutingSettings,
    #[serde(default)]
    pub providers: Vec<Provider>,
}

/// Resuelve `env:VAR` a su valor; las keys sin prefijo van tal cual.
pub fn resolve_key(raw: &str) -> Option<String> {
    if let Some(var) = raw.strip_prefix("env:") {
        std::env::var(var).ok().filter(|v| !v.is_empty())
    } else if raw.is_empty() {
        None
    } else {
        Some(raw.to_string())
    }
}

impl ProxyConfig {
    /// Carga la config del primer sitio que exista; si ninguno, defaults.
    pub fn load() -> (Self, String) {
        if let Ok(p) = std::env::var("ALX_PROXY_CONFIG") {
            if let Ok(t) = std::fs::read_to_string(&p) {
                return (Self::parse(&t), p);
            }
        }
        let home = std::env::var("HOME").unwrap_or_default();
        let user_cfg = std::path::PathBuf::from(&home).join(".config/alexandria/proxy.toml");
        if let Ok(t) = std::fs::read_to_string(&user_cfg) {
            return (Self::parse(&t), user_cfg.display().to_string());
        }
        let repo_cfg = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../config/proxy.toml");
        if let Ok(t) = std::fs::read_to_string(&repo_cfg) {
            return (Self::parse(&t), repo_cfg.display().to_string());
        }
        (Self::defaults(), "(defaults embebidos)".into())
    }

    pub fn parse(toml_text: &str) -> Self {
        toml::from_str(toml_text).unwrap_or_else(|e| {
            eprintln!("alx-proxy: config inválida ({e}); uso defaults");
            Self::defaults()
        })
    }

    /// Defaults: routatic/opencode-go local como único proveedor (tier 0).
    /// Funciona sin ninguna key — la cadena que ya existe sigue sirviendo.
    pub fn defaults() -> Self {
        Self {
            proxy: ProxySettings::default(),
            routing: RoutingSettings::default(),
            providers: vec![Provider {
                name: "routatic".into(),
                protocol: "anthropic".into(),
                base_url: "http://127.0.0.1:3456".into(),
                api_keys: vec![],
                models: vec![
                    "deepseek-v4-flash".into(),
                    "hy3".into(),
                    "kimi-k2.7-code".into(),
                    "glm-5".into(),
                ],
                tier: 0,
                weight: 10,
                timeout_s: 300,
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsea_toml_completo_y_preserva_orden() {
        let t = r#"
[proxy]
port = 9000
visible_model = "x[1m]"

[[providers]]
name = "a"
base_url = "http://a"
models = ["m1"]
tier = 1
weight = 2

[[providers]]
name = "b"
protocol = "openai"
base_url = "http://b"
api_keys = ["env:NO_EXISTE_XYZ", "sk-raw"]
models = ["m2", "m3"]
tier = 0
"#;
        let cfg = ProxyConfig::parse(t);
        assert_eq!(cfg.proxy.port, 9000);
        assert_eq!(cfg.providers.len(), 2);
        assert_eq!(cfg.providers[0].name, "a"); // orden preservado
        assert_eq!(cfg.providers[1].protocol, "openai");
        // env: inexistente → None; raw → tal cual
        assert_eq!(resolve_key("env:NO_EXISTE_XYZ"), None);
        assert_eq!(resolve_key("sk-raw"), Some("sk-raw".into()));
        assert_eq!(resolve_key(""), None);
    }

    #[test]
    fn defaults_sin_fichero_sirven_routatic() {
        let cfg = ProxyConfig::defaults();
        assert_eq!(cfg.providers.len(), 1);
        assert_eq!(cfg.providers[0].name, "routatic");
        assert_eq!(cfg.proxy.port, 8797);
    }
}
