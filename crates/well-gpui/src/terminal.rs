use crate::theme::Theme;
use gpui::{rgb, Rgba};
use vt100::Color;

pub fn convert_color(color: Color, default: Rgba, theme: &Theme) -> Rgba {
    match color {
        Color::Default => default,
        Color::Idx(i) => theme.ansi_colors[(i as usize) % 16],
        Color::Rgb(r, g, b) => rgb(((r as u32) << 16) | ((g as u32) << 8) | (b as u32)),
    }
}

/// Computes terminal (cols, rows) from viewport dimensions and font metrics with clamped boundaries.
pub fn calculate_grid_dimensions(width: f32, height: f32, font_size: f32) -> (u16, u16) {
    let char_width = (font_size * 0.6).max(1.0);
    let line_height = (font_size * 1.25).max(1.0);
    let cols = (width / char_width).clamp(1.0, 500.0) as u16;
    let rows = (height / line_height).clamp(1.0, 200.0) as u16;
    (cols, rows)
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
    fn test_convert_color_mappings() {
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
