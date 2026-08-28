//! Router del proxy: selección de proveedor/modelo/key por tarea.
//!
//! 1. `classify_prompt_text` del governor decide el tier (riesgo → premium,
//!    mecánico → cheap, tamaño → medium) o llega `X-Alx-Tier` del cliente.
//! 2. Candidatos: proveedores con `tier <= pedido`, ordenados por cercanía de
//!    tier (primero el más ajustado), peso y round-robin. Si ninguno llega,
//!    escalan todos (asc por tier).
//! 3. Cada candidato = (proveedor, modelo rotado, key rotada round-robin).
//! 4. Circuit-breaker por (proveedor, modelo): 3 fallos seguidos → abierto
//!    120 s. Éxito lo resetea.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::config::{resolve_key, Provider, ProxyConfig};

/// Un intento concreto upstream: proveedor + modelo + key resuelta.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub provider: String,
    pub protocol: String,
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub tier: u8,
    /// Timeout del proveedor para este intento (s).
    pub timeout_s: u64,
}

#[derive(Debug)]
struct Breaker {
    failures: u32,
    open_until: Option<Instant>,
}

const BREAKER_THRESHOLD: u32 = 3;
const BREAKER_COOLDOWN: Duration = Duration::from_secs(120);
const MAX_CANDIDATES: usize = 8;

pub struct RouteEngine {
    cfg: ProxyConfig,
    /// Índice de key rotatoria por proveedor.
    rr_key: Vec<AtomicUsize>,
    /// Índice de modelo rotatorio por proveedor.
    rr_model: Vec<AtomicUsize>,
    /// Desempate round-robin entre proveedores del mismo tier.
    rr_tie: AtomicUsize,
    breaker: Mutex<HashMap<String, Breaker>>,
}

impl RouteEngine {
    pub fn new(cfg: ProxyConfig) -> Self {
        let n = cfg.providers.len();
        Self {
            cfg,
            rr_key: (0..n).map(|_| AtomicUsize::new(0)).collect(),
            rr_model: (0..n).map(|_| AtomicUsize::new(0)).collect(),
            rr_tie: AtomicUsize::new(0),
            breaker: Mutex::new(HashMap::new()),
        }
    }

    pub fn config(&self) -> &ProxyConfig {
        &self.cfg
    }

    /// Tier pedido: `X-Alx-Tier: fast|cheap|medium|balanced|heavy|premium`
    /// (o 1|2|3). Si no viene, `None` y el router clasifica el prompt.
    pub fn tier_from_header(v: Option<&str>) -> Option<u8> {
        match v?.trim().to_ascii_lowercase().as_str() {
            "fast" | "cheap" | "light" | "1" => Some(1),
            "medium" | "balanced" | "2" => Some(2),
            "heavy" | "premium" | "3" => Some(3),
            _ => None,
        }
    }

    /// Lista ordenada de intentos para un tier pedido. El breaker filtra los
    /// circuitos abiertos; si TODO está abierto, igual devuelve candidatos
    /// (mejor reintentar que morir sin probar).
    pub fn candidates(&self, requested_tier: u8) -> Vec<Candidate> {
        let n = self.cfg.providers.len();
        if n == 0 {
            return vec![];
        }
        // (proveedor, |tier - pedido|) ordenado: primero el más ajustado.
        let mut order: Vec<usize> = (0..n).collect();
        let dumb = self.cfg.routing.dumb;
        let keys: Vec<(u8, u8, std::cmp::Reverse<u8>)> = order
            .iter()
            .map(|&i| {
                let p = &self.cfg.providers[i];
                if dumb {
                    (0, 0, std::cmp::Reverse(p.weight))
                } else if p.tier <= requested_tier {
                    (0, requested_tier - p.tier, std::cmp::Reverse(p.weight))
                } else {
                    (1, p.tier - requested_tier, std::cmp::Reverse(p.weight))
                }
            })
            .collect();
        order.sort_by_key(|&i| keys[i]);
        // round-robin de desempate: rota SOLO el grupo empatado del frente
        // (misma clave de orden). Rotar toda la lista degradaría el routing.
        let tie = self.rr_tie.fetch_add(1, Ordering::Relaxed);
        if tie > 0 && order.len() > 1 && keys[order[1]] == keys[order[0]] {
            let mut end = 1;
            while end < order.len() && keys[order[end]] == keys[order[0]] {
                end += 1;
            }
            order[..end].rotate_left(tie % end);
        }

        let mut out = Vec::new();
        for &i in &order {
            let p: &Provider = &self.cfg.providers[i];
            if p.models.is_empty() {
                continue;
            }
            let mi = self.rr_model[i].fetch_add(1, Ordering::Relaxed);
            // primer modelo el rotatorio; el resto en orden (failover intra)
            for k in 0..p.models.len() {
                let model = p.models[(mi + k) % p.models.len()].clone();
                let breaker_key = format!("{}/{}", p.name, model);
                if self.breaker_open(&breaker_key) {
                    continue;
                }
                let api_key = self.next_key(i, p);
                out.push(Candidate {
                    provider: p.name.clone(),
                    protocol: p.protocol.clone(),
                    base_url: p.base_url.clone(),
                    model,
                    api_key,
                    tier: p.tier,
                    timeout_s: p.timeout_s,
                });
                if out.len() >= MAX_CANDIDATES {
                    return out;
                }
                break; // por proveedor solo 1 modelo por pasada; el failover
                      // intra-proveedor se cubre en la siguiente pasada
            }
        }
        // segunda pasada intra-proveedor si aún hay hueco
        if out.len() < MAX_CANDIDATES {
            for &i in &order {
                let p: &Provider = &self.cfg.providers[i];
                if p.models.len() < 2 {
                    continue;
                }
                let mi = self.rr_model[i].load(Ordering::Relaxed);
                for k in 1..p.models.len() {
                    let model = p.models[(mi + k) % p.models.len()].clone();
                    let breaker_key = format!("{}/{}", p.name, model);
                    if self.breaker_open(&breaker_key) {
                        continue;
                    }
                    let api_key = self.next_key(i, p);
                    out.push(Candidate {
                        provider: p.name.clone(),
                        protocol: p.protocol.clone(),
                        base_url: p.base_url.clone(),
                        model,
                        api_key,
                        tier: p.tier,
                        timeout_s: p.timeout_s,
                    });
                    if out.len() >= MAX_CANDIDATES {
                        break;
                    }
                }
                if out.len() >= MAX_CANDIDATES {
                    break;
                }
            }
        }
        out
    }

    fn next_key(&self, i: usize, p: &Provider) -> Option<String> {
        if p.api_keys.is_empty() {
            return None;
        }
        if p.api_keys.len() == 1 {
            return resolve_key(&p.api_keys[0]);
        }
        let idx = self.rr_key[i].fetch_add(1, Ordering::Relaxed) % p.api_keys.len();
        resolve_key(&p.api_keys[idx])
    }

    pub fn record_failure(&self, provider: &str, model: &str) {
        let key = format!("{provider}/{model}");
        let mut map = self.breaker.lock().unwrap();
        let b = map.entry(key).or_insert(Breaker { failures: 0, open_until: None });
        b.failures += 1;
        if b.failures >= BREAKER_THRESHOLD {
            b.open_until = Some(Instant::now() + BREAKER_COOLDOWN);
        }
    }

    pub fn record_success(&self, provider: &str, model: &str) {
        let key = format!("{provider}/{model}");
        let mut map = self.breaker.lock().unwrap();
        map.insert(key, Breaker { failures: 0, open_until: None });
    }

    fn breaker_open(&self, key: &str) -> bool {
        let map = self.breaker.lock().unwrap();
        match map.get(key) {
            Some(b) => matches!(b.open_until, Some(t) if Instant::now() < t),
            None => false,
        }
    }

    /// Resumen de circuitos para /proxy/status.
    pub fn breaker_summary(&self) -> Vec<(String, u32, bool)> {
        let map = self.breaker.lock().unwrap();
        let mut v: Vec<(String, u32, bool)> = map
            .iter()
            .map(|(k, b)| {
                (
                    k.clone(),
                    b.failures,
                    matches!(b.open_until, Some(t) if Instant::now() < t),
                )
            })
            .collect();
        v.sort();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProxyConfig;

    fn cfg(providers: Vec<crate::config::Provider>) -> ProxyConfig {
        ProxyConfig { providers, ..ProxyConfig::defaults() }
    }

    fn prov(name: &str, protocol: &str, tier: u8, weight: u8, keys: Vec<&str>, models: Vec<&str>) -> crate::config::Provider {
        crate::config::Provider {
            name: name.into(),
            protocol: protocol.into(),
            base_url: format!("http://{name}"),
            api_keys: keys.into_iter().map(String::from).collect(),
            models: models.into_iter().map(String::from).collect(),
            tier,
            weight,
            timeout_s: 60,
        }
    }

    #[test]
    fn tier_pedido_elige_el_mas_ajustado_primero() {
        let e = RouteEngine::new(cfg(vec![
            prov("premium", "anthropic", 3, 5, vec![], vec!["p1"]),
            prov("cheap", "anthropic", 0, 5, vec![], vec!["c1"]),
            prov("mid", "anthropic", 1, 5, vec![], vec!["m1"]),
        ]));
        let c = e.candidates(1);
        assert_eq!(c[0].provider, "mid"); // tier 1 pedido → tier 1 exacto
        let c3 = e.candidates(3);
        assert_eq!(c3[0].provider, "premium"); // heavy → premium primero
    }

    #[test]
    fn keys_roten_round_robin() {
        let e = RouteEngine::new(cfg(vec![prov(
            "pool",
            "openai",
            0,
            5,
            vec!["k1", "k2", "k3"],
            vec!["m1"],
        )]));
        let mut got = Vec::new();
        for _ in 0..6 {
            got.push(e.candidates(2)[0].api_key.clone().unwrap());
        }
        // 6 pedidos sobre 3 keys → cada key sale 2 veces, sin repetición seguida
        for k in ["k1", "k2", "k3"] {
            assert_eq!(got.iter().filter(|g| *g == k).count(), 2, "key {k}");
        }
    }

    #[test]
    fn breaker_abre_tras_3_fallos_y_cuela_el_siguiente() {
        let e = RouteEngine::new(cfg(vec![
            prov("roto", "anthropic", 0, 10, vec![], vec!["r1"]),
            prov("sano", "anthropic", 0, 5, vec![], vec!["s1"]),
        ]));
        for _ in 0..3 {
            e.record_failure("roto", "r1");
        }
        let c = e.candidates(2);
        assert_eq!(c[0].provider, "sano"); // el roto desaparece de la lista
        e.record_success("roto", "r1");
        let c2 = e.candidates(2);
        assert_eq!(c2[0].provider, "roto"); // éxito resetea (weight mayor vuelve a ganar)
    }

    #[test]
    fn header_de_tier_se_parsea() {
        assert_eq!(RouteEngine::tier_from_header(Some("heavy")), Some(3));
        assert_eq!(RouteEngine::tier_from_header(Some("fast")), Some(1));
        assert_eq!(RouteEngine::tier_from_header(Some("bob")), None);
    }
}
