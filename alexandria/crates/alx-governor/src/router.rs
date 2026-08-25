//! Routing por disponibilidad: tier → cadena de proxies (red corregida, plan 09 §1).
//!
//! Cadena canónica (v2): `headroom → routa-gateway → routatic`. Fallback
//! global: omniroute. El gateway (:3460) oculta el modelo real tras un nombre
//! visible `[1m]`, aplica suelo de max_tokens y gobierna la entropía de la red
//! (techo de concurrencia + backoff jitterizado + circuit-breaker).

use alx_core::types::ModelTier;
use serde::{Deserialize, Serialize};

/// Ruta materializada para un tier: lista ordenada de URLs de proxies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Route {
    pub tier: ModelTier,
    /// URLs de proxies en orden de uso (primero = entrada).
    pub chain: Vec<String>,
}

/// Conjunto de rutas + fallback único (omniroute).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Router {
    pub routes: Vec<Route>,
    /// Fallback global: omniroute (gateway multi-proveedor) — solo entra si
    /// routatic cae. NO es parte de la cadena principal.
    pub fallback: String,
}

pub const ROUTATIC: &str = "http://127.0.0.1:3456";
pub const HEADROOM: &str = "http://127.0.0.1:8788";
/// routa-gateway (:3460) — antes `cc-model-mask`; mismo puerto, más funciones.
pub const GATEWAY: &str = "http://127.0.0.1:3460";
pub const OMNIROUTE: &str = "http://127.0.0.1:20128";

impl Router {
    /// Las 3 rutas del plan:
    /// - T1: routatic directo (sin compresión)
    /// - T2: headroom → gateway → routatic
    /// - T3: igual que T2
    pub fn default_routes() -> Self {
        let medium_chain =
            vec![HEADROOM.to_string(), GATEWAY.to_string(), ROUTATIC.to_string()];
        Self {
            routes: vec![
                Route { tier: ModelTier::T1Cheap, chain: vec![ROUTATIC.to_string()] },
                Route { tier: ModelTier::T2Medium, chain: medium_chain.clone() },
                Route { tier: ModelTier::T3Premium, chain: medium_chain },
            ],
            fallback: OMNIROUTE.to_string(),
        }
    }

    /// Ruta para el tier, si existe.
    pub fn route_for(&self, tier: &ModelTier) -> Option<&Route> {
        self.routes.iter().find(|r| &r.tier == tier)
    }

    /// URL del fallback omniroute.
    pub fn fallback_url(&self) -> &str {
        &self.fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_routes_chains_exact() {
        let router = Router::default_routes();
        assert_eq!(router.routes.len(), 3);

        let t1 = router.route_for(&ModelTier::T1Cheap).expect("T1 route");
        assert_eq!(t1.chain, vec!["http://127.0.0.1:3456"]);

        let t2 = router.route_for(&ModelTier::T2Medium).expect("T2 route");
        assert_eq!(
            t2.chain,
            vec![
                "http://127.0.0.1:8788".to_string(),
                "http://127.0.0.1:3460".to_string(),
                "http://127.0.0.1:3456".to_string(),
            ]
        );

        let t3 = router.route_for(&ModelTier::T3Premium).expect("T3 route");
        assert_eq!(t3.chain, t2.chain); // T3 = igual que T2
        assert_eq!(t2.tier, ModelTier::T2Medium);
        assert_eq!(t3.tier, ModelTier::T3Premium);
    }

    #[test]
    fn fallback_is_omniroute() {
        let router = Router::default_routes();
        assert_eq!(router.fallback_url(), "http://127.0.0.1:20128");
    }

    #[test]
    fn route_for_missing_returns_none() {
        let router = Router {
            routes: vec![],
            fallback: OMNIROUTE.to_string(),
        };
        assert!(router.route_for(&ModelTier::T1Cheap).is_none());
    }
}
