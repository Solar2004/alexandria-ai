//! Binario `alx-proxy`: EL proxy de Alexandria.
//!
//! Config: `$ALX_PROXY_CONFIG` → `~/.config/alexandria/proxy.toml` →
//! `alexandria/config/proxy.toml` → defaults (routatic local).

#[tokio::main]
async fn main() {
    let (cfg, src) = alx_proxy::ProxyConfig::load();
    eprintln!(
        "alx-proxy: config {} — {} proveedor(es), modelo visible '{}'",
        src,
        cfg.providers.len(),
        cfg.proxy.visible_model
    );
    alx_proxy::server::serve(cfg).await;
}
