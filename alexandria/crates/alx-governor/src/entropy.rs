//! Entropy — control de caos para la red de modelos.
//!
//! Problema que resuelve: statusline + hooks + agentes + Claude Code disparan
//! peticiones concurrentes sin coordinación; la cuenta de opencode-go satura,
//! el upstream devuelve 5xx en ráfaga y la sesión muere con "all models
//! failed" justo en el segundo mensaje. La cura son tres piezas:
//!
//! - [`Backoff`]: reintentos exponenciales JITTERIZADOS. Sin jitter, todos los
//!   procesos reintentan sincronizados y vuelven a saturar juntos; el ruido
//!   desincroniza la manada.
//! - [`CooldownState`]: enfriamiento COMPARTIDO entre procesos vía fichero de
//!   estado (`state/net-cooldown.json`): si un proceso detecta red caída, los
//!   demás no insisten durante la ventana.
//! - [`probe_url`]: sondeo GET barato (nada de generaciones de pago).
//!
//! Std-only: sin `rand` — la entropía sale de nanosegundos del reloj + pid
//! alimentando un xorshift64.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Fuente de entropía barata y determinista por instancia: xorshift64
/// sembrado con nanos + pid + contador de proceso. No es criptográfico ni
/// pretende serlo: solo necesita que dos instancias NO duerman lo mismo, ni
/// siquiera creadas en el mismo tick del reloj.
pub struct Jitter {
    state: u64,
}

impl Jitter {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E3779B97F4A7C15);
        let pid = std::process::id() as u64;
        let seed = nanos
            ^ seq.wrapping_mul(0xBF58476D1CE4E5B9)
            ^ pid.wrapping_mul(0x9E3779B97F4A7C15);
        Self { state: seed | 1 }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Duración base escalada por [1-frac, 1+frac].
    pub fn around(&mut self, base: Duration, frac: f64) -> Duration {
        // u64 → f64 en [0,1): 11 bits de mantisa bastan para esto.
        let u = (self.next_u64() >> 53) as f64 / 2048.0;
        let factor = 1.0 - frac + 2.0 * frac * u;
        Duration::from_secs_f64((base.as_secs_f64() * factor).max(0.0))
    }
}

impl Default for Jitter {
    fn default() -> Self {
        Self::new()
    }
}

/// Backoff exponencial jitterizado: intento n espera
/// `base * factor^n ± frac`. `max` techa cada espera.
///
/// ```no_run
/// use alx_governor::entropy::{Backoff, Jitter};
/// use std::time::Duration;
/// let mut b = Backoff::new(Duration::from_millis(500), 2.0, Duration::from_secs(30));
/// let mut rng = Jitter::new();
/// for intento in 0..3 {
///     let espera = b.wait_for(intento, &mut rng); // ~0.5s, ~1s, ~2s (con ruido)
///     // std::thread::sleep(espera);
/// }
/// ```
pub struct Backoff {
    base: Duration,
    factor: f64,
    max: Duration,
}

impl Backoff {
    pub fn new(base: Duration, factor: f64, max: Duration) -> Self {
        Self { base, factor, max }
    }

    /// Espera recomendada antes del intento `attempt` (0-indexed).
    pub fn wait_for(&self, attempt: u32, rng: &mut Jitter) -> Duration {
        let raw = self.base.as_secs_f64() * self.factor.powi(attempt as i32);
        let capped = Duration::from_secs_f64(raw).min(self.max);
        rng.around(capped, 0.35)
    }

    /// Espera total si se agotan todos los intentos (útil para presupuestar).
    pub fn total(&self, attempts: u32, rng: &mut Jitter) -> Duration {
        (0..attempts).map(|a| self.wait_for(a, rng)).sum()
    }
}

/// Ventana de enfriamiento compartida entre procesos.
///
/// El fichero vive en `<raiz>/state/net-cooldown.json`; se escribe con
/// tmp+rename (atómico) y se lee tolerando corrupción (un JSON roto = sin
/// cooldown, nunca un bloqueo permanente).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CooldownState {
    /// Epoch seconds hasta el que conviene no insistir.
    pub until_epoch_secs: u64,
    /// Quién lo puso (nombre corto del proceso).
    pub origin: String,
    /// Último error visto.
    pub reason: String,
}

impl CooldownState {
    pub fn path_under(root: &std::path::Path) -> PathBuf {
        root.join("state").join("net-cooldown.json")
    }

    pub fn load(path: &std::path::Path) -> Option<CooldownState> {
        let raw = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string(self).unwrap_or_default())?;
        std::fs::rename(&tmp, path)
    }

    /// ¿Sigue la ventana abierta?
    pub fn active(&self, now_epoch_secs: u64) -> bool {
        now_epoch_secs < self.until_epoch_secs
    }

    /// Abre/renueva la ventana.
    pub fn trip(path: &std::path::Path, secs: u64, origin: &str, reason: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let st = CooldownState {
            until_epoch_secs: now.saturating_add(secs),
            origin: origin.to_string(),
            reason: reason.chars().take(300).collect(),
        };
        let _ = st.save(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_se_mueve_alrededor_de_la_base() {
        let mut rng = Jitter::new();
        let base = Duration::from_millis(1000);
        for _ in 0..50 {
            let d = rng.around(base, 0.35);
            let ms = d.as_millis();
            assert!((650..=1350).contains(&ms), "jitter fuera de rango: {ms}ms");
        }
    }

    #[test]
    fn dos_jitters_difieren() {
        let a = Jitter::new().around(Duration::from_millis(100), 0.35);
        let b = Jitter::new().around(Duration::from_millis(100), 0.35);
        // probabilidad de colisión exacta ~ nula; no imposible, pero ok.
        assert_ne!(a, b);
    }

    #[test]
    fn backoff_crece_y_techa() {
        let mut rng = Jitter::new();
        let b = Backoff::new(Duration::from_millis(500), 2.0, Duration::from_secs(10));
        let w0 = b.wait_for(0, &mut rng).as_secs_f64();
        let w2 = b.wait_for(2, &mut rng).as_secs_f64();
        let w9 = b.wait_for(9, &mut rng).as_secs_f64();
        assert!(w0 < 1.0, "intento 0 cerca de 0.5s");
        assert!(w2 > w0 * 2.0, "crece exponencialmente");
        assert!(w9 <= 14.0, "techo respetado (10s + jitter 35%)");
    }

    #[test]
    fn cooldown_roundtrip_y_ventana() {
        let dir = std::env::temp_dir().join(format!("alx-entropy-test-{}", std::process::id()));
        let path = CooldownState::path_under(&dir);
        let _ = std::fs::remove_file(&path);

        assert!(CooldownState::load(&path).is_none(), "sin fichero no hay cooldown");

        CooldownState::trip(&path, 60, "alx-test", "upstream 500 x5");
        let st = CooldownState::load(&path).expect("cooldown persistido");
        assert_eq!(st.origin, "alx-test");
        assert!(st.active(st.until_epoch_secs - 1));
        assert!(!st.active(st.until_epoch_secs + 1));

        // JSON corrupto = sin cooldown (nunca bloqueo permanente)
        std::fs::write(&path, "{roto").unwrap();
        assert!(CooldownState::load(&path).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
