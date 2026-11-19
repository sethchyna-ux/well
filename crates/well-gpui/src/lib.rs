#![recursion_limit = "256"]

//! Headless subsystems, diagnostics, themes, and terminal coordinate translations for Well GPUI.

pub mod actions;
pub mod diagnostics;
pub mod plugins;
pub mod settings_view;
pub mod ssh_console;
pub mod terminal;
pub mod theme;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_opendyslexic_fonts_integrity() {
        let fonts = well_render::embedded_opendyslexic_fonts();
        assert_eq!(fonts.len(), 6, "Expected 6 embedded OpenDyslexic font assets");
        for (idx, font_cow) in fonts.iter().enumerate() {
            assert!(
                !font_cow.is_empty(),
                "Embedded font at index {idx} must not be empty"
            );
            // Verify font starts with valid OpenType / TrueType magic (0x00010000 or 'OTTO')
            assert!(
                font_cow.starts_with(b"OTTO") || font_cow.starts_with(&[0, 1, 0, 0]),
                "Embedded font at index {idx} must be a valid OpenType / TrueType binary"
            );
        }
    }

    #[test]
    fn test_canonical_font_family_resolution() {
        assert_eq!(
            well_render::canonical_font_family("OpenDyslexic"),
            "OpenDyslexic Nerd Font"
        );
        assert_eq!(
            well_render::canonical_font_family("OpenDyslexic Nerd Font"),
            "OpenDyslexic Nerd Font"
        );
        assert_eq!(
            well_render::canonical_font_family("OpenDyslexicM Nerd Font Mono"),
            "OpenDyslexicM Nerd Font Mono"
        );
        assert_eq!(
            well_render::canonical_font_family("OpenDyslexicMono"),
            "OpenDyslexicM Nerd Font Mono"
        );
        assert_eq!(
            well_render::canonical_font_family("dyslexic-mono"),
            "OpenDyslexicM Nerd Font Mono"
        );
        assert_eq!(
            well_render::canonical_font_family("Menlo"),
            "Menlo"
        );
        assert_eq!(
            well_render::canonical_font_family("JetBrains Mono"),
            "JetBrains Mono"
        );
    }

    #[test]
    fn test_terminal_settings_initializes_with_opendyslexic() {
        let settings = settings_view::TerminalSettings::new();
        assert!(
            settings.font_family.contains("OpenDyslexic") || settings.font_family == "OpenDyslexic Nerd Font",
            "Initial font family should be OpenDyslexic, found: {}",
            settings.font_family
        );
    }

    fn mk_keystroke(key: &str, key_char: Option<&str>, shift: bool) -> gpui::Keystroke {
        gpui::Keystroke {
            modifiers: gpui::Modifiers {
                shift,
                ..Default::default()
            },
            key: key.to_string(),
            key_char: key_char.map(str::to_string),
        }
    }

    #[test]
    fn test_shift_and_capslock_printable_char_and_terminal_bytes() {
        // Unshifted lowercase letter
        let ks_lower = mk_keystroke("a", Some("a"), false);
        assert_eq!(
            settings_view::extract_printable_char_with_capslock(&ks_lower, false).as_deref(),
            Some("a")
        );
        assert_eq!(
            settings_view::encode_terminal_key_bytes(&ks_lower, false),
            b"a"
        );

        // Shifted letter with key_char = Some("A")
        let ks_shift_with_char = mk_keystroke("a", Some("A"), true);
        assert_eq!(
            settings_view::extract_printable_char_with_capslock(&ks_shift_with_char, false)
                .as_deref(),
            Some("A")
        );
        assert_eq!(
            settings_view::encode_terminal_key_bytes(&ks_shift_with_char, false),
            b"A"
        );

        // Shifted letter with key_char = None (synthetic / fallback path)
        let ks_shift_no_char = mk_keystroke("a", None, true);
        assert_eq!(
            settings_view::extract_printable_char_with_capslock(&ks_shift_no_char, false)
                .as_deref(),
            Some("A")
        );
        assert_eq!(
            settings_view::encode_terminal_key_bytes(&ks_shift_no_char, false),
            b"A"
        );

        // Caps Lock active on macOS (key_char is lowercase "a", shift is false, capslock_on = true)
        assert_eq!(
            settings_view::extract_printable_char_with_capslock(&ks_lower, true).as_deref(),
            Some("A")
        );
        assert_eq!(
            settings_view::encode_terminal_key_bytes(&ks_lower, true),
            b"A"
        );

        // Caps Lock active on Linux (key_char is already "A", capslock_on = true)
        let ks_linux_caps = mk_keystroke("a", Some("A"), false);
        assert_eq!(
            settings_view::extract_printable_char_with_capslock(&ks_linux_caps, true).as_deref(),
            Some("A")
        );
        assert_eq!(
            settings_view::encode_terminal_key_bytes(&ks_linux_caps, true),
            b"A"
        );

        // Caps Lock active with digits and punctuation leaves them unchanged
        let ks_digit = mk_keystroke("1", Some("1"), false);
        assert_eq!(
            settings_view::extract_printable_char_with_capslock(&ks_digit, true).as_deref(),
            Some("1")
        );
        assert_eq!(
            settings_view::encode_terminal_key_bytes(&ks_digit, true),
            b"1"
        );

        // Shifted symbols: platform-resolved and synthetic fallback
        let ks_bang_resolved = mk_keystroke("!", Some("!"), false);
        assert_eq!(
            settings_view::extract_printable_char_with_capslock(&ks_bang_resolved, false)
                .as_deref(),
            Some("!")
        );
        assert_eq!(
            settings_view::encode_terminal_key_bytes(&ks_bang_resolved, false),
            b"!"
        );

        let ks_bang_synthetic = mk_keystroke("1", None, true);
        assert_eq!(
            settings_view::extract_printable_char_with_capslock(&ks_bang_synthetic, false)
                .as_deref(),
            Some("!")
        );
        assert_eq!(
            settings_view::encode_terminal_key_bytes(&ks_bang_synthetic, false),
            b"!"
        );

        // Shift+Tab vs Tab
        let ks_tab = mk_keystroke("tab", Some("\t"), false);
        let ks_shift_tab = mk_keystroke("tab", None, true);
        assert_eq!(
            settings_view::encode_terminal_key_bytes(&ks_tab, false),
            b"\t"
        );
        assert_eq!(
            settings_view::encode_terminal_key_bytes(&ks_shift_tab, false),
            b"\x1b[Z"
        );
    }
}
