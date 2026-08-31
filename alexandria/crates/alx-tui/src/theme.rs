//! alx-tui — paleta y estilos compartidos del dashboard.
//!
//! La TUI usa una paleta reducida con significado fijo:
//!
//! - violeta (marca) para identidad: título, pestañas activas, bordes clave
//! - verde/rojo solo para estado real (vivo/caído, promovido/retirado)
//! - amarillo para progreso, gris para metadatos
//!
//! Todo lo demás queda neutro para que el estado sea lo que destaque.

use ratatui::style::{Color, Modifier, Style};

/// Identidad de marca: violeta 256-color usado para título, pestañas y
/// bordes de paneles clave.
pub const BRAND: Color = Color::Indexed(99);
/// Violeta apagado para bordes secundarios.
pub const BRAND_DIM: Color = Color::Indexed(61);
/// Teal para sugerencias y valores informativos.
pub const TEAL: Color = Color::Indexed(80);
/// Amarillo de progreso (gauges, pasos en curso).
pub const PROGRESS: Color = Color::Yellow;
/// Gris de metadatos (timestamps, cwd, texto secundario).
pub const MUTED: Color = Color::DarkGray;

/// Título de la app sobre fondo de marca.
pub fn title_style() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(BRAND)
        .add_modifier(Modifier::BOLD)
}

/// Pestaña activa.
pub fn tab_active() -> Style {
    Style::default().fg(BRAND).add_modifier(Modifier::BOLD)
}

/// Borde de un panel clave (Resumen, Red).
pub fn border_primary() -> Style {
    Style::default().fg(BRAND)
}

/// Borde de un panel secundario.
pub fn border_secondary() -> Style {
    Style::default().fg(BRAND_DIM)
}

/// Cabecera de tabla.
pub fn table_header() -> Style {
    Style::default()
        .fg(Color::Indexed(250))
        .add_modifier(Modifier::BOLD)
}

/// Valor informativo (modelo, máscara).
pub fn info_value() -> Style {
    Style::default().fg(TEAL)
}

/// Estado vivo/caído.
pub fn ok_style(ok: bool) -> Style {
    Style::default().fg(if ok {
        Color::LightGreen
    } else {
        Color::LightRed
    })
}

/// Estado de tarea/harness: Done verde, InProgress amarillo, Blocked rojo,
/// retirado gris, promovido/permanente verde, temporal amarillo.
pub fn state_color(state: &str) -> Color {
    match state {
        "Done" | "promoted" | "permanent" => Color::LightGreen,
        "InProgress" | "temporal" => PROGRESS,
        "Blocked" => Color::LightRed,
        "retired" => MUTED,
        _ => Color::Reset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_colors_are_distinct() {
        assert_ne!(state_color("Done"), state_color("InProgress"));
        assert_ne!(state_color("InProgress"), state_color("Blocked"));
        assert_eq!(state_color("retired"), MUTED);
    }
}
