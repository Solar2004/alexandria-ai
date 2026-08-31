//! alx-ui — sistema compartido de color y estilo ANSI para la salida del CLI `alx`.
//!
//! Un único lugar define la paleta, los símbolos de estado y los helpers de
//! composición. Reglas:
//! - Color solo cuando stdout es TTY (`color_enabled()`); si no, texto plano.
//! - Los helpers devuelven `String` ya envuelto; componer con `format!`.
//! - Sin dependencias externas: ANSI puro, igual que el resto del motor.

use std::io::IsTerminal;
use std::sync::OnceLock;

/// ¿La salida actual admite color? (stdout TTY, sin NO_COLOR, TERM != dumb).
pub fn color_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::io::stdout().is_terminal()
            && std::env::var_os("NO_COLOR").is_none()
            && std::env::var_os("TERM").map(|t| t != "dumb").unwrap_or(true)
    })
}

// ─────────────────────────────────────────── paleta (ANSI SGR)

pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const ITALIC: &str = "\x1b[3m";
pub const UNDERLINE: &str = "\x1b[4m";

// Colores base (8/16)
pub const BLACK: &str = "\x1b[30m";
pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const BLUE: &str = "\x1b[34m";
pub const MAGENTA: &str = "\x1b[35m";
pub const CYAN: &str = "\x1b[36m";
pub const WHITE: &str = "\x1b[37m";
pub const GRAY: &str = "\x1b[90m";

// Colores brillantes (accents de la identidad alx)
pub const BRIGHT_RED: &str = "\x1b[91m";
pub const BRIGHT_GREEN: &str = "\x1b[92m";
pub const BRIGHT_YELLOW: &str = "\x1b[93m";
pub const BRIGHT_BLUE: &str = "\x1b[94m";
pub const BRIGHT_MAGENTA: &str = "\x1b[95m";
pub const BRIGHT_CYAN: &str = "\x1b[96m";
pub const BRIGHT_WHITE: &str = "\x1b[97m";

// 256-color: identidad de marca alx (violeta + teal + naranja)
pub const BRAND: &str = "\x1b[38;5;99m"; // violeta
pub const BRAND_DIM: &str = "\x1b[38;5;61m"; // violeta apagado
pub const TEAL: &str = "\x1b[38;5;80m"; // teal (sugerencias)
pub const ORANGE: &str = "\x1b[38;5;208m"; // naranja (advertencias)
pub const CORAL: &str = "\x1b[38;5;203m"; // coral (errores suaves)

// Fondos
pub const BG_BRAND: &str = "\x1b[48;5;99m";
pub const BG_DARK: &str = "\x1b[48;5;236m";

// ─────────────────────────────────────────── helpers de composición

/// Envuelve `text` con `codes` solo si el color está habilitado.
pub fn paint(codes: &[&str], text: &str) -> String {
    if color_enabled() && !codes.is_empty() {
        format!("{}{text}{RESET}", codes.join(""))
    } else {
        text.to_string()
    }
}

pub fn bold(text: &str) -> String {
    paint(&[BOLD], text)
}

pub fn dim(text: &str) -> String {
    paint(&[DIM], text)
}

pub fn brand(text: &str) -> String {
    paint(&[BRAND, BOLD], text)
}

// ─────────────────────────────────────────── semántica de estado

/// Símbolo de estado: ✓ verde / ✗ rojo (plano sin color).
pub fn ok_mark(ok: bool) -> String {
    if ok {
        paint(&[BRIGHT_GREEN, BOLD], "✓")
    } else {
        paint(&[BRIGHT_RED, BOLD], "✗")
    }
}

/// Línea de estado semántica: `✓ texto` verde o `✗ texto` rojo.
pub fn status_line(ok: bool, text: &str) -> String {
    let (mark, color) = if ok { ("✓", GREEN) } else { ("✗", RED) };
    paint(&[color], &format!("{mark} {text}"))
}

/// Etiqueta de sección: `── Título ` en violeta de marca.
pub fn section(title: &str) -> String {
    paint(&[BRAND, BOLD], &format!("── {title} "))
}

/// Valor destacado (números, modelos, nombres): cian brillante.
pub fn value(text: &str) -> String {
    paint(&[BRIGHT_CYAN], text)
}

/// Advertencia: naranja.
pub fn warn(text: &str) -> String {
    paint(&[ORANGE], text)
}

/// Error: rojo brillante en negrita.
pub fn error(text: &str) -> String {
    paint(&[BRIGHT_RED, BOLD], text)
}

/// Éxito: verde brillante.
pub fn success(text: &str) -> String {
    paint(&[BRIGHT_GREEN], text)
}

/// Metadatos/timestamps: gris.
pub fn meta(text: &str) -> String {
    paint(&[GRAY], text)
}

/// Sugerencia de reparación: `→ texto` en teal.
pub fn hint(text: &str) -> String {
    paint(&[TEAL], &format!("→ {text}"))
}

// ─────────────────────────────────────────── composición de informe

/// Cabecera de informe en color de marca: `══ TÍTULO ══...`.
pub fn banner(title: &str) -> String {
    let width = 60usize.saturating_sub(title.chars().count() + 4).max(2);
    let bar = "═".repeat(width);
    paint(&[BRAND, BOLD], &format!("══ {title} {bar}"))
}

/// Par clave:valor alineado: etiqueta gris + valor cian.
pub fn kv(key: &str, val: &str) -> String {
    format!("{} {}", meta(&format!("{key:<14}")), value(val))
}

/// Línea de ayuda de comando: comando cian en negrita + descripción gris.
pub fn cmd_help(cmd: &str, desc: &str) -> String {
    format!("  {} {}", paint(&[BRIGHT_CYAN, BOLD], cmd), meta(desc))
}

/// Bloque de ejemplos con viñetas grises.
pub fn example(lines: &[&str]) -> String {
    let mut out = String::new();
    for l in lines {
        out.push_str(&format!("  {} {l}\n", meta("·")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paint_plain_without_tty() {
        // En `cargo test` stdout no es TTY → texto plano.
        assert_eq!(paint(&[RED], "x"), "x");
        assert_eq!(bold("y"), "y");
        assert_eq!(brand("z"), "z");
    }

    #[test]
    fn ok_mark_symbols() {
        assert_eq!(ok_mark(true), "✓");
        assert_eq!(ok_mark(false), "✗");
    }

    #[test]
    fn status_line_symbols() {
        assert!(status_line(true, "listo").contains("✓"));
        assert!(status_line(false, "caído").contains("✗"));
    }

    #[test]
    fn hint_prefix() {
        assert!(hint("prueba").starts_with("→ prueba"));
    }
}
