//! Two accent colours for the TUI: `Color::Rgb` when the terminal
//! advertises truecolor support, falling back to the nearest 256-colour
//! (`Color::Indexed`) value otherwise — "assume some corporate
//! terminals lack truecolor," per `PLAN.md`.

use ratatui::style::Color;

/// The `COLORTERM` sniff every major terminal emulator sets when it
/// supports 24-bit colour (kitty, Alacritty, iTerm2, WezTerm, the VS
/// Code integrated terminal, GNOME Terminal, …) — the de facto standard
/// way CLI tools detect this, since there is no ANSI-queryable
/// capability for it.
pub fn supports_truecolor() -> bool {
    detect_truecolor(std::env::var("COLORTERM").ok().as_deref())
}

fn detect_truecolor(colorterm: Option<&str>) -> bool {
    matches!(colorterm, Some("truecolor") | Some("24bit"))
}

/// Primary accent: a cyan used for active/working chrome (the sprite
/// while working, the active pane border, in-progress status text).
pub fn accent_primary(truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(0x4d, 0xd0, 0xc7)
    } else {
        Color::Indexed(80) // nearest 256-colour teal/cyan
    }
}

/// Secondary accent: a warm amber for completion/success chrome (the
/// sprite once done, tool-call highlights) — distinct from the primary
/// without clashing against it.
pub fn accent_secondary(truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(0xe0, 0xa4, 0x58)
    } else {
        Color::Indexed(179) // nearest 256-colour amber
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_standard_truecolor_values() {
        assert!(detect_truecolor(Some("truecolor")));
        assert!(detect_truecolor(Some("24bit")));
        assert!(!detect_truecolor(Some("256color")));
        assert!(!detect_truecolor(None));
    }

    #[test]
    fn truecolor_and_fallback_both_yield_distinct_accents() {
        assert!(matches!(accent_primary(true), Color::Rgb(..)));
        assert!(matches!(accent_primary(false), Color::Indexed(_)));
        assert!(matches!(accent_secondary(true), Color::Rgb(..)));
        assert!(matches!(accent_secondary(false), Color::Indexed(_)));
        assert_ne!(accent_primary(true), accent_secondary(true));
        assert_ne!(accent_primary(false), accent_secondary(false));
    }
}
