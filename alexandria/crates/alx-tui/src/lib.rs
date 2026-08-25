//! alx-tui — dashboard vivo de ALEXANDRIA (ratatui).
//!
//! `alx tui` abre este dashboard. Todo sondeo es GET local y barato (la lección
//! de la saturación: NUNCA un POST de generación para pintar una pantalla).
//!
//! Paneles:
//!   - Red: gateway/headroom/routatic/omniroute (GET con timeout corto)
//!   - Gobernador: telemetría del routa-gateway (/stats)
//!   - Harnesses: registry evolutivo (alx-evolve)
//!   - Iteración: estado R24 del bucle VERIFICA→CRITICA→MEJORA
//!
//! Teclas: q/Esc salir · r refrescar ahora · c forzar cooldown off (no implementado).

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Row, Table};
use ratatui::Terminal;

// ------------------------------------------------------------------ datos
pub struct NetRow {
    pub name: String,
    pub url: String,
    pub code: String,
    pub ok: bool,
    pub ms: u128,
}

/// GET HTTP mínimo sobre TCP crudo — sin dependencias ni proxies raros.
/// Devuelve (código HTTP, latencia ms). None = caído/timeout.
fn http_get_simple(url: &str, timeout_ms: u64) -> Option<(u16, u128)> {
    let rest = url.strip_prefix("http://")?;
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let t0 = Instant::now();
    let mut stream = TcpStream::connect(hostport).ok()?;
    stream.set_read_timeout(Some(Duration::from_millis(timeout_ms))).ok()?;
    stream.set_write_timeout(Some(Duration::from_millis(timeout_ms))).ok()?;
    let req = format!("GET {path} HTTP/1.1\r\nHost: {hostport}\r\nUser-Agent: alx-tui\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).ok()?;
    let mut buf = vec![0u8; 4096];
    let mut leido = Vec::new();
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                leido.extend_from_slice(&buf[..n]);
                if leido.len() > 65536 {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let ms = t0.elapsed().as_millis();
    let head = String::from_utf8_lossy(&leido);
    let code = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse::<u16>().ok())?;
    Some((code, ms))
}

fn poll_net() -> Vec<NetRow> {
    let endpoints = [
        ("gateway", "http://127.0.0.1:3460/health"),
        ("headroom", "http://127.0.0.1:8788/readyz"),
        ("routatic", "http://127.0.0.1:3456/v1/models"),
        ("omniroute", "http://127.0.0.1:20128/"),
    ];
    endpoints
        .iter()
        .map(|(name, url)| match http_get_simple(url, 1500) {
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
    queued_peak: u32,
    served: u32,
    retries: u32,
    failovers: u32,
    breaker_opens: u32,
    last_served_model: String,
    last_error: String,
}

fn poll_governor() -> GovernorStats {
    let body = match http_get_body("http://127.0.0.1:3460/stats", 1500) {
        Some(b) => b,
        None => return GovernorStats::default(),
    };
    let v: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let g = &v["governor"];
    GovernorStats {
        in_flight: g["in_flight"].as_u64().unwrap_or(0) as u32,
        queued_peak: g["queued_peak"].as_u64().unwrap_or(0) as u32,
        served: g["served"].as_u64().unwrap_or(0) as u32,
        retries: g["retries"].as_u64().unwrap_or(0) as u32,
        failovers: g["failovers"].as_u64().unwrap_or(0) as u32,
        breaker_opens: g["breaker_opens"].as_u64().unwrap_or(0) as u32,
        last_served_model: g["last_served_model"].as_str().unwrap_or("").to_string(),
        last_error: g["last_error"].as_str().unwrap_or("").chars().take(80).collect(),
    }
}

fn http_get_body(url: &str, timeout_ms: u64) -> Option<String> {
    let rest = url.strip_prefix("http://")?;
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let mut stream = TcpStream::connect(hostport).ok()?;
    stream.set_read_timeout(Some(Duration::from_millis(timeout_ms))).ok()?;
    stream.set_write_timeout(Some(Duration::from_millis(timeout_ms))).ok()?;
    let req = format!("GET {path} HTTP/1.1\r\nHost: {hostport}\r\nUser-Agent: alx-tui\r\nConnection: close\r\n\r\n");
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
    // separar headers del body (HTTP/1.1 close-delimited aquí)
    let sep = leido.windows(4).position(|w| w == b"\r\n\r\n")?;
    Some(String::from_utf8_lossy(&leido[sep + 4..]).into_owned())
}

fn load_iterate() -> (u32, u32) {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../harnesses/iterate/state.toml");
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
    let path = format!("{home}/.config/routatic-proxy/config.json");
    let txt = std::fs::read_to_string(path).unwrap_or_default();
    let v: serde_json::Value = serde_json::from_str(&txt).unwrap_or_default();
    v["models"]["default"]["model_id"].as_str().unwrap_or("?").to_string()
}

// ------------------------------------------------------------------- vista
fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut last_poll = Instant::now() - Duration::from_secs(60);
    let mut net = Vec::new();
    let mut gov = GovernorStats::default();
    let mut tick = 0u64;
    loop {
        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(k) = event::read()? {
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('r') => last_poll = Instant::now() - Duration::from_secs(60),
                    _ => {}
                }
            }
        }
        if last_poll.elapsed() >= Duration::from_secs(5) {
            net = poll_net();
            gov = poll_governor();
            last_poll = Instant::now();
            tick += 1;
        }

        terminal.draw(|f| {
            let area = f.area();
            let root = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(0)])
                .split(area);
            let modelo = load_model_real();
            let header = Paragraph::new(Line::from(vec![
                Span::styled(" ALEXANDRIA ", Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(format!("modelo real: {modelo}"), Style::default().fg(Color::Yellow)),
                Span::raw("   · q salir · r refrescar"),
            ]))
            .block(Block::default().borders(Borders::NONE));
            f.render_widget(header, root[0]);

            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                .split(root[1]);
            let left = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(cols[0]);
            let right = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(cols[1]);

            // -- panel red --
            let rows: Vec<Row> = net
                .iter()
                .map(|r| {
                    let mark = if r.ok { "✓" } else { "✗" };
                    let color = if r.ok { Color::Green } else { Color::Red };
                    Row::new(vec![
                        Span::styled(mark.to_string(), Style::default().fg(color)),
                        Span::raw(r.name.clone()),
                        Span::raw(r.code.clone()),
                        Span::raw(format!("{}ms", r.ms)),
                    ])
                })
                .collect();
            let tabla = Table::new(
                rows,
                [
                    Constraint::Length(2),
                    Constraint::Length(12),
                    Constraint::Length(5),
                    Constraint::Length(7),
                ],
            )
            .header(Row::new(vec!["", "servicio", "http", "latencia"]).style(Style::default().add_modifier(Modifier::BOLD)))
            .block(Block::default().title(" Red (GET sin coste) ").borders(Borders::ALL));
            f.render_widget(tabla, left[0]);

            // -- panel harnesses --
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../harnesses");
            let reg = alx_evolve::HarnessRegistry::load_from(&dir);
            let hrows: Vec<Row> = reg
                .all()
                .iter()
                .map(|h| {
                    Row::new(vec![
                        Span::raw(h.id.clone()),
                        Span::raw(h.uses.to_string()),
                        Span::raw(format!("{:?}", h.state).to_lowercase()),
                    ])
                })
                .collect();
            let htabla = Table::new(
                hrows,
                [Constraint::Length(20), Constraint::Length(5), Constraint::Length(18)],
            )
            .header(Row::new(vec!["harness", "usos", "estado"]).style(Style::default().add_modifier(Modifier::BOLD)))
            .block(Block::default().title(" Auto-mejora (R20-R23) ").borders(Borders::ALL));
            f.render_widget(htabla, left[1]);

            // -- panel gobernador --
            let lines = vec![
                Line::from(format!("en vuelo      : {}", gov.in_flight)),
                Line::from(format!("cola pico     : {}", gov.queued_peak)),
                Line::from(format!("servidos      : {}", gov.served)),
                Line::from(format!("reintentos    : {}", gov.retries)),
                Line::from(format!("failovers     : {}", gov.failovers)),
                Line::from(format!("fusible abrió : {}", gov.breaker_opens)),
                Line::from(format!("último modelo : {}", gov.last_served_model)),
                Line::styled(
                    if gov.last_error.is_empty() {
                        "sin errores".to_string()
                    } else {
                        format!("último error  : {}", gov.last_error)
                    },
                    Style::default().fg(if gov.last_error.is_empty() { Color::Green } else { Color::Red }),
                ),
            ];
            let gp = Paragraph::new(lines)
                .block(Block::default().title(" Gobernador de entropía ").borders(Borders::ALL));
            f.render_widget(gp, right[0]);

            // -- panel iteración --
            let (iter, max) = load_iterate();
            let pct = iter.checked_mul(100).and_then(|v| v.checked_div(max)).unwrap_or(0).min(100) as u16;
            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(0)])
                .split(right[1]);
            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(Color::Yellow))
                .percent(pct)
                .label(format!("iter {iter}/{max}"));
            f.render_widget(gauge, inner[0]);
            let nota = Paragraph::new(format!(
                "ciclos de pantalla: {tick}\nVERIFICA → CRITICA → MEJORA (R24)\ncada sesión: SessionStart cicla `alx evolve`"
            ))
            .block(Block::default().title(" Bucle de iteración ").borders(Borders::ALL));
            f.render_widget(nota, inner[1]);
        })?;
    }
}

pub fn main_tui() -> io::Result<()> {
    let mut stdout = io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let res = run_app(&mut terminal);
    crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen)?;
    crossterm::terminal::disable_raw_mode()?;
    res
}
