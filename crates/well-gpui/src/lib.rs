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
}
