use crate::theme::Theme;
use gpui::{rgb, Rgba};
use vt100::Color;

pub fn convert_color(color: Color, default: Rgba, theme: &Theme) -> Rgba {
    match color {
        Color::Default => default,
        Color::Idx(i @ 0..=15) => theme.ansi_colors[i as usize],
        Color::Idx(i @ 16..=231) => {
            let n = i as u32 - 16;
            let levels = [0, 95, 135, 175, 215, 255];
            rgb((levels[(n / 36) as usize] << 16)
                | (levels[((n / 6) % 6) as usize] << 8)
                | levels[(n % 6) as usize])
        }
        Color::Idx(i) => {
            let c = 8 + 10 * (i as u32 - 232);
            rgb((c << 16) | (c << 8) | c)
        }

        Color::Rgb(r, g, b) => rgb(((r as u32) << 16) | ((g as u32) << 8) | (b as u32)),
    }
}

/// Computes terminal (cols, rows) from viewport dimensions and font metrics with clamped boundaries.
#[allow(dead_code)]
pub fn calculate_grid_dimensions(width: f32, height: f32, font_size: f32) -> (u16, u16) {
    let char_width = (font_size * 0.6).max(1.0);
    let line_height = (font_size * 1.25).max(1.0);
    let cols = (width / char_width).clamp(1.0, 500.0) as u16;
    let rows = (height / line_height).clamp(1.0, 200.0) as u16;
    (cols, rows)
}

/// Returns fixed cell `(char_width, line_height)` in logical pixels for a given font family and size.
/// OpenDyslexic has wider horizontal advance and taller ascenders/descenders than condensed monospace fonts,
/// so we allocate a wider cell and integer-ceiled line height to prevent bottom overflow and horizontal drift.
pub fn cell_metrics_for_font(font_family: &str, font_size: f32) -> (f32, f32) {
    let is_dyslexic = font_family.to_ascii_lowercase().contains("dyslexic");
    let char_width = if is_dyslexic {
        (font_size * 0.68).max(1.0)
    } else {
        (font_size * 0.60).max(1.0)
    };
    let line_height = if is_dyslexic {
        (font_size * 1.52).ceil().max(1.0)
    } else {
        (font_size * 1.32).ceil().max(1.0)
    };
    (char_width, line_height)
}

/// Computes terminal `(cols, rows)` using font-aware cell metrics so the PTY row count matches
/// the exact number of fixed-height rows that fit inside the visible terminal pane.
pub fn calculate_grid_dimensions_for_font(
    width: f32,
    height: f32,
    font_family: &str,
    font_size: f32,
) -> (u16, u16) {
    let (char_width, line_height) = cell_metrics_for_font(font_family, font_size);
    let cols = (width / char_width).floor().clamp(1.0, 500.0) as u16;
    let rows = (height / line_height).floor().clamp(1.0, 200.0) as u16;
    (cols, rows)
}

pub fn image_format(bytes: &[u8]) -> Result<gpui::ImageFormat, String> {
    let format = image::guess_format(bytes).map_err(|e| e.to_string())?;
    gpui::ImageFormat::from_mime_type(format.to_mime_type())
        .ok_or_else(|| "Image format is not supported by GPUI".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine;

    #[test]
    fn test_grid_dimension_calculation_standard_and_clamped() {
        let font_size = 14.0f32;
        let char_width = font_size * 0.6;
        let line_height = font_size * 1.25;

        // Standard 80x24 layout
        let (cols, rows) =
            calculate_grid_dimensions(80.0 * char_width, 24.0 * line_height, font_size);
        assert_eq!(cols, 80);
        assert_eq!(rows, 24);

        // Clamped minimum (tiny bounds)
        let (min_cols, min_rows) = calculate_grid_dimensions(0.5, 0.5, font_size);
        assert_eq!(min_cols, 1);
        assert_eq!(min_rows, 1);

        // Clamped maximum (huge bounds)
        let (max_cols, max_rows) = calculate_grid_dimensions(100_000.0, 50_000.0, font_size);
        assert_eq!(max_cols, 500);
        assert_eq!(max_rows, 200);
    }

    #[test]
    fn test_font_aware_grid_dimensions_never_overflow_viewport() {
        for font_size in [13.5f32, 16.0, 21.0, 24.0] {
            for family in ["OpenDyslexic Nerd Font", "JetBrains Mono"] {
                let (cw, lh) = cell_metrics_for_font(family, font_size);
                let usable_w = 1040.0f32;
                let usable_h = 720.0f32;
                let (cols, rows) =
                    calculate_grid_dimensions_for_font(usable_w, usable_h, family, font_size);
                assert!((cols as f32) * cw <= usable_w);
                assert!((rows as f32) * lh <= usable_h);
            }
        }
    }

    #[test]
    fn test_convert_color_mappings() {
        let theme = Theme::get(0);
        assert_eq!(
            convert_color(Color::Idx(196), theme.fg, &theme),
            rgb(0xff0000)
        );
        assert_eq!(
            convert_color(Color::Idx(231), theme.fg, &theme),
            rgb(0xffffff)
        );
        assert_eq!(
            convert_color(Color::Idx(232), theme.fg, &theme),
            rgb(0x080808)
        );
        assert_eq!(
            convert_color(Color::Idx(255), theme.fg, &theme),
            rgb(0xeeeeee)
        );
        let theme = Theme::theia_prism();
        let default_color = rgb(0x123456);

        // Default maps to fallback default
        assert_eq!(
            convert_color(Color::Default, default_color, &theme),
            default_color
        );

        // Idx maps to ANSI table
        assert_eq!(
            convert_color(Color::Idx(0), default_color, &theme),
            theme.ansi_colors[0]
        );
        assert_eq!(
            convert_color(Color::Idx(15), default_color, &theme),
            theme.ansi_colors[15]
        );

        // Direct RGB maps cleanly
        let direct_rgb = convert_color(Color::Rgb(254, 128, 25), default_color, &theme);
        assert_eq!(direct_rgb, rgb(0xfe8019));
    }

    #[test]
    fn test_kitty_protocol_base64_decoding_and_magic() {
        let png_1x1_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";
        let decoded = BASE64.decode(png_1x1_b64).expect("valid base64");
        assert_eq!(&decoded[0..8], b"\x89PNG\r\n\x1a\n");
    }
}
