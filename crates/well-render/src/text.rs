//! Font-backed terminal cells. Positions are converted from physical pixels to NDC.
use crate::fonts::{FontCatalog, FontSet};
use crate::{GlyphInstance, TerminalRenderer};
use ab_glyph::{point, Font, PxScale, ScaleFont};
use std::collections::HashMap;
use well_shell::pty::{Color, ScreenSnapshot};

const ATLAS: u32 = 2048;
#[cfg(test)]
const BG: [u8; 3] = [27, 35, 45];

#[derive(Clone, Copy, Debug)]
pub struct CellMetrics {
    pub width: u32,
    pub height: u32,
    pub padding: u32,
}
impl CellMetrics {
    pub fn grid(self, width: u32, height: u32) -> (u16, u16) {
        let rows = height.saturating_sub(self.padding * 2) / self.height.max(1);
        let cols = width.saturating_sub(self.padding * 2) / self.width.max(1);
        (
            rows.clamp(1, u16::MAX as u32) as u16,
            cols.clamp(1, u16::MAX as u32) as u16,
        )
    }
}

pub struct TextRenderer {
    renderer: TerminalRenderer,
    texture: wgpu::Texture,
    fonts: FontSet,
    appearance: well_config::Appearance,
    scale: PxScale,
    ascent: f32,
    pub metrics: CellMetrics,
    cache: HashMap<(String, u8), u32>,
    next_slot: u32,
    tile: [u32; 2],
    capacity: u32,
    srgb_surface: bool,
}
impl TextRenderer {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        scale_factor: f64,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::with_appearance(
            device,
            config,
            scale_factor,
            FontCatalog::default(),
            &well_config::Appearance::default(),
        )
    }
    pub fn with_appearance(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        scale_factor: f64,
        catalog: FontCatalog,
        appearance: &well_config::Appearance,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let fonts = FontSet::new(catalog, &appearance.font_family)?;
        let scale = PxScale::from(appearance.font_size * scale_factor as f32);
        let scaled = fonts.primary[0].as_scaled(scale);
        let metrics = CellMetrics {
            width: scaled.h_advance(scaled.glyph_id('M')).ceil().max(1.0) as u32,
            height: (scaled.height()
                + scaled.line_gap()
                + appearance.line_spacing * scale_factor as f32)
                .ceil()
                .max(1.0) as u32,
            padding: (appearance.padding as f64 * scale_factor).round() as u32,
        };
        let ascent = scaled.ascent();
        let tile = [metrics.width * 2 + 2, metrics.height + 2];
        if tile[0] > ATLAS || tile[1] > ATLAS {
            return Err("Font and display scale exceed the glyph atlas dimensions".into());
        }
        let capacity = (ATLAS / tile[0]) * (ATLAS / tile[1]);
        if capacity < 2 {
            return Err("Font and display scale leave no room for fallback glyphs".into());
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Terminal glyph atlas"),
            size: wgpu::Extent3d {
                width: ATLAS,
                height: ATLAS,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Glyph sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let renderer = TerminalRenderer::new(device, config, &view, &sampler);
        Ok(Self {
            renderer,
            texture,
            fonts,
            appearance: appearance.clone(),
            scale,
            ascent,
            metrics,
            cache: HashMap::new(),
            next_slot: 0,
            tile,
            capacity,
            srgb_surface: config.format.is_srgb(),
        })
    }
    pub fn background_color(&self) -> wgpu::Color {
        let c = surface_color(
            color(Color::Default, self.appearance.background),
            self.srgb_surface,
        );
        wgpu::Color {
            r: c[0] as f64,
            g: c[1] as f64,
            b: c[2] as f64,
            a: 1.0,
        }
    }
    fn glyph(&mut self, queue: &wgpu::Queue, text: &str, style: u8) -> u32 {
        let key = (text.to_owned(), style);
        if let Some(&slot) = self.cache.get(&key) {
            return slot;
        }
        if self.next_slot == self.capacity {
            // A frame must not overwrite glyphs already referenced by its instances.
            return self.glyph(queue, "\u{fffd}", 0);
        }
        let slot = self.next_slot;
        self.next_slot += 1;
        let mut pixels = vec![0u8; (self.tile[0] * self.tile[1]) as usize];
        for ch in text.chars() {
            if matches!(ch, '\u{fe0e}' | '\u{fe0f}' | '\u{200d}') {
                continue;
            }
            let font = self.fonts.font_for(ch, style);
            let scaled = font.as_scaled(self.scale);
            let id = scaled.glyph_id(ch);
            let id = if id.0 == 0 {
                scaled.glyph_id('\u{fffd}')
            } else {
                id
            };
            let glyph = id.with_scale_and_position(self.scale, point(1.0, 1.0 + self.ascent));
            if let Some(outline) = font.outline_glyph(glyph) {
                let bounds = outline.px_bounds();
                outline.draw(|x, y, coverage| {
                    let x = x as i32 + bounds.min.x as i32;
                    let y = y as i32 + bounds.min.y as i32;
                    if x >= 0 && y >= 0 && x < self.tile[0] as i32 && y < self.tile[1] as i32 {
                        let i = y as usize * self.tile[0] as usize + x as usize;
                        pixels[i] = pixels[i].max((coverage * 255.0) as u8);
                    }
                });
            }
        }
        let cols = ATLAS / self.tile[0];
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: (slot % cols) * self.tile[0],
                    y: (slot / cols) * self.tile[1],
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(self.tile[0]),
                rows_per_image: Some(self.tile[1]),
            },
            wgpu::Extent3d {
                width: self.tile[0],
                height: self.tile[1],
                depth_or_array_layers: 1,
            },
        );
        self.cache.insert(key, slot);
        slot
    }
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        viewport: [u32; 2],
        screen: &ScreenSnapshot,
    ) {
        self.draw_in(
            device,
            queue,
            encoder,
            view,
            viewport,
            [0, 0, viewport[0], viewport[1]],
            screen,
        );
    }
    pub fn draw_in(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        viewport: [u32; 2],
        region: [u32; 4],
        screen: &ScreenSnapshot,
    ) {
        self.draw_selected(device, queue, encoder, view, viewport, region, screen, None);
    }
    pub fn draw_selected(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        viewport: [u32; 2],
        region: [u32; 4],
        screen: &ScreenSnapshot,
        selection: Option<(usize, usize)>,
    ) {
        if self.next_slot == self.capacity {
            self.cache.clear();
            self.next_slot = 0;
        }
        // Pin these entries so empty backgrounds and unsupported glyphs always work.
        self.glyph(queue, "", 0);
        self.glyph(queue, "\u{fffd}", 0);
        let mut instances = Vec::with_capacity(screen.cells.len() * 2);
        for row in 0..screen.rows {
            for col in 0..screen.cols {
                let cell = &screen.cells[row as usize * screen.cols as usize + col as usize];
                let mut fg = color(cell.fgcolor(), self.appearance.foreground);
                let mut bg = color(cell.bgcolor(), self.appearance.background);
                if cell.inverse() {
                    std::mem::swap(&mut fg, &mut bg);
                }
                if screen.cursor == Some((row, col)) {
                    std::mem::swap(&mut fg, &mut bg);
                }
                let index = row as usize * screen.cols as usize + col as usize;
                if selection.is_some_and(|(start, end)| {
                    index >= start.saturating_sub(cell.is_wide() as usize) && index <= end
                }) {
                    bg = color(Color::Rgb(61, 65, 96), self.appearance.background);
                }
                fg = surface_color(fg, self.srgb_surface);
                bg = surface_color(bg, self.srgb_surface);
                let width = if cell.is_wide() {
                    self.metrics.width * 2
                } else {
                    self.metrics.width
                };
                let x = region[0] + self.metrics.padding + col as u32 * self.metrics.width;
                let y = region[1] + self.metrics.padding + row as u32 * self.metrics.height;
                if x + width > region[0] + region[2]
                    || y + self.metrics.height > region[1] + region[3]
                {
                    continue;
                }
                // A wide cell paints its continuation too; skip the latter's foreground/background.
                if cell.is_wide_continuation() {
                    continue;
                }
                let style = cell.bold() as u8 | ((cell.italic() as u8) << 1);
                let slot = self.glyph(queue, cell.contents(), style);
                let atlas_cols = ATLAS / self.tile[0];
                let ax = (slot % atlas_cols) * self.tile[0] + 1;
                let ay = (slot / atlas_cols) * self.tile[1] + 1;
                instances.push(instance(
                    [x, y, width, self.metrics.height],
                    viewport,
                    [ax as f32 / ATLAS as f32, ay as f32 / ATLAS as f32],
                    [
                        (ax + width) as f32 / ATLAS as f32,
                        (ay + self.metrics.height) as f32 / ATLAS as f32,
                    ],
                    fg,
                    bg,
                ));
                if cell.underline() {
                    let blank = self.cache[&(String::new(), 0)];
                    let uv = [
                        ((blank % atlas_cols) * self.tile[0]) as f32 / ATLAS as f32,
                        ((blank / atlas_cols) * self.tile[1]) as f32 / ATLAS as f32,
                    ];
                    instances.push(instance(
                        [x, y + self.metrics.height - 2, width, 1],
                        viewport,
                        uv,
                        uv,
                        fg,
                        fg,
                    ));
                }
            }
        }
        self.renderer.draw(device, encoder, view, queue, &instances);
    }
}
fn instance(
    rect: [u32; 4],
    viewport: [u32; 2],
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    fg: [f32; 4],
    bg: [f32; 4],
) -> GlyphInstance {
    GlyphInstance {
        position: [
            2.0 * rect[0] as f32 / viewport[0] as f32 - 1.0,
            1.0 - 2.0 * rect[1] as f32 / viewport[1] as f32,
        ],
        size: [
            2.0 * rect[2] as f32 / viewport[0] as f32,
            -2.0 * rect[3] as f32 / viewport[1] as f32,
        ],
        uv_min,
        uv_max,
        fg_color: fg,
        bg_color: bg,
    }
}
fn color(color: Color, default: [u8; 3]) -> [f32; 4] {
    const ANSI: [[u8; 3]; 16] = [
        [0, 0, 0],
        [205, 49, 49],
        [13, 188, 121],
        [229, 229, 16],
        [36, 114, 200],
        [188, 63, 188],
        [17, 168, 205],
        [229, 229, 229],
        [102, 102, 102],
        [241, 76, 76],
        [35, 209, 139],
        [245, 245, 67],
        [59, 142, 234],
        [214, 112, 214],
        [41, 184, 219],
        [255, 255, 255],
    ];
    let rgb = match color {
        Color::Default => default,
        Color::Rgb(r, g, b) => [r, g, b],
        Color::Idx(i @ 0..=15) => ANSI[i as usize],
        Color::Idx(i @ 16..=231) => {
            let n = i - 16;
            let level = |v| if v == 0 { 0 } else { 55 + v * 40 };
            [level(n / 36), level(n / 6 % 6), level(n % 6)]
        }
        Color::Idx(i) => [8 + (i - 232) * 10; 3],
    };
    [
        rgb[0] as f32 / 255.0,
        rgb[1] as f32 / 255.0,
        rgb[2] as f32 / 255.0,
        1.0,
    ]
}
fn surface_color(mut color: [f32; 4], srgb: bool) -> [f32; 4] {
    if srgb {
        for c in &mut color[..3] {
            *c = if *c <= 0.04045 {
                *c / 12.92
            } else {
                ((*c + 0.055) / 1.055).powf(2.4)
            };
        }
    }
    color
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grid_is_bounded_and_uses_padding() {
        let m = CellMetrics {
            width: 8,
            height: 16,
            padding: 8,
        };
        assert_eq!(m.grid(656, 400), (24, 80));
        assert_eq!(m.grid(0, 0), (1, 1));
    }
    #[test]
    fn indexed_palette_has_cube_and_greys() {
        assert_eq!(color(Color::Idx(196), BG), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(color(Color::Idx(255), BG)[0], 238.0 / 255.0);
    }
}
