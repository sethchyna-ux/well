// Orpheus – WebGPU dynamic glyph‑atlas pipeline

mod renderer;
mod text;
pub use text::{CellMetrics, TextRenderer};

pub use renderer::{GlyphInstance, TerminalRenderer};

pub mod prelude {
    pub use super::{GlyphInstance, TerminalRenderer};
}

/// Convenience wrapper to create a `TerminalRenderer`.
pub fn init_renderer(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    atlas_texture_view: &wgpu::TextureView,
    atlas_sampler: &wgpu::Sampler,
) -> TerminalRenderer {
    TerminalRenderer::new(device, config, atlas_texture_view, atlas_sampler)
}
mod fonts;
pub mod graphics_protocol;
pub mod orpheus;
pub use fonts::{
    canonical_font_family, embedded_opendyslexic_fonts, FontCatalog, EMBEDDED_OPENDYSLEXIC_BOLD,
    EMBEDDED_OPENDYSLEXIC_BOLD_ITALIC, EMBEDDED_OPENDYSLEXIC_ITALIC, EMBEDDED_OPENDYSLEXIC_MONO,
    EMBEDDED_OPENDYSLEXIC_M_REG, EMBEDDED_OPENDYSLEXIC_REG,
};
pub use orpheus::{
    extract_text_from_screen, vt100_color_to_rgb, CellBuildOptions, CellInstance, OrpheusRenderer,
    SelectionRange,
};
