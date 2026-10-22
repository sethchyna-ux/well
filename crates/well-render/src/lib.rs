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
pub mod orpheus;
pub use fonts::FontCatalog;
