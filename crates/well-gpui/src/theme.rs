use gpui::{rgb, Rgba};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomThemeHex {
    pub name: String,
    pub bg: String,
    pub fg: String,
    pub border_active: String,
    pub border_inactive: String,
    pub tab_active_bg: String,
    pub tab_inactive_bg: String,
}

impl Default for CustomThemeHex {
    fn default() -> Self {
        Self::from_theme(&Theme::theia_prism())
    }
}

impl CustomThemeHex {
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            name: "Custom Theme".into(),
            bg: rgba_to_hex(theme.bg),
            fg: rgba_to_hex(theme.fg),
            border_active: rgba_to_hex(theme.border_active),
            border_inactive: rgba_to_hex(theme.border_inactive),
            tab_active_bg: rgba_to_hex(theme.tab_active_bg),
            tab_inactive_bg: rgba_to_hex(theme.tab_inactive_bg),
        }
    }

    pub fn to_theme(&self) -> Theme {
        let base = Theme::theia_prism();
        Theme {
            id: 5,
            name: if self.name.trim().is_empty() {
                "Custom Theme".into()
            } else {
                self.name.clone()
            },
            bg: parse_hex_color(&self.bg).map(rgb).unwrap_or(base.bg),
            fg: parse_hex_color(&self.fg).map(rgb).unwrap_or(base.fg),
            border_active: parse_hex_color(&self.border_active)
                .map(rgb)
                .unwrap_or(base.border_active),
            border_inactive: parse_hex_color(&self.border_inactive)
                .map(rgb)
                .unwrap_or(base.border_inactive),
            tab_active_bg: parse_hex_color(&self.tab_active_bg)
                .map(rgb)
                .unwrap_or(base.tab_active_bg),
            tab_inactive_bg: parse_hex_color(&self.tab_inactive_bg)
                .map(rgb)
                .unwrap_or(base.tab_inactive_bg),
            ansi_colors: base.ansi_colors,
        }
    }
}

/// Parses `#RRGGBB`, `RRGGBB`, `0xRRGGBB`, or 3-digit `#RGB` into a 24-bit `u32` RGB value.
pub fn parse_hex_color(input: &str) -> Option<u32> {
    let trimmed = input.trim();
    let hex = trimmed
        .strip_prefix('#')
        .or_else(|| trimmed.strip_prefix("0x"))
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);

    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        u32::from_str_radix(hex, 16).ok()
    } else if hex.len() == 3 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        let mut expanded = String::with_capacity(6);
        for ch in hex.chars() {
            expanded.push(ch);
            expanded.push(ch);
        }
        u32::from_str_radix(&expanded, 16).ok()
    } else {
        None
    }
}

/// Formats a `gpui::Rgba` color into `#RRGGBB` uppercase hex notation.
pub fn rgba_to_hex(color: Rgba) -> String {
    let r = (color.r.clamp(0.0, 1.0) * 255.0).round() as u8;
    let g = (color.g.clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = (color.b.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{r:02X}{g:02X}{b:02X}")
}

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub id: u32,
    pub name: String,
    pub bg: Rgba,
    pub fg: Rgba,
    pub border_active: Rgba,
    pub border_inactive: Rgba,
    pub tab_active_bg: Rgba,
    pub tab_inactive_bg: Rgba,
    pub ansi_colors: [Rgba; 16],
}

impl Theme {
    pub fn get(id: u32) -> Self {
        match id {
            0 => Self::theia_prism(),
            1 => Self::cyber_neon(),
            2 => Self::tokyo_night(),
            3 => Self::dracula(),
            4 => Self::crimson_prod_ssh(),
            _ => Self::theia_prism(),
        }
    }

    pub fn theia_prism() -> Self {
        Self {
            id: 0,
            name: "Theia's Prism".into(),
            bg: rgb(0x1d2021),
            fg: rgb(0xebdbb2),
            border_active: rgb(0xfe8019),
            border_inactive: rgb(0x3c3836),
            tab_active_bg: rgb(0x282828),
            tab_inactive_bg: rgb(0x1d2021),
            ansi_colors: [
                rgb(0x282828),
                rgb(0xcc241d),
                rgb(0x98971a),
                rgb(0xd79921),
                rgb(0x458588),
                rgb(0xb16286),
                rgb(0x689d6a),
                rgb(0xa89984),
                rgb(0x928374),
                rgb(0xfb4934),
                rgb(0xb8bb26),
                rgb(0xfabd2f),
                rgb(0x83a598),
                rgb(0xd3869b),
                rgb(0x8ec07c),
                rgb(0xebdbb2),
            ],
        }
    }

    pub fn cyber_neon() -> Self {
        Self {
            id: 1,
            name: "Cyber-Neon".into(),
            bg: rgb(0x0a0a0f),
            fg: rgb(0x00ffff), // Changed to Cyan
            border_active: rgb(0x00ffff),
            border_inactive: rgb(0x222233),
            tab_active_bg: rgb(0x1a1a24),
            tab_inactive_bg: rgb(0x0f0f15),
            ansi_colors: [
                rgb(0x000000),
                rgb(0xff003c),
                rgb(0x39ff14),
                rgb(0xf3f315),
                rgb(0x00ffff),
                rgb(0xff00ff),
                rgb(0x00ffff),
                rgb(0xffffff),
                rgb(0x333333),
                rgb(0xff4d79),
                rgb(0x8aff70),
                rgb(0xffff70),
                rgb(0x70ffff),
                rgb(0xff70ff),
                rgb(0x70ffff),
                rgb(0xffffff),
            ],
        }
    }

    pub fn tokyo_night() -> Self {
        Self {
            id: 2,
            name: "Tokyo Night".into(),
            bg: rgb(0x1a1b26),
            fg: rgb(0x7dcfff), // Changed to Light Blue
            border_active: rgb(0x7aa2f7),
            border_inactive: rgb(0x292e42),
            tab_active_bg: rgb(0x24283b),
            tab_inactive_bg: rgb(0x1f2335),
            ansi_colors: [
                rgb(0x15161e),
                rgb(0xf7768e),
                rgb(0x9ece6a),
                rgb(0xe0af68),
                rgb(0x7aa2f7),
                rgb(0xbb9af7),
                rgb(0x7dcfff),
                rgb(0xa9b1d6),
                rgb(0x414868),
                rgb(0xf7768e),
                rgb(0x9ece6a),
                rgb(0xe0af68),
                rgb(0x7aa2f7),
                rgb(0xbb9af7),
                rgb(0x7dcfff),
                rgb(0xc0caf5),
            ],
        }
    }

    pub fn dracula() -> Self {
        Self {
            id: 3,
            name: "Dracula".into(),
            bg: rgb(0x282a36),
            fg: rgb(0x8be9fd), // Changed to Cyan

            border_active: rgb(0xbd93f9),
            border_inactive: rgb(0x44475a),
            tab_active_bg: rgb(0x44475a),
            tab_inactive_bg: rgb(0x282a36),
            ansi_colors: [
                rgb(0x21222c),
                rgb(0xff5555),
                rgb(0x50fa7b),
                rgb(0xf1fa8c),
                rgb(0xbd93f9),
                rgb(0xff79c6),
                rgb(0x8be9fd),
                rgb(0xf8f8f2),
                rgb(0x6272a4),
                rgb(0xff6e6e),
                rgb(0x69ff94),
                rgb(0xffffa5),
                rgb(0xd6acff),
                rgb(0xff92df),
                rgb(0xa4ffff),
                rgb(0xffffff),
            ],
        }
    }

    pub fn crimson_prod_ssh() -> Self {
        Self {
            id: 4,
            name: "Crimson Prod SSH".into(),
            bg: rgb(0x221112),
            fg: rgb(0xfbf1c7),
            border_active: rgb(0xfb4934),
            border_inactive: rgb(0x5c2320),
            tab_active_bg: rgb(0x3c1818),
            tab_inactive_bg: rgb(0x221112),
            ansi_colors: [
                rgb(0x281818),
                rgb(0xcc241d),
                rgb(0x98971a),
                rgb(0xd79921),
                rgb(0x458588),
                rgb(0xb16286),
                rgb(0x689d6a),
                rgb(0xa89984),
                rgb(0x928374),
                rgb(0xfb4934),
                rgb(0xb8bb26),
                rgb(0xfabd2f),
                rgb(0x83a598),
                rgb(0xd3869b),
                rgb(0x8ec07c),
                rgb(0xebdbb2),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theia_prism_palette_tokens() {
        let theia = Theme::theia_prism();
        assert_eq!(theia.id, 0);
        assert_eq!(theia.name, "Theia's Prism");
        assert_eq!(theia.bg, rgb(0x1d2021));
        assert_eq!(theia.border_active, rgb(0xfe8019));
        assert_eq!(theia.fg, rgb(0xebdbb2));
    }

    #[test]
    fn test_theme_presets_ansi_colors_defined() {
        for id in 0..=3 {
            let t = Theme::get(id);
            assert_eq!(t.ansi_colors.len(), 16);
            assert!(!t.name.is_empty());
        }
        // Unknown id defaults safely to Theia's Prism
        let fallback = Theme::get(999);
        assert_eq!(fallback.name, "Theia's Prism");
    }

    #[test]
    fn test_parse_hex_and_custom_theme_roundtrip() {
        assert_eq!(parse_hex_color("#fe8019"), Some(0xfe8019));
        assert_eq!(parse_hex_color("1D2021"), Some(0x1d2021));
        assert_eq!(parse_hex_color("#0ff"), Some(0x00ffff));
        assert_eq!(parse_hex_color("invalid"), None);

        let mut custom = CustomThemeHex::from_theme(&Theme::tokyo_night());
        custom.bg = "#101216".into();
        custom.border_active = "#00FFAA".into();
        let theme = custom.to_theme();
        assert_eq!(theme.id, 5);
        assert_eq!(theme.bg, rgb(0x101216));
        assert_eq!(theme.border_active, rgb(0x00ffaa));
        assert_eq!(rgba_to_hex(theme.border_active), "#00FFAA");
    }
}
