//! alx-tui — dashboard vivo y UNIFICADO de ALEXANDRIA (ratatui).
//!
//! `alx tui` abre este dashboard. Regla de oro: todo sondeo es GET local o
//! lectura de fichero de estado — NUNCA un POST de generación para pintar
//! una pantalla (lección de la saturación).
//!
//! Pestañas:
//!   [1] Panel    — resumen ejecutivo: red, contadores, última actividad
//!   [2] Red      — salud de los 5 servicios con latencia
//!   [3] Proxy    — alx-proxy: proveedores, circuitos y ledger de intentos
//!   [4] Agentes  — sesiones vivas (activity.jsonl) + mailbox A2A
//!   [5] Harnesses— registry evolutivo global + proyecto + skill-harness
//!   [6] Tareas   — DAG de tareas con presupuesto
//!   [7] Recalls  — memoria comprimida (top por peso)
//!
//! Teclas: 1-7/Tab pestaña · q/Esc salir · r refrescar ahora.
//! Auto-refresh: HTTP cada 3 s, ficheros cada 1 s (ambos baratos).

pub mod theme;

use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEvent,
    MouseEventKind,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, Gauge, Paragraph, Row, Table, Tabs, Wrap,
};
use ratatui::Terminal;

use crate::theme::{
    border_primary, border_secondary, info_value, state_color, table_header, tab_active, title_style,
    PROGRESS,
};

// ─────────────────────────────────────────── rutas

/// Raíz del repo: cwd con state/ → cwd; cwd/alexandria → ese; si no, la ruta
/// de compilación (máquina dev).
fn repo_root() -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.join("state").is_dir() && cwd.join("harnesses").is_dir() {
            return cwd;
        }
        if cwd.join("alexandria/state").is_dir() {
            return cwd.join("alexandria");
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn state_dir() -> PathBuf {
    repo_root().join("state")
}

fn proxy_ledger_path() -> PathBuf {
    if let Ok(p) = std::env::var("ALX_PROXY_LEDGER") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(format!("{home}/.local/state/alexandria/proxy-ledger.jsonl"))
}

// ─────────────────────────────────────────── HTTP mínimo (GET barato)

/// GET HTTP mínimo sobre TCP crudo — sin dependencias ni proxies raros.
/// Devuelve (código HTTP, latencia ms). None = caído/timeout.
fn http_get_simple(url: &str, timeout_ms: u64) -> Option<(u16, u128)> {
    let t0 = Instant::now();
    let rest = url.strip_prefix("http://")?;
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let mut stream = TcpStream::connect(hostport).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(timeout_ms)))
        .ok()?;
    stream
        .set_write_timeout(Some(Duration::from_millis(timeout_ms)))
        .ok()?;
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {hostport}\r\nUser-Agent: alx-tui\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).ok()?;
    let mut buf = [0u8; 2048];
    let mut head = Vec::new();
    while head.len() < 8192 {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                head.extend_from_slice(&buf[..n]);
                if head.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let text = String::from_utf8_lossy(&head);
    let code: u16 = text
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    Some((code, t0.elapsed().as_millis()))
}

fn http_get_body(url: &str, timeout_ms: u64) -> Option<String> {
    let rest = url.strip_prefix("http://")?;
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let mut stream = TcpStream::connect(hostport).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(timeout_ms)))
        .ok()?;
    stream
        .set_write_timeout(Some(Duration::from_millis(timeout_ms)))
        .ok()?;
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {hostport}\r\nUser-Agent: alx-tui\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).ok()?;
    let mut buf = vec![0u8; 8192];
    let mut leido = Vec::new();
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                leido.extend_from_slice(&buf[..n]);
                if leido.len() > 262_144 {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let sep = leido.windows(4).position(|w| w == b"\r\n\r\n")?;
    Some(String::from_utf8_lossy(&leido[sep + 4..]).into_owned())
}

// ─────────────────────────────────────────── pollers: red

pub struct NetRow {
    pub name: String,
    pub url: String,
    pub code: String,
    pub ok: bool,
    pub ms: u128,
}

fn poll_net() -> Vec<NetRow> {
    let endpoints = [
        ("alx-proxy", "http://127.0.0.1:8797/health"),
        ("routatic", "http://127.0.0.1:3456/v1/models"),
        ("gateway", "http://127.0.0.1:3460/health"),
        ("headroom", "http://127.0.0.1:8788/readyz"),
        ("omniroute", "http://127.0.0.1:20128/"),
    ];
    endpoints
        .iter()
        .map(|(name, url)| match http_get_simple(url, 1200) {
            Some((code, ms)) => NetRow {
                name: name.to_string(),
                url: url.to_string(),
                code: code.to_string(),
                ok: (200..500).contains(&code),
                ms,
            },
            None => NetRow {
                name: name.to_string(),
                url: url.to_string(),
                code: "000".to_string(),
                ok: false,
                ms: 0,
            },
        })
        .collect()
}

#[derive(Default)]
struct GovernorStats {
    in_flight: u32,
    served: u32,
    retries: u32,
    failovers: u32,
    last_served_model: String,
    last_error: String,
}

fn poll_governor() -> GovernorStats {
    let Some(b) = http_get_body("http://127.0.0.1:3460/stats", 1200) else {
        return GovernorStats::default();
    };
    let v: serde_json::Value = serde_json::from_str(&b).unwrap_or_default();
    let g = &v["governor"];
    GovernorStats {
        in_flight: g["in_flight"].as_u64().unwrap_or(0) as u32,
        served: g["served"].as_u64().unwrap_or(0) as u32,
        retries: g["retries"].as_u64().unwrap_or(0) as u32,
        failovers: g["failovers"].as_u64().unwrap_or(0) as u32,
        last_served_model: g["last_served_model"].as_str().unwrap_or("").to_string(),
        last_error: g["last_error"].as_str().unwrap_or("").chars().take(80).collect(),
    }
}

// ─────────────────────────────────────────── pollers: proxy

#[derive(Default)]
struct ProxyView {
    visible_model: String,
    providers: Vec<(String, String, u8, usize, usize)>, // nombre, protocolo, tier, keys, modelos
    breakers: Vec<(String, u32, bool)>,                 // circuito, fallos, abierto
    ledger: Vec<(String, String, bool, u64)>,           // provider/model, model, ok, ms (más nuevo primero)
    ok: bool,
}

fn poll_proxy() -> ProxyView {
    let mut out = ProxyView::default();
    if let Some(b) = http_get_body("http://127.0.0.1:8797/proxy/status", 1200) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&b) {
            out.ok = true;
            out.visible_model = v["visible_model"].as_str().unwrap_or("?").to_string();
            for p in v["providers"].as_array().unwrap_or(&vec![]) {
                out.providers.push((
                    p["name"].as_str().unwrap_or("?").into(),
                    p["protocol"].as_str().unwrap_or("?").into(),
                    p["tier"].as_u64().unwrap_or(0) as u8,
                    p["keys"].as_u64().unwrap_or(0) as usize,
                    p["models"].as_array().map(|m| m.len()).unwrap_or(0),
                ));
            }
            for b in v["breakers"].as_array().unwrap_or(&vec![]) {
                out.breakers.push((
                    b["circuit"].as_str().unwrap_or("?").into(),
                    b["failures"].as_u64().unwrap_or(0) as u32,
                    b["open"].as_bool().unwrap_or(false),
                ));
            }
        }
    }
    // ledger: tail 14, más nuevo primero
    if let Ok(txt) = std::fs::read_to_string(proxy_ledger_path()) {
        for line in txt.lines().rev().take(14) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                out.ledger.push((
                    v["provider"].as_str().unwrap_or("?").into(),
                    v["model"].as_str().unwrap_or("?").into(),
                    v["ok"].as_bool().unwrap_or(false),
                    v["ms"].as_u64().unwrap_or(0),
                ));
            }
        }
    }
    out
}

// ─────────────────────────────────────────── pollers: agentes/sesiones

#[derive(Default)]
struct SessionRow {
    id: String,
    events: u32,
    last_ev: String,
    last_tool: String,
    cwd: String,
    ago: String,
}

/// Sesiones vivas desde state/activity.jsonl (una línea por evento con
/// session+cwd): agrupa por sesión y muestra la más reciente primero.
fn poll_sessions() -> Vec<SessionRow> {
    let txt = std::fs::read_to_string(state_dir().join("activity.jsonl")).unwrap_or_default();
    let mut map: BTreeMap<String, SessionRow> = BTreeMap::new();
    for line in txt.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let id = v["session"].as_str().unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let ts = v["ts"].as_u64().unwrap_or(0);
        let e = map.entry(id.clone()).or_insert_with(|| SessionRow {
            id: id.chars().take(12).collect(),
            ..SessionRow::default()
        });
        e.events += 1;
        e.last_ev = v["ev"].as_str().unwrap_or("?").into();
        e.last_tool = v["tool"].as_str().unwrap_or("").into();
        e.cwd = v["cwd"].as_str().unwrap_or("").chars().take(38).collect();
        e.ago = hace(ts);
    }
    let mut v: Vec<SessionRow> = map.into_values().collect();
    v.sort_by(|a, b| b.ago.cmp(&a.ago)); // los "justo ahora" arriba
    v
}

/// Convierte ts ms unix a "hace Xm" (o la fecha si el reloj no cuadra).
fn hace(ts_ms: u64) -> String {
    if ts_ms == 0 {
        return "?".into();
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let d = now.saturating_sub(ts_ms) / 1000;
    if now < ts_ms {
        return "ahora".into();
    }
    match d {
        0..=59 => format!("hace {d}s"),
        60..=3599 => format!("hace {}m", d / 60),
        3600..=86399 => format!("hace {}h", d / 3600),
        _ => format!("hace {}d", d / 86400),
    }
}

/// Mailbox A2A: fichero por sesión destino con sus mensajes pendientes.
fn poll_mailbox() -> Vec<(String, usize)> {
    let dir = state_dir().join("mailbox");
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            if e.path().extension().map(|x| x == "jsonl").unwrap_or(false) {
                let n = std::fs::read_to_string(e.path())
                    .map(|t| t.lines().count())
                    .unwrap_or(0);
                out.push((e.file_name().to_string_lossy().to_string(), n));
            }
        }
    }
    out.sort();
    out
}

// ─────────────────────────────────────────── pollers: harnesses

struct HarnessRow {
    id: String,
    kind: String,
    state: String,
    uses: u32,
    scope: String,
    steps: Option<(usize, usize)>, // done, total (skill-harness)
}

fn registry_rows(dir: &std::path::Path, scope: &str) -> Vec<HarnessRow> {
    let reg = alx_evolve::HarnessRegistry::load_from(dir);
    reg.all()
        .iter()
        .map(|h| {
            let steps = if h.id.starts_with("hx-skill-") {
                let path = dir.join("active").join(format!("{}.steps.json", h.id));
                std::fs::read_to_string(path)
                    .ok()
                    .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                    .and_then(|v| {
                        let arr = v["steps"].as_array()?;
                        let total = arr.len();
                        let done = arr.iter().filter(|s| s["done"].as_bool() == Some(true)).count();
                        Some((done, total))
                    })
            } else {
                None
            };
            HarnessRow {
                id: h.id.clone(),
                kind: format!("{:?}", h.kind).to_lowercase(),
                state: format!("{:?}", h.state).to_lowercase(),
                uses: h.uses,
                scope: scope.into(),
                steps,
            }
        })
        .collect()
}

fn poll_harnesses() -> Vec<HarnessRow> {
    let root = repo_root();
    let mut out = registry_rows(&root.join("harnesses"), "global");
    out.extend(registry_rows(&root.join(".alexandria/harnesses"), "proyecto"));
    out
}

// ─────────────────────────────────────────── pollers: tareas y recalls

struct TaskRow {
    id: String,
    title: String,
    status: String,
    phase: String,
    spent: u64,
    total: u64,
}

fn poll_tasks() -> Vec<TaskRow> {
    let txt = std::fs::read_to_string(state_dir().join("tasks.jsonl")).unwrap_or_default();
    txt.lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .map(|v| TaskRow {
            id: v["id"].as_str().unwrap_or("?").into(),
            title: v["title"].as_str().unwrap_or("?").chars().take(44).collect(),
            status: v["status"].as_str().unwrap_or("?").into(),
            phase: v["phase"].as_str().unwrap_or("?").into(),
            spent: v["budget"]["spent"].as_u64().unwrap_or(0),
            total: v["budget"]["total"].as_u64().unwrap_or(0),
        })
        .collect()
}

fn poll_recalls() -> Vec<(u64, String, String)> {
    // (peso, source, texto) top 16 por peso
    let txt = std::fs::read_to_string(state_dir().join("recalls.json")).unwrap_or_default();
    let v: serde_json::Value = serde_json::from_str(&txt).unwrap_or_default();
    let mut out: Vec<(u64, String, String)> = v["recalls"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|r| {
            (
                r["weight"].as_u64().unwrap_or(0),
                r["source"].as_str().unwrap_or("?").into(),
                r["text"].as_str().unwrap_or("").chars().take(70).collect(),
            )
        })
        .collect();
    out.sort_by_key(|r| std::cmp::Reverse(r.0));
    out.truncate(16);
    out
}

fn load_iterate() -> (u32, u32) {
    let path = repo_root().join("harnesses/iterate/state.toml");
    let txt = std::fs::read_to_string(path).unwrap_or_default();
    let get = |k: &str| {
        txt.lines()
            .find(|l| l.starts_with(k))
            .and_then(|l| l.split('=').nth(1))
            .and_then(|v| v.trim().parse::<u32>().ok())
            .unwrap_or(0)
    };
    (get("iter"), get("max_iter"))
}

fn load_model_real() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let txt = std::fs::read_to_string(format!("{home}/.config/routatic-proxy/config.json"))
        .unwrap_or_default();
    let v: serde_json::Value = serde_json::from_str(&txt).unwrap_or_default();
    v["models"]["default"]["model_id"].as_str().unwrap_or("?").to_string()
}

fn last_activity() -> String {
    let txt = std::fs::read_to_string(state_dir().join("activity.jsonl")).unwrap_or_default();
    txt.lines()
        .next_back()
        .and_then(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .map(|v| {
            format!(
                "{} {} {} · {} · {}",
                hace(v["ts"].as_u64().unwrap_or(0)),
                v["ev"].as_str().unwrap_or("?"),
                v["tool"].as_str().unwrap_or(""),
                v["session"].as_str().unwrap_or(""),
                v["cwd"].as_str().unwrap_or(""),
            )
        })
        .unwrap_or_else(|| "sin actividad registrada".into())
}

// ─────────────────────────────────────────── estilos
// Estado y acentos vienen de theme.rs; ok_style se reexporta localmente para
// que las pestañas existentes no cambien de firma.
use theme::ok_style;

const TABS: [&str; 7] = ["1 Panel", "2 Red", "3 Proxy", "4 Agentes", "5 Harnesses", "6 Tareas", "7 Recalls"];

// ─────────────────────────────────────────── ui state

/// Estado de interacción compartido entre pestañas.
#[derive(Default)]
struct UiState {
    /// Fila seleccionada por pestaña (índice sobre la tabla visible).
    selected: [usize; 7],
    /// Offset de scroll por pestaña (para párrafos largos).
    scroll: [u16; 7],
    /// Ayuda contextual visible (tecla ?).
    show_help: bool,
    /// Auto-refresh en pausa (tecla espacio).
    paused: bool,
    /// Mensaje flash en la status bar (con instante de expiración).
    flash: Option<(String, Instant)>,
}

impl UiState {
    fn sel(&self, tab: usize) -> usize {
        self.selected[tab]
    }
    fn set_sel(&mut self, tab: usize, len: usize, new: usize) {
        if len == 0 {
            self.selected[tab] = 0;
            return;
        }
        self.selected[tab] = new.min(len - 1);
    }
    fn move_sel(&mut self, tab: usize, len: usize, delta: isize) {
        if len == 0 {
            return;
        }
        let cur = self.selected[tab] as isize;
        let next = (cur + delta).clamp(0, len as isize - 1);
        self.selected[tab] = next as usize;
    }
    fn flash_msg(&mut self, msg: impl Into<String>) {
        self.flash = Some((msg.into(), Instant::now()));
    }
}

/// Número de filas seleccionables de la pestaña actual.
fn row_count(tab: usize, net: &[NetRow], sessions: &[SessionRow], harnesses: &[HarnessRow], tasks: &[TaskRow], recalls: &[(u64, String, String)], proxy: &ProxyView) -> usize {
    match tab {
        1 => net.len(),
        2 => proxy.ledger.len(),
        3 => sessions.len(),
        4 => harnesses.len(),
        5 => tasks.len(),
        6 => recalls.len(),
        _ => 0,
    }
}

/// Texto de ayuda contextual por pestaña.
fn help_text(tab: usize) -> &'static str {
    match tab {
        0 => "Panel: resumen ejecutivo de la cadena governor.\n\n  r        refrescar ahora\n  espacio  pausar/reanudar auto-refresh\n  ?        esta ayuda\n  1-7/Tab  cambiar de pestaña\n  q/Esc    salir",
        1 => "Red: salud de los 5 servicios locales (GET barato, sin coste).\n\n  ↑↓/j/k   navegar servicios\n  Enter    ping manual del servicio seleccionado\n  r        refrescar todos\n  espacio  pausar auto-refresh\n  ?        esta ayuda",
        2 => "Proxy: proveedores, circuitos y ledger de alx-proxy.\n\n  ↑↓/j/k   navegar ledger\n  espacio  pausar auto-refresh\n  ?        esta ayuda",
        3 => "Agentes: sesiones vivas + mailbox A2A.\n\n  ↑↓/j/k   navegar sesiones\n  m        ver mensajes de la sesión seleccionada\n  ?        esta ayuda",
        4 => "Harnesses: registry evolutivo (global + proyecto).\n\n  ↑↓/j/k   navegar harnesses\n  Enter    detalle del harness seleccionado\n  ?        esta ayuda",
        5 => "Tareas: DAG con presupuesto.\n\n  ↑↓/j/k   navegar tareas\n  Enter    detalle de la tarea seleccionada\n  ?        esta ayuda",
        _ => "Recalls: memoria comprimida, top por peso.\n\n  ↑↓/j/k   navegar recalls\n  PgUp/PgDn scroll rápido\n  ?        esta ayuda",
    }
}

// ─────────────────────────────────────────── app

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut tab: usize = 0;
    let mut last_http = Instant::now() - Duration::from_secs(60);
    let mut last_files = Instant::now() - Duration::from_secs(60);
    let mut ui = UiState::default();

    let mut net: Vec<NetRow> = Vec::new();
    let mut gov = GovernorStats::default();
    let mut proxy = ProxyView::default();
    let mut sessions: Vec<SessionRow> = Vec::new();
    let mut mailbox: Vec<(String, usize)> = Vec::new();
    let mut harnesses: Vec<HarnessRow> = Vec::new();
    let mut tasks: Vec<TaskRow> = Vec::new();
    let mut recalls: Vec<(u64, String, String)> = Vec::new();
    let mut tick: u64 = 0;

    loop {
        if event::poll(Duration::from_millis(150))? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Esc if !ui.show_help => return Ok(()),
                    KeyCode::Char('?') => ui.show_help = !ui.show_help,
                    KeyCode::Esc => ui.show_help = false,
                    KeyCode::Char(' ') => {
                        ui.paused = !ui.paused;
                        ui.flash_msg(if ui.paused { "auto-refresh EN PAUSA (espacio para reanudar)" } else { "auto-refresh reanudado" });
                    }
                    KeyCode::Char('r') => {
                        last_http -= Duration::from_secs(60);
                        last_files -= Duration::from_secs(60);
                        ui.flash_msg("refrescando…");
                    }
                    KeyCode::Tab => tab = (tab + 1) % TABS.len(),
                    KeyCode::BackTab => tab = (tab + TABS.len() - 1) % TABS.len(),
                    KeyCode::Char(c @ '1'..='7') => {
                        tab = c as usize - '1' as usize;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let len = row_count(tab, &net, &sessions, &harnesses, &tasks, &recalls, &proxy);
                        ui.move_sel(tab, len, 1);
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        let len = row_count(tab, &net, &sessions, &harnesses, &tasks, &recalls, &proxy);
                        ui.move_sel(tab, len, -1);
                    }
                    KeyCode::Home | KeyCode::Char('g') => {
                        let len = row_count(tab, &net, &sessions, &harnesses, &tasks, &recalls, &proxy);
                        ui.set_sel(tab, len, 0);
                    }
                    KeyCode::End | KeyCode::Char('G') => {
                        let len = row_count(tab, &net, &sessions, &harnesses, &tasks, &recalls, &proxy);
                        ui.set_sel(tab, len, len.saturating_sub(1));
                    }
                    KeyCode::PageDown => {
                        let len = row_count(tab, &net, &sessions, &harnesses, &tasks, &recalls, &proxy);
                        ui.move_sel(tab, len, 10);
                    }
                    KeyCode::PageUp => {
                        let len = row_count(tab, &net, &sessions, &harnesses, &tasks, &recalls, &proxy);
                        ui.move_sel(tab, len, -10);
                    }
                    KeyCode::Enter if tab == 1 => {
                        // ping manual del servicio seleccionado
                        if let Some(row) = net.get(ui.sel(1)) {
                            let url = row.url.clone();
                            let res = http_get_simple(&url, 1500);
                            match res {
                                Some((code, ms)) => ui.flash_msg(format!("ping {}: HTTP {} ({}ms)", row.name, code, ms)),
                                None => ui.flash_msg(format!("ping {}: SIN RESPUESTA", row.name)),
                            }
                        }
                    }
                    KeyCode::Enter if tab == 4 => {
                        if let Some(h) = harnesses.get(ui.sel(4)) {
                            ui.flash_msg(format!("harness {} · {} · usos {}", h.id, h.state, h.uses));
                        }
                    }
                    KeyCode::Enter if tab == 5 => {
                        if let Some(t) = tasks.get(ui.sel(5)) {
                            let pct = if t.total > 0 { (t.spent * 100).checked_div(t.total).unwrap_or(0).min(100) } else { 0 };
                            ui.flash_msg(format!("tarea {} · {} · presupuesto {}/{} ({}%)", t.id, t.status, t.spent, t.total, pct));
                        }
                    }
                    _ => {}
                },
                Event::Mouse(MouseEvent { kind, column, row: mrow, .. }) => {
                    let area = terminal.get_frame().area();
                    match kind {
                        MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                            // click en la barra de pestañas (root[1] = fila 1..3)
                            if (1..=3).contains(&mrow) && area.width > 0 {
                                let col = (column as usize).saturating_sub(1);
                                // calcular en qué pestaña cayó el click
                                let mut acc = 2usize; // separador inicial "  "
                                let mut clicked: Option<usize> = None;
                                for (i, t) in TABS.iter().enumerate() {
                                    let w = t.len() + 2; // "N Nombre" + separador
                                    if col >= acc && col < acc + w {
                                        clicked = Some(i);
                                        break;
                                    }
                                    acc += w;
                                }
                                if let Some(i) = clicked {
                                    tab = i;
                                }
                            } else if mrow > 3 {
                                // click en una fila de tabla: seleccionar
                                let len = row_count(tab, &net, &sessions, &harnesses, &tasks, &recalls, &proxy);
                                // la primera fila de datos suele estar en mrow 5 (borde+header)
                                let data_row = (mrow as usize).saturating_sub(5);
                                if data_row < len {
                                    ui.set_sel(tab, len, data_row);
                                }
                            }
                        }
                        MouseEventKind::ScrollUp => {
                            let len = row_count(tab, &net, &sessions, &harnesses, &tasks, &recalls, &proxy);
                            ui.move_sel(tab, len, -1);
                        }
                        MouseEventKind::ScrollDown => {
                            let len = row_count(tab, &net, &sessions, &harnesses, &tasks, &recalls, &proxy);
                            ui.move_sel(tab, len, 1);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        if !ui.paused && last_http.elapsed() >= Duration::from_secs(3) {
            net = poll_net();
            gov = poll_governor();
            proxy = poll_proxy();
            last_http = Instant::now();
            tick += 1;
        }
        if !ui.paused && last_files.elapsed() >= Duration::from_secs(1) {
            sessions = poll_sessions();
            mailbox = poll_mailbox();
            harnesses = poll_harnesses();
            tasks = poll_tasks();
            recalls = poll_recalls();
            last_files = Instant::now();
        }

        terminal.draw(|f| {
            let area = f.area();
            let root = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),  // título
                    Constraint::Length(3),  // pestañas
                    Constraint::Min(0),     // cuerpo
                    Constraint::Length(1),  // status bar
                ])
                .split(area);

            // línea de título (ASCII logo compacto)
            let modelo = load_model_real();
            let paused_mark = if ui.paused { " ⏸ PAUSA" } else { "" };
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(" ◆ ALEXANDRIA ", title_style()),
                    Span::styled(
                        format!(" modelo real: {modelo} · tick {tick}{paused_mark} "),
                        info_value(),
                    ),
                ])),
                root[0],
            );

            // pestañas
            let titles: Vec<Line> = TABS
                .iter()
                .map(|t| Line::from(Span::raw(t.to_string())))
                .collect();
            f.render_widget(
                Tabs::new(titles)
                    .select(tab)
                    .highlight_style(tab_active())
                    .block(Block::default().borders(Borders::ALL).border_style(border_secondary())),
                root[1],
            );

            let body = root[2];
            match tab {
                0 => draw_panel(
                    f,
                    body,
                    PanelSnapshot { net: &net, gov: &gov, proxy: &proxy, sessions: &sessions, harnesses: &harnesses, tasks: &tasks },
                ),
                1 => draw_red(f, body, &net, &gov, ui.sel(1)),
                2 => draw_proxy(f, body, &proxy, ui.sel(2)),
                3 => draw_agentes(f, body, &sessions, &mailbox, ui.sel(3)),
                4 => draw_harnesses(f, body, &harnesses, ui.sel(4)),
                5 => draw_tareas(f, body, &tasks, ui.sel(5)),
                _ => draw_recalls(f, body, &recalls, ui.sel(6), ui.scroll[6]),
            }

            // status bar con hints de teclas + flash message
            let flash = ui.flash.take_if(|(_, t)| t.elapsed() > Duration::from_secs(3));
            let status_line = if let Some((msg, _)) = flash {
                Line::from(vec![
                    Span::styled(" ⚡ ", Style::default().fg(Color::Yellow)),
                    Span::styled(msg, Style::default().fg(Color::Yellow)),
                ])
            } else {
                Line::from(vec![
                    Span::styled(" ↑↓/j/k navegar · ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Enter", Style::default().fg(theme::TEAL)),
                    Span::styled(" detalle · ", Style::default().fg(Color::DarkGray)),
                    Span::styled("r", Style::default().fg(theme::TEAL)),
                    Span::styled(" refrescar · ", Style::default().fg(Color::DarkGray)),
                    Span::styled("espacio", Style::default().fg(theme::TEAL)),
                    Span::styled(" pausa · ", Style::default().fg(Color::DarkGray)),
                    Span::styled("?", Style::default().fg(theme::TEAL)),
                    Span::styled(" ayuda · ", Style::default().fg(Color::DarkGray)),
                    Span::styled("q", Style::default().fg(theme::TEAL)),
                    Span::styled(" salir · ", Style::default().fg(Color::DarkGray)),
                    Span::styled("🖱 click pestañas/filas · scroll rueda", Style::default().fg(Color::DarkGray)),
                ])
            };
            f.render_widget(Paragraph::new(status_line), root[3]);

            // overlay de ayuda
            if ui.show_help {
                let w = area.width.min(64);
                let h = 14;
                let popup = Rect {
                    x: (area.width.saturating_sub(w)) / 2,
                    y: (area.height.saturating_sub(h)) / 2,
                    width: w,
                    height: h,
                };
                f.render_widget(Clear, popup);
                f.render_widget(
                    Paragraph::new(help_text(tab))
                        .wrap(Wrap { trim: false })
                        .block(
                            Block::default()
                                .title(" Ayuda (? para cerrar) ")
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(theme::TEAL)),
                        ),
                    popup,
                );
            }
        })?;
    }
}

// ─────────────────────────────────────────── pestañas

struct PanelSnapshot<'a> {
    net: &'a [NetRow],
    gov: &'a GovernorStats,
    proxy: &'a ProxyView,
    sessions: &'a [SessionRow],
    harnesses: &'a [HarnessRow],
    tasks: &'a [TaskRow],
}

fn draw_panel(f: &mut ratatui::Frame, area: ratatui::layout::Rect, s: PanelSnapshot<'_>) {
    let net = s.net;
    let gov = s.gov;
    let proxy = s.proxy;
    let sessions = s.sessions;
    let harnesses = s.harnesses;
    let tasks = s.tasks;
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(cols[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(cols[1]);

    // resumen de red
    let ups = net.iter().filter(|n| n.ok).count();
    let lineas = vec![
        Line::from(vec![
            Span::raw("servicios    : "),
            Span::styled(
                format!("{ups}/{} vivos", net.len()),
                ok_style(ups == net.len()),
            ),
        ]),
        Line::from(format!(
            "alx-proxy    : {}",
            if proxy.ok { format!("vivo · máscara {}", proxy.visible_model) } else { "CAÍDO (atg cae a routatic)".to_string() }
        )),
        Line::from(format!("en vuelo/gw  : {}", gov.in_flight)),
        Line::from(format!("último model : {}", gov.last_served_model)),
        Line::from(format!(
            "sesiones     : {} registradas",
            sessions.len()
        )),
        Line::from(format!(
            "harnesses    : {} (skill: {})",
            harnesses.len(),
            harnesses.iter().filter(|h| h.id.starts_with("hx-skill-")).count()
        )),
        Line::from(format!(
            "tareas       : {} ({} pending, {} in-progress)",
            tasks.len(),
            tasks.iter().filter(|t| t.status == "Pending").count(),
            tasks.iter().filter(|t| t.status == "InProgress").count(),
        )),
    ];
    f.render_widget(
        Paragraph::new(lineas).block(
            Block::default()
                .title(" Resumen ")
                .borders(Borders::ALL)
                .border_style(border_primary()),
        ),
        left[0],
    );

    // red compacta
    let rows: Vec<Row> = net
        .iter()
        .map(|r| {
            Row::new(vec![
                Span::styled(if r.ok { "✓" } else { "✗" }, ok_style(r.ok)),
                Span::raw(r.name.clone()),
                Span::raw(format!("{}ms", r.ms)),
            ])
        })
        .collect();
    f.render_widget(
        Table::new(rows, [Constraint::Length(2), Constraint::Length(12), Constraint::Length(8)])
            .header(Row::new(vec!["", "servicio", "lat"]).style(table_header()))
            .block(Block::default().title(" Red ").borders(Borders::ALL).border_style(border_primary())),
        left[1],
    );

    // iteración R24
    let (iter, max) = load_iterate();
    let pct = iter.checked_mul(100).and_then(|v| v.checked_div(max)).unwrap_or(0).min(100) as u16;
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(right[0]);
    f.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(PROGRESS))
            .percent(pct)
            .label(format!("iter {iter}/{max} (R24)")),
        inner[0],
    );
    f.render_widget(
        Paragraph::new(format!("última actividad:\n{}", last_activity()))
            .block(Block::default().title(" Actividad ").borders(Borders::ALL).border_style(border_secondary())),
        inner[1],
    );

    // skill-harnesses activos
    let skills: Vec<Line> = harnesses
        .iter()
        .filter(|h| h.steps.is_some())
        .map(|h| {
            let (done, total) = h.steps.unwrap();
            let color = if done == total { Color::Green } else { Color::Yellow };
            Line::from(vec![
                Span::raw(format!("{:<22}", h.id.trim_start_matches("hx-skill-"))),
                Span::styled(format!("{done}/{total} pasos"), Style::default().fg(color)),
            ])
        })
        .collect();
    f.render_widget(
        Paragraph::new(if skills.is_empty() {
            vec![Line::from("ninguna skill en ejecución (Skill tool activa harnesses)")]
        } else {
            skills
        })
        .block(Block::default().title(" Skills en ejecución ").borders(Borders::ALL).border_style(border_secondary())),
        right[1],
    );
}

fn draw_red(f: &mut ratatui::Frame, area: ratatui::layout::Rect, net: &[NetRow], gov: &GovernorStats, selected: usize) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);
    let rows: Vec<Row> = net
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut row = Row::new(vec![
                Span::styled(if r.ok { "✓" } else { "✗" }, ok_style(r.ok)),
                Span::raw(r.name.clone()),
                Span::raw(r.code.clone()),
                Span::raw(format!("{}ms", r.ms)),
                Span::raw(r.url.clone()),
            ]);
            if i == selected {
                row = row.style(Style::default().bg(Color::Indexed(236)).add_modifier(Modifier::BOLD));
            }
            row
        })
        .collect();
    f.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(2),
                Constraint::Length(11),
                Constraint::Length(5),
                Constraint::Length(8),
                Constraint::Min(10),
            ],
        )
        .header(
            Row::new(vec!["", "servicio", "http", "latencia", "url"]).style(table_header()),
        )
        .block(Block::default().title(format!(" Red (GET sin coste) — fila {}/{} ", selected + 1, net.len())).borders(Borders::ALL).border_style(border_primary())),
        cols[0],
    );
    let lines = vec![
        Line::from(format!("en vuelo      : {}", gov.in_flight)),
        Line::from(format!("servidos      : {}", gov.served)),
        Line::styled(format!("reintentos    : {}", gov.retries), ok_style(gov.retries == 0)),
        Line::styled(format!("failovers     : {}", gov.failovers), ok_style(gov.failovers == 0)),
        Line::styled(format!("último modelo : {}", gov.last_served_model), info_value()),
        Line::styled(
            if gov.last_error.is_empty() { "sin errores".into() } else { format!("último error  : {}", gov.last_error) },
            ok_style(gov.last_error.is_empty()),
        ),
    ];
    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(" Gobernador de entropía ")
                .borders(Borders::ALL)
                .border_style(border_secondary()),
        ),
        cols[1],
    );
}

fn draw_proxy(f: &mut ratatui::Frame, area: ratatui::layout::Rect, p: &ProxyView, selected: usize) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    let mut lines = vec![
        Line::from(vec![
            Span::raw("estado       : "),
            Span::styled(if p.ok { "vivo :8797" } else { "CAÍDO" }, ok_style(p.ok)),
        ]),
        Line::styled(format!("máscara      : {}", p.visible_model), info_value()),
        Line::raw(""),
        Line::styled("proveedores", Style::default().add_modifier(Modifier::BOLD)),
    ];
    for (n, proto, tier, keys, models) in &p.providers {
        lines.push(Line::from(format!(
            "  {n:<12} {proto:<9} tier{tier} keys:{keys} modelos:{models}"
        )));
    }
    if !p.breakers.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled("circuitos", Style::default().add_modifier(Modifier::BOLD)));
        for (c, fails, open) in &p.breakers {
            lines.push(Line::styled(
                format!("  {c:<24} fallos:{fails} {}", if *open { "ABIERTO" } else { "ok" }),
                ok_style(!open),
            ));
        }
    }
    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(" alx-proxy :8797 ")
                .borders(Borders::ALL)
                .border_style(border_primary()),
        ),
        cols[0],
    );

    let rows: Vec<Row> = p
        .ledger
        .iter()
        .enumerate()
        .map(|(i, (prov, model, ok, ms))| {
            let mut row = Row::new(vec![
                Span::styled(if *ok { "✓" } else { "✗" }, ok_style(*ok)),
                Span::raw(prov.clone()),
                Span::raw(model.clone()),
                Span::raw(format!("{ms}ms")),
            ]);
            if i == selected {
                row = row.style(Style::default().bg(Color::Indexed(236)).add_modifier(Modifier::BOLD));
            }
            row
        })
        .collect();
    f.render_widget(
        Table::new(
            rows,
            [Constraint::Length(2), Constraint::Length(11), Constraint::Min(14), Constraint::Length(8)],
        )
        .header(
            Row::new(vec!["", "proveedor", "modelo real", "ms"]).style(table_header()),
        )
        .block(
            Block::default()
                .title(format!(" Últimos intentos (ledger) — fila {}/{} ", selected + 1, p.ledger.len()))
                .borders(Borders::ALL)
                .border_style(border_secondary()),
        ),
        cols[1],
    );
}

fn draw_agentes(f: &mut ratatui::Frame, area: ratatui::layout::Rect, sessions: &[SessionRow], mailbox: &[(String, usize)], selected: usize) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);
    let rows: Vec<Row> = sessions
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let mut row = Row::new(vec![
                Span::styled(s.ago.clone(), Style::default().fg(Color::DarkGray)),
                Span::raw(s.id.clone()),
                Span::raw(s.events.to_string()),
                Span::raw(format!("{} {}", s.last_ev, s.last_tool)),
                Span::raw(s.cwd.clone()),
            ]);
            if i == selected {
                row = row.style(Style::default().bg(Color::Indexed(236)).add_modifier(Modifier::BOLD));
            }
            row
        })
        .collect();
    f.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(9),
                Constraint::Length(12),
                Constraint::Length(7),
                Constraint::Length(18),
                Constraint::Min(10),
            ],
        )
        .header(
            Row::new(vec!["visto", "sesión", "eventos", "último evento", "cwd"]).style(table_header()),
        )
        .block(
            Block::default()
                .title(format!(" Sesiones (activity.jsonl) — fila {}/{} ", selected + 1, sessions.len()))
                .borders(Borders::ALL)
                .border_style(border_primary()),
        ),
        cols[0],
    );
    let lines: Vec<Line> = if mailbox.is_empty() {
        vec![Line::from("sin mensajes A2A pendientes"), Line::raw(""), Line::from("envía con: alx mail send <sesión> <msg>")]
    } else {
        mailbox
            .iter()
            .map(|(f_, n)| Line::from(format!("{f_:<28} {n} mensaje(s)")))
            .collect()
    };
    f.render_widget(
        Paragraph::new(lines)
            .block(Block::default().title(" Mailbox A2A ").borders(Borders::ALL).border_style(border_secondary())),
        cols[1],
    );
}

fn draw_harnesses(f: &mut ratatui::Frame, area: ratatui::layout::Rect, hs: &[HarnessRow], selected: usize) {
    let rows: Vec<Row> = hs
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let pasos = h
                .steps
                .map(|(d, t)| format!("{d}/{t}"))
                .unwrap_or_else(|| "—".into());
            let mut row = Row::new(vec![
                Span::raw(h.scope.clone()),
                Span::raw(h.id.clone()),
                Span::raw(h.kind.clone()),
                Span::styled(
                    h.state.clone(),
                    Style::default().fg(match h.state.as_str() {
                        "promoted" | "permanent" => Color::Green,
                        "retired" => Color::DarkGray,
                        _ => Color::Yellow,
                    }),
                ),
                Span::raw(h.uses.to_string()),
                Span::raw(pasos),
            ]);
            if i == selected {
                row = row.style(Style::default().bg(Color::Indexed(236)).add_modifier(Modifier::BOLD));
            }
            row
        })
        .collect();
    f.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(8),
                Constraint::Min(20),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(6),
                Constraint::Length(8),
            ],
        )
        .header(
            Row::new(vec!["scope", "id", "kind", "estado", "usos", "pasos"]).style(table_header()),
        )
        .block(
            Block::default()
                .title(format!(" Harnesses evolutivos — fila {}/{} ", selected + 1, hs.len()))
                .borders(Borders::ALL)
                .border_style(border_primary()),
        ),
        area,
    );
}

fn draw_tareas(f: &mut ratatui::Frame, area: ratatui::layout::Rect, ts: &[TaskRow], selected: usize) {
    let rows: Vec<Row> = ts
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let pct = if t.total > 0 { (t.spent * 100).checked_div(t.total).unwrap_or(0).min(100) } else { 0 };
            let color = state_color(t.status.as_str());
            let mut row = Row::new(vec![
                Span::raw(t.id.clone()),
                Span::raw(t.title.clone()),
                Span::styled(t.status.clone(), Style::default().fg(color)),
                Span::raw(t.phase.clone()),
                Span::raw(format!("{}/{} ({pct}%)", t.spent, t.total)),
            ]);
            if i == selected {
                row = row.style(Style::default().bg(Color::Indexed(236)).add_modifier(Modifier::BOLD));
            }
            row
        })
        .collect();
    f.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(16),
                Constraint::Min(20),
                Constraint::Length(11),
                Constraint::Length(7),
                Constraint::Length(16),
            ],
        )
        .header(
            Row::new(vec!["id", "título", "estado", "fase", "presupuesto"]).style(table_header()),
        )
        .block(
            Block::default()
                .title(format!(" Tareas (tasks.jsonl) — fila {}/{} ", selected + 1, ts.len()))
                .borders(Borders::ALL)
                .border_style(border_primary()),
        ),
        area,
    );
}

fn draw_recalls(f: &mut ratatui::Frame, area: ratatui::layout::Rect, rs: &[(u64, String, String)], selected: usize, scroll: u16) {    let lines: Vec<Line> = if rs.is_empty() {
        vec![Line::from("sin recalls (state/recalls.json)")]
    } else {
        rs.iter()
            .enumerate()
            .map(|(i, (w, src, text))| {
                let mark = if i == selected { "▶" } else { " " };
                let style = if i == selected {
                    Style::default().bg(Color::Indexed(236)).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                Line::from(vec![
                    Span::styled(mark, style),
                    Span::styled(format!(" {w:>2} "), Style::default().fg(Color::Yellow)),
                    Span::styled(format!("{src:<6}"), Style::default().fg(Color::DarkGray)),
                    Span::styled(text.clone(), style),
                ])
            })
            .collect()
    };
    f.render_widget(
        Paragraph::new(lines)
            .scroll((scroll, 0))
            .block(
                Block::default()
                    .title(format!(" Memoria (recalls) — fila {}/{} ", selected + 1, rs.len()))
                    .borders(Borders::ALL)
                    .border_style(border_secondary()),
            ),
        area,
    );
}

// ─────────────────────────────────────────── entrada

pub fn main_tui() -> io::Result<()> {
    let mut stdout = io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        EnableMouseCapture
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let res = run_app(&mut terminal);
    crossterm::execute!(
        io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    crossterm::terminal::disable_raw_mode()?;
    res
}
