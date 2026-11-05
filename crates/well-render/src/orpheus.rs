//! Orpheus is the canonical font-backed terminal renderer.
//! This module retains the subsystem name without a separate rendering path.
pub use crate::text::{CellMetrics, TextRenderer as OrpheusRenderer};

/// Packed per-cell instance data for Orpheus terminal cell quads.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CellInstance {
    pub grid_position: [u32; 2],
    pub glyph_id_and_effects: u32,
    pub packed_colors: u32,
}

/// Options controlling cell building from a terminal screen.
#[derive(Clone, Copy, Debug)]
pub struct CellBuildOptions {
    pub rows: u16,
    pub cols: u16,
    pub cursor_visible: bool,
    pub cursor_style: u8,
}

impl CellBuildOptions {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            rows,
            cols,
            cursor_visible: true,
            cursor_style: 0,
        }
    }
}

/// A 2D selection bounding range in terminal grid coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionRange {
    pub start_col: u16,
    pub start_row: u16,
    pub end_col: u16,
    pub end_row: u16,
}

impl SelectionRange {
    pub fn new(start_col: u16, start_row: u16, end_col: u16, end_row: u16) -> Self {
        Self {
            start_col,
            start_row,
            end_col,
            end_row,
        }
    }
}

/// 16-color ANSI palette converted to 12-bit RGB (0xRGB).
const ANSI_12BIT: [u16; 16] = [
    0x000, // 0: Black
    0xF08, // 1: Red (Well prompt/cyber accent)
    0x0F0, // 2: Green
    0xFF0, // 3: Yellow
    0x00F, // 4: Blue
    0xF0F, // 5: Magenta
    0x0FF, // 6: Cyan
    0xCCC, // 7: White
    0x888, // 8: Bright Black
    0xF55, // 9: Bright Red
    0x5F5, // 10: Bright Green
    0xFF5, // 11: Bright Yellow
    0x55F, // 12: Bright Blue
    0xF5F, // 13: Bright Magenta
    0x5FF, // 14: Bright Cyan
    0xFFF, // 15: Bright White
];

/// Converts a `vt100::Color` into a 12-bit RGB representation (`0xRGB`).
pub fn vt100_color_to_rgb(color: vt100::Color, default: u16) -> u16 {
    match color {
        vt100::Color::Default => default,
        vt100::Color::Idx(i @ 0..=15) => ANSI_12BIT[i as usize],
        vt100::Color::Idx(i @ 16..=231) => {
            let n = i - 16;
            let r_step = n / 36;
            let g_step = (n / 6) % 6;
            let b_step = n % 6;
            let to_4bit = |s: u8| -> u16 {
                match s {
                    0 => 0,
                    1 => 5,
                    2 => 8,
                    3 => 11,
                    4 => 13,
                    _ => 15,
                }
            };
            (to_4bit(r_step) << 8) | (to_4bit(g_step) << 4) | to_4bit(b_step)
        }
        vt100::Color::Idx(i) => {
            let v = (((i - 232) as u16 * 10 + 8) >> 4).min(15);
            (v << 8) | (v << 4) | v
        }
        vt100::Color::Rgb(r, g, b) => {
            (((r as u16) >> 4) << 8) | (((g as u16) >> 4) << 4) | ((b as u16) >> 4)
        }
    }
}

/// Extracts text from a `vt100::Screen` within a `SelectionRange`.
pub fn extract_text_from_screen(screen: &vt100::Screen, range: SelectionRange) -> String {
    let mut text = String::new();
    let (_, cols) = screen.size();
    for row in range.start_row..=range.end_row {
        let col_start = if row == range.start_row {
            range.start_col
        } else {
            0
        };
        let col_end = if row == range.end_row {
            range.end_col
        } else {
            cols.saturating_sub(1)
        };
        for col in col_start..=col_end {
            if let Some(cell) = screen.cell(row, col) {
                if !cell.is_wide_continuation() {
                    text.push_str(cell.contents());
                }
            }
        }
        if row < range.end_row {
            text.push('\n');
        }
    }
    text
}

impl OrpheusRenderer {
    /// Builds GPU cell instances from a `vt100::Screen`.
    pub fn build_cells(
        screen: &vt100::Screen,
        instances: &mut Vec<CellInstance>,
        options: CellBuildOptions,
    ) {
        let (rows, cols) = screen.size();
        let max_rows = rows.min(options.rows);
        let max_cols = cols.min(options.cols);

        for r in 0..max_rows {
            for c in 0..max_cols {
                if let Some(cell) = screen.cell(r, c) {
                    let contents = cell.contents();
                    let has_text = !contents.is_empty() && contents != " ";
                    let has_bg = cell.bgcolor() != vt100::Color::Default;
                    if has_text || has_bg {
                        let fg_rgb = vt100_color_to_rgb(cell.fgcolor(), 0xCCC);
                        let bg_rgb = vt100_color_to_rgb(cell.bgcolor(), 0x000);
                        let packed_colors = ((fg_rgb as u32) << 12) | (bg_rgb as u32 & 0xFFF);
                        let mut effects = 0u32;
                        if cell.underline() {
                            effects |= 1 << 13;
                        }
                        instances.push(CellInstance {
                            grid_position: [c as u32, r as u32],
                            glyph_id_and_effects: effects,
                            packed_colors,
                        });
                    }
                }
            }
        }

        if options.cursor_visible && !screen.hide_cursor() {
            let (cur_row, cur_col) = screen.cursor_position();
            if cur_row < options.rows && cur_col < options.cols {
                let mut effects = 0u32;
                if options.cursor_style == 2 {
                    effects |= 1 << 13;
                }
                instances.push(CellInstance {
                    grid_position: [cur_col as u32, cur_row as u32],
                    glyph_id_and_effects: effects,
                    packed_colors: 0xFFF << 12,
                });
            }
        }
    }
}
