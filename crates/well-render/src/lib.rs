// well-render-orpheus.rs
//
// ORPHEUS: The high-performance WebGPU-backed text-shaping and glyph-atlas rendering engine
// for "Well" (Phrear) terminal. Renders the entire terminal grid in a single instanced draw call.
//
// Naming Theme: Orpheus, the master of harmonious visual composition.
// Mechanism: Dynamic 2D Texture Atlas, instanced VBOs, and branchless WGSL pipelines.

pub mod compute;
pub mod graphics_protocol;
pub mod pipeline;

use font8x8::{UnicodeFonts, BASIC_FONTS, BLOCK_FONTS, BOX_FONTS};
use std::collections::HashMap;
use std::sync::Arc;

// packed vertex attribute for quad corners
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub tex_coord: [f32; 2],
}

// Packed per-cell instance data (64 bits / 8 bytes)
// Packs Glyph ID (16 bits) and 24-bit RGB colors for foreground and background.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CellInstance {
    pub grid_position: [u32; 2],   // grid_x, grid_y
    pub glyph_id_and_effects: u32, // Bits 0-12: Slot ID, Bit 13: Underline, Bit 14: Strikethrough, Bit 15: Emoji
    pub packed_colors: u32, // Bits 0-11: FG (12-bit color or full 24-bit packed split), Bits 12-23: BG
}

impl CellInstance {
    pub fn new(x: u32, y: u32, ch: char, fg_color: u32, bg_color: u32) -> Self {
        let slot = match ch {
            ' '..='~' => (ch as u32) - 32,
            '█' => 95,
            '❯' => 96,
            '•' => 97,
            '═' => 98,
            _ => 0,
        };
        let packed_colors = ((fg_color & 0xFFF) << 12) | (bg_color & 0xFFF);
        Self {
            grid_position: [x, y],
            glyph_id_and_effects: slot,
            packed_colors,
        }
    }
}

/// Represents an inclusive rectangular or linear range of terminal cells selected by the user.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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

    /// Normalizes the selection range so that `(c1, r1)` precedes or equals `(c2, r2)`.
    pub fn normalized(&self) -> ((u16, u16), (u16, u16)) {
        if (self.start_row, self.start_col) <= (self.end_row, self.end_col) {
            (
                (self.start_col, self.start_row),
                (self.end_col, self.end_row),
            )
        } else {
            (
                (self.end_col, self.end_row),
                (self.start_col, self.start_row),
            )
        }
    }

    /// Checks if a cell at (col, row) is enclosed within this selection range.
    pub fn contains(&self, col: u16, row: u16) -> bool {
        let ((c1, r1), (c2, r2)) = self.normalized();
        if row < r1 || row > r2 {
            return false;
        }
        if r1 == r2 {
            col >= c1 && col <= c2
        } else if row == r1 {
            col >= c1
        } else if row == r2 {
            col <= c2
        } else {
            true
        }
    }

    /// Returns true if the start and end anchors point to the exact same cell.
    pub fn is_empty(&self) -> bool {
        self.start_col == self.end_col && self.start_row == self.end_row
    }
}

/// Extracts text from a `vt100::Screen` within the specified `SelectionRange`.
///
/// Lines are trimmed of trailing blank padding and separated by newlines.
pub fn extract_text_from_screen(screen: &vt100::Screen, selection: SelectionRange) -> String {
    let ((c1, r1), (c2, r2)) = selection.normalized();
    let (_, screen_cols) = screen.size();
    let mut lines = Vec::new();

    for r in r1..=r2 {
        let start_c = if r == r1 { c1 } else { 0 };
        let end_c = if r == r2 {
            c2
        } else {
            screen_cols.saturating_sub(1)
        };
        let mut line = String::new();
        for c in start_c..=end_c {
            if let Some(cell) = screen.cell(r, c) {
                let s = cell.contents();
                if s.is_empty() {
                    line.push(' ');
                } else {
                    line.push_str(s);
                }
            } else {
                line.push(' ');
            }
        }
        lines.push(line.trim_end_matches(' ').to_string());
    }

    lines.join("\n")
}

/// Identifies word boundaries around `(col, row)` for double-click word selection.
pub fn find_word_bounds(screen: &vt100::Screen, col: u16, row: u16) -> Option<SelectionRange> {
    let (_, screen_cols) = screen.size();
    if col >= screen_cols {
        return None;
    }

    let cell_char = |c: u16| -> Option<char> {
        screen
            .cell(row, c)
            .and_then(|cell| cell.contents().chars().next())
    };

    let is_word_char = |c: u16| -> bool {
        match cell_char(c) {
            Some(ch) => !ch.is_whitespace(),
            None => false,
        }
    };

    if !is_word_char(col) {
        return None;
    }

    let mut start_c = col;
    while start_c > 0 && is_word_char(start_c - 1) {
        start_c -= 1;
    }

    let mut end_c = col;
    while end_c + 1 < screen_cols && is_word_char(end_c + 1) {
        end_c += 1;
    }

    Some(SelectionRange::new(start_c, row, end_c, row))
}

// Glyph Atlas slot categories based on "Well" specification
pub const SLOT_ASCII_NORMAL_START: u32 = 0;
pub const SLOT_ASCII_NORMAL_END: u32 = 94; // 95 pre-allocated standard ASCII slots
pub const SLOT_ASCII_STYLIZED_START: u32 = 95;
pub const SLOT_ASCII_STYLIZED_END: u32 = 2047; // 1953 LRU-managed stylized slots (Bold, Italic, BoldItalic)
pub const SLOT_DOUBLE_WIDE_START: u32 = 2048;
pub const SLOT_DOUBLE_WIDE_END: u32 = 6143; // 4096 slots for CJK and wide scripts
pub const SLOT_EMOJI_START: u32 = 6144;
pub const SLOT_EMOJI_END: u32 = 8191; // 2048 high-resolution multi-colored emoji slots

// Cache entry representing mapped glyph coordinates in the 2D texture array
#[derive(Copy, Clone, Debug)]
pub struct AtlasCoordinates {
    pub layer: u32,
    pub col: u32, // x position in the 1x32 horizontal grid
    pub row: u32, // y position in the layer
}

// LRU Node for tracking active glyph slots
pub struct LruNode {
    pub key: (u32, u32), // (Unicode codepoint, Style bits)
    pub slot_id: u32,
}

pub struct OrpheusRenderer {
    // WebGPU Context Handles
    pub scrollbar_pipeline: Option<scrollbar::ScrollbarDensityPipeline>,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub render_pipeline: wgpu::RenderPipeline,

    // Buffers
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub instance_buffer: wgpu::Buffer,
    pub uniform_buffer: wgpu::Buffer,

    // Bind Groups
    pub bind_group: wgpu::BindGroup,

    // Texture Atlas
    pub atlas_texture: wgpu::Texture,
    pub atlas_view: wgpu::TextureView,
    pub atlas_sampler: wgpu::Sampler,

    // Layout and Sizing
    pub grid_rows: u32,
    pub grid_cols: u32,
    pub cell_width: f32,
    pub cell_height: f32,
    pub offset_x: f32,
    pub offset_y: f32,

    // Cache management
    pub glyph_cache: HashMap<(u32, u32), AtlasCoordinates>, // (Codepoint, Style) -> Coordinates
    pub lru_cache: Vec<LruNode>, // Tracks active dynamic slots for eviction
    pub next_stylized_slot: u32,
    pub next_double_wide_slot: u32,
    pub next_emoji_slot: u32,
    pub font: Option<fontdue::Font>,
    pub compute_engine: Option<crate::compute::HephaestusEngine>,
}

impl OrpheusRenderer {
    pub async fn new(
        instance: &wgpu::Instance,
        surface: &wgpu::Surface<'_>,
        surface_format: wgpu::TextureFormat,
        grid_rows: u32,
        grid_cols: u32,
        cell_width: f32,
        cell_height: f32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or("Failed to find WebGPU adapter")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("WellOrpheusDevice"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        // 1. Create a 2D Texture Array for the Glyph Atlas (layer-based texture format)
        // Array layers contain 1x32 grids of glyphs for cache-coherent GPU texture lookups.
        let atlas_width = cell_width as u32 * 32;
        let atlas_height = cell_height as u32;
        let atlas_layers = 256; // 256 layers to hold all categories (ASCII, CJK, Emojis)

        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("OrpheusAtlasTextureArray"),
            size: wgpu::Extent3d {
                width: atlas_width,
                height: atlas_height,
                depth_or_array_layers: atlas_layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("OrpheusAtlasTextureView"),
            format: Some(wgpu::TextureFormat::Rgba8Unorm),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: Some(atlas_layers),
        });

        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("OrpheusAtlasSampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest, // Keep text boundaries crisp
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // 2. Uniform buffers for projection matrix and viewport sizes
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("OrpheusUniformBuffer"),
            size: 96, // Mat4x4 (64) + Cell/Grid (16) + Viewport Offsets/Pad (16)
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 3. Compile WGSL Shader Core
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("OrpheusShadingWGSL"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(WGSL_SHADER_SOURCE)),
        });

        // 4. Create Pipeline Layouts
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("OrpheusBindGroupLayout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("OrpheusBindGroup"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("OrpheusPipelineLayout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("OrpheusRenderPipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[
                    // Quad Vertex Attributes
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 0,
                                shader_location: 0,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 8,
                                shader_location: 1,
                            },
                        ],
                    },
                    // Instanced Cell Attributes
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<CellInstance>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Uint32x2,
                                offset: 0,
                                shader_location: 2, // grid_position
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Uint32,
                                offset: 8,
                                shader_location: 3, // glyph_id_and_effects
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Uint32,
                                offset: 12,
                                shader_location: 4, // packed_colors
                            },
                        ],
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Static quad vertices
        let vertices = [
            Vertex {
                position: [0.0, 0.0],
                tex_coord: [0.0, 0.0],
            },
            Vertex {
                position: [1.0, 0.0],
                tex_coord: [1.0, 0.0],
            },
            Vertex {
                position: [1.0, 1.0],
                tex_coord: [1.0, 1.0],
            },
            Vertex {
                position: [0.0, 1.0],
                tex_coord: [0.0, 1.0],
            },
        ];

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("OrpheusQuadVertexBuffer"),
            size: std::mem::size_of_val(&vertices) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&vertices));

        let indices: [u16; 6] = [0, 1, 2, 2, 3, 0];
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("OrpheusQuadIndexBuffer"),
            size: std::mem::size_of_val(&indices) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&index_buffer, 0, bytemuck::cast_slice(&indices));

        // Flat cell buffer allocation with ample headroom for full grid + cursor
        let instance_buffer_size = ((grid_rows * grid_cols + 512).max(16384)
            * std::mem::size_of::<CellInstance>() as u32) as u64;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("OrpheusInstanceBuffer"),
            size: instance_buffer_size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut renderer = Self {
            device: Arc::clone(&device),
            queue: Arc::clone(&queue),
            scrollbar_pipeline: Some(scrollbar::ScrollbarDensityPipeline::new(
                &device,
                surface_format,
            )),
            render_pipeline,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            uniform_buffer,
            bind_group,
            atlas_texture,
            atlas_view,
            atlas_sampler,
            grid_rows,
            grid_cols,
            cell_width,
            cell_height,
            offset_x: 0.0,
            offset_y: 0.0,
            glyph_cache: HashMap::new(),
            lru_cache: Vec::new(),
            next_stylized_slot: SLOT_ASCII_STYLIZED_START,
            next_double_wide_slot: SLOT_DOUBLE_WIDE_START,
            next_emoji_slot: SLOT_EMOJI_START,
            font: {
                let font_bytes =
                    include_bytes!("../../../assets/fonts/OpenDyslexicNerdFont-Regular.otf");
                fontdue::Font::from_bytes(&font_bytes[..], fontdue::FontSettings::default()).ok()
            },
            compute_engine: Some(crate::compute::HephaestusEngine::new(
                device.clone(),
                queue.clone(),
            )),
        };

        // Pre-rasterize all standard ASCII characters (32 to 126) + cursor block '█' into atlas
        for ch_code in 32u32..=126u32 {
            renderer.resolve_or_allocate_glyph(ch_code, 0, false);
        }
        renderer.resolve_or_allocate_glyph('█' as u32, 0, false);
        renderer.resolve_or_allocate_glyph('❯' as u32, 0, false);
        renderer.resolve_or_allocate_glyph('•' as u32, 0, false);
        renderer.resolve_or_allocate_glyph('═' as u32, 0, false);

        Ok(renderer)
    }

    // Resolves coordinates from the Flat Atlas Cache, or registers/evicts on demand
    pub fn resolve_or_allocate_glyph(
        &mut self,
        codepoint: u32,
        style: u32,
        is_emoji: bool,
    ) -> AtlasCoordinates {
        let key = (codepoint, style);

        if let Some(&coords) = self.glyph_cache.get(&key) {
            self.refresh_lru(key);
            return coords;
        }

        // Slot allocation with category boundary protections
        let slot_id = if is_emoji {
            let slot = self.next_emoji_slot;
            self.next_emoji_slot += 1;
            if self.next_emoji_slot > SLOT_EMOJI_END {
                self.next_emoji_slot = SLOT_EMOJI_START; // Ring buffer eviction fallback
            }
            slot
        } else if codepoint >= 32 && codepoint <= 126 && style == 0 {
            // Pre-allocated fixed fast-path ASCII normal slots (0 to 94)
            codepoint - 32
        } else if codepoint == '█' as u32 {
            95
        } else if codepoint == '❯' as u32 {
            96
        } else if codepoint == '•' as u32 {
            97
        } else if codepoint == '═' as u32 {
            98
        } else if codepoint <= 127 {
            let slot = self.next_stylized_slot;
            self.next_stylized_slot += 1;
            if self.next_stylized_slot > SLOT_ASCII_STYLIZED_END {
                self.evict_lru_and_reallocate(SLOT_ASCII_STYLIZED_START, SLOT_ASCII_STYLIZED_END)
            } else {
                slot
            }
        } else {
            let slot = self.next_double_wide_slot;
            self.next_double_wide_slot += 1;
            if self.next_double_wide_slot > SLOT_DOUBLE_WIDE_END {
                self.evict_lru_and_reallocate(SLOT_DOUBLE_WIDE_START, SLOT_DOUBLE_WIDE_END)
            } else {
                slot
            }
        };

        // Compute 2D texture array coordinates
        let layer = slot_id / 32;
        let position = slot_id % 32;

        let coords = AtlasCoordinates {
            layer,
            col: position,
            row: 0,
        };

        // Insert into caches
        self.glyph_cache.insert(key, coords);
        self.lru_cache.push(LruNode { key, slot_id });

        // Trigger on-demand glyph rasterization via host queue writes
        self.rasterize_glyph_to_gpu(codepoint, style, coords);

        coords
    }

    fn refresh_lru(&mut self, key: (u32, u32)) {
        if let Some(pos) = self.lru_cache.iter().position(|x| x.key == key) {
            let node = self.lru_cache.remove(pos);
            self.lru_cache.push(node);
        }
    }

    fn evict_lru_and_reallocate(&mut self, min: u32, max: u32) -> u32 {
        // Evicts the least recently used node that matches the slot boundary ranges
        if let Some(pos) = self
            .lru_cache
            .iter()
            .position(|node| node.slot_id >= min && node.slot_id <= max)
        {
            let node = self.lru_cache.remove(pos);
            self.glyph_cache.remove(&node.key);
            node.slot_id
        } else {
            min // Absolute fallback
        }
    }

    fn rasterize_glyph_to_gpu(&self, codepoint: u32, _style: u32, coords: AtlasCoordinates) {
        let w = self.cell_width as u32;
        let h = self.cell_height as u32;
        let mut pixels = vec![0u8; (w * h * 4) as usize];

        let ch = char::from_u32(codepoint).unwrap_or('?');

        if ch == '█' || codepoint == 0x2588 {
            for y in 0..h {
                for x in 0..w {
                    let idx = ((y * w + x) * 4) as usize;
                    pixels[idx] = 255;
                    pixels[idx + 1] = 255;
                    pixels[idx + 2] = 255;
                    pixels[idx + 3] = 255;
                }
            }
        } else {
            let mut rasterized = false;

            if let Some(font) = &self.font {
                // OpenDyslexic Nerd Font rasterization with antialiased coverage
                let font_size = (h as f32 * 0.72).max(12.0);
                let (metrics, bitmap) = font.rasterize(ch, font_size);

                if metrics.width > 0 && metrics.height > 0 && !bitmap.is_empty() {
                    let x_offset = ((w as i32 - metrics.width as i32) / 2).max(0) as u32;
                    let baseline = (h as f32 * 0.76) as i32;
                    let y_offset = (baseline - metrics.height as i32 - metrics.ymin).max(0) as u32;

                    for by in 0..metrics.height {
                        for bx in 0..metrics.width {
                            let px = x_offset + bx as u32;
                            let py = y_offset + by as u32;
                            if px < w && py < h {
                                let coverage = bitmap[by * metrics.width + bx];
                                if coverage > 0 {
                                    let idx = ((py * w + px) * 4) as usize;
                                    pixels[idx] = 255;
                                    pixels[idx + 1] = 255;
                                    pixels[idx + 2] = 255;
                                    pixels[idx + 3] = coverage;
                                }
                            }
                        }
                    }
                    rasterized = true;
                }
            }

            if !rasterized {
                if let Some(glyph) = BASIC_FONTS
                    .get(ch)
                    .or_else(|| BLOCK_FONTS.get(ch))
                    .or_else(|| BOX_FONTS.get(ch))
                {
                    for font_y in 0..8u32 {
                        let row_byte = glyph[font_y as usize];
                        for font_x in 0..8u32 {
                            if (row_byte >> font_x) & 1 == 1 {
                                let start_x = (font_x * w) / 8;
                                let end_x = ((font_x + 1) * w) / 8;
                                let start_y = (font_y * h) / 8;
                                let end_y = ((font_y + 1) * h) / 8;

                                for py in start_y..end_y {
                                    for px in start_x..end_x {
                                        if px < w && py < h {
                                            let idx = ((py * w + px) * 4) as usize;
                                            pixels[idx] = 255;
                                            pixels[idx + 1] = 255;
                                            pixels[idx + 2] = 255;
                                            pixels[idx + 3] = 255;
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if ch == '❯' || ch == '>' {
                    for y in 0..h {
                        let mid = h / 2;
                        let dist = if y < mid { y } else { h - 1 - y };
                        let target_x = (dist * (w.saturating_sub(2))) / (mid.max(1)) + 1;
                        for px in target_x.saturating_sub(1)..=(target_x + 1) {
                            if px < w {
                                let idx = ((y * w + px) * 4) as usize;
                                pixels[idx] = 255;
                                pixels[idx + 1] = 255;
                                pixels[idx + 2] = 255;
                                pixels[idx + 3] = 255;
                            }
                        }
                    }
                } else if ch == '═' {
                    let y1 = h / 3;
                    let y2 = (2 * h) / 3;
                    for x in 0..w {
                        let idx1 = ((y1 * w + x) * 4) as usize;
                        pixels[idx1] = 255;
                        pixels[idx1 + 1] = 255;
                        pixels[idx1 + 2] = 255;
                        pixels[idx1 + 3] = 255;

                        let idx2 = ((y2 * w + x) * 4) as usize;
                        pixels[idx2] = 255;
                        pixels[idx2 + 1] = 255;
                        pixels[idx2 + 2] = 255;
                        pixels[idx2 + 3] = 255;
                    }
                } else if ch == '•' {
                    let mid_x = w / 2;
                    let mid_y = h / 2;
                    for y in (mid_y.saturating_sub(2))..=(mid_y + 2).min(h - 1) {
                        for x in (mid_x.saturating_sub(2))..=(mid_x + 2).min(w - 1) {
                            let idx = ((y * w + x) * 4) as usize;
                            pixels[idx] = 255;
                            pixels[idx + 1] = 255;
                            pixels[idx + 2] = 255;
                            pixels[idx + 3] = 255;
                        }
                    }
                }
            }
        }

        // Upload to specific layer of our 2D Texture Array
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: coords.col * w,
                    y: coords.row * h,
                    z: coords.layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn ensure_instance_capacity(&mut self, required_cells: usize) {
        let current_capacity =
            (self.instance_buffer.size() / std::mem::size_of::<CellInstance>() as u64) as usize;
        if required_cells > current_capacity {
            let new_capacity = (required_cells + 4096).next_power_of_two();
            let new_size = (new_capacity * std::mem::size_of::<CellInstance>()) as u64;
            self.instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("OrpheusInstanceBufferResized"),
                size: new_size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
    }

    pub fn set_dimensions(&mut self, width: f32, height: f32, cols: u32, rows: u32) {
        self.grid_cols = cols;
        self.grid_rows = rows;
        self.ensure_instance_capacity((cols * rows) as usize);
        self.update_projection_matrix(width, height);
    }

    pub fn set_offsets(
        &mut self,
        offset_x: f32,
        offset_y: f32,
        window_width: f32,
        window_height: f32,
    ) {
        self.offset_x = offset_x;
        self.offset_y = offset_y;
        self.update_projection_matrix(window_width, window_height);
    }

    pub fn update_projection_matrix(&self, width: f32, height: f32) {
        // Compute and write standard orthographic screen projections to our uniform buffer
        let w = width.max(1.0);
        let h = height.max(1.0);
        let proj = [
            2.0 / w,
            0.0,
            0.0,
            0.0,
            0.0,
            -2.0 / h,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            -1.0,
            1.0,
            0.0,
            1.0,
        ];

        let mut uniform_data = vec![0u8; 96];
        uniform_data[0..64].copy_from_slice(bytemuck::cast_slice(&proj));
        uniform_data[64..68].copy_from_slice(&self.cell_width.to_ne_bytes());
        uniform_data[68..72].copy_from_slice(&self.cell_height.to_ne_bytes());
        uniform_data[72..76].copy_from_slice(&(self.grid_cols).to_ne_bytes());
        uniform_data[76..80].copy_from_slice(&(self.grid_rows).to_ne_bytes());
        uniform_data[80..84].copy_from_slice(&self.offset_x.to_ne_bytes());
        uniform_data[84..88].copy_from_slice(&self.offset_y.to_ne_bytes());

        self.queue
            .write_buffer(&self.uniform_buffer, 0, &uniform_data);
    }

    // Single Draw-Call Renderer Execution
    pub fn draw_frame(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        cells: &[CellInstance],
        clear_color: Option<wgpu::Color>,
    ) {
        if !cells.is_empty() {
            let max_cells =
                (self.instance_buffer.size() / std::mem::size_of::<CellInstance>() as u64) as usize;
            let upload_cells = if cells.len() > max_cells {
                &cells[..max_cells]
            } else {
                cells
            };
            self.queue
                .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(upload_cells));
        }

        let default_clear = wgpu::Color {
            r: 0.02,
            g: 0.02,
            b: 0.04,
            a: 1.0,
        };
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("OrpheusRenderPass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color.unwrap_or(default_clear)),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if !cells.is_empty() {
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

            // Single draw call for the entire cell matrix
            render_pass.draw_indexed(0..6, 0, 0..(cells.len() as u32));
            if let Some(scrollbar) = &self.scrollbar_pipeline {
                scrollbar.record_pass(&mut render_pass);
            }
        }
    }

    /// Helper to convert a string slice into a row of CellInstances
    pub fn append_text_line(
        cells: &mut Vec<CellInstance>,
        line: &str,
        row: u32,
        col_offset: u32,
        fg_color: u32,
        bg_color: u32,
    ) {
        for (i, ch) in line.chars().enumerate() {
            cells.push(CellInstance::new(
                col_offset + i as u32,
                row,
                ch,
                fg_color,
                bg_color,
            ));
        }
    }

    /// Converts a vt100 virtual terminal screen snapshot into instanced GPU quads
    /// Update cell metrics and projection uniform buffer when font sizing or window sizing changes
    pub fn update_cell_metrics(
        &mut self,
        cell_width: f32,
        cell_height: f32,
        grid_cols: u32,
        grid_rows: u32,
        window_width: f32,
        window_height: f32,
    ) {
        self.cell_width = cell_width;
        self.cell_height = cell_height;
        self.grid_cols = grid_cols;
        self.grid_rows = grid_rows;
        self.update_projection_matrix(window_width, window_height);
    }

    /// Converts a vt100 virtual terminal screen snapshot into instanced GPU quads with default styling
    pub fn build_cells_from_vt100(
        &mut self,
        screen: &vt100::Screen,
        cells: &mut Vec<CellInstance>,
        max_rows: u32,
        max_cols: u32,
    ) {
        self.build_cells_from_vt100_ext(
            screen, cells, max_rows, max_cols, true, // cursor visible
            0,    // block
            0,    // cyber-neon
            0,    // default cursor color
            None, // no selection
        );
    }

    /// Converts a vt100 virtual terminal screen snapshot into instanced GPU quads with live cursor, theme & selection controls
    pub fn build_cells_from_vt100_ext(
        &mut self,
        screen: &vt100::Screen,
        cells: &mut Vec<CellInstance>,
        max_rows: u32,
        max_cols: u32,
        cursor_visible: bool,
        cursor_style: u32, // 0 = Block, 1 = Beam, 2 = Underline
        theme_id: u32,
        cursor_color_override: u32,
        selection: Option<SelectionRange>,
    ) {
        Self::build_cells_ext(
            screen,
            cells,
            max_rows,
            max_cols,
            cursor_visible,
            cursor_style,
            theme_id,
            cursor_color_override,
            selection,
        );
    }

    /// Pure function converting vt100 virtual terminal screen snapshot into instanced GPU quads
    pub fn build_cells(
        screen: &vt100::Screen,
        cells: &mut Vec<CellInstance>,
        max_rows: u32,
        max_cols: u32,
        cursor_visible: bool,
        cursor_style: u32, // 0 = Block, 1 = Beam, 2 = Underline
        theme_id: u32,
        cursor_color_override: u32,
    ) {
        Self::build_cells_ext(
            screen,
            cells,
            max_rows,
            max_cols,
            cursor_visible,
            cursor_style,
            theme_id,
            cursor_color_override,
            None,
        );
    }

    /// Pure function converting vt100 virtual terminal screen snapshot into instanced GPU quads with selection support
    pub fn build_cells_ext(
        screen: &vt100::Screen,
        cells: &mut Vec<CellInstance>,
        max_rows: u32,
        max_cols: u32,
        cursor_visible: bool,
        cursor_style: u32, // 0 = Block, 1 = Beam, 2 = Underline
        theme_id: u32,
        cursor_color_override: u32,
        selection: Option<SelectionRange>,
    ) {
        cells.clear();

        let (screen_rows, screen_cols) = screen.size();
        let rows = (screen_rows as u32).min(max_rows);
        let cols = (screen_cols as u32).min(max_cols);

        let (default_fg, cursor_color) = match theme_id {
            1 => (0xCEF, 0x7AF), // Tokyo Night: cool lavender text, sky-blue cursor
            2 => (0x0F4, 0x3F5), // Matrix Green: high-voltage matrix green
            3 => (0xFE2, 0xF08), // Synthwave '84: neon amber text, neon pink cursor
            _ => (0x3F1, 0x3BF), // Cyber-Neon: neon green text, electric light blue cursor
        };
        let cursor_color = if cursor_color_override != 0 {
            cursor_color_override
        } else {
            cursor_color
        };
        let default_bg = 0x000; // Deep obsidian black (#050508)

        for row in 0..rows {
            for col in 0..cols {
                if let Some(cell) = screen.cell(row as u16, col as u16) {
                    let text = cell.contents();
                    let ch = text.chars().next().unwrap_or(' ');
                    let is_selected =
                        selection.map_or(false, |s| s.contains(col as u16, row as u16));

                    if ch == ' ' && cell.bgcolor() == vt100::Color::Default && !is_selected {
                        continue;
                    }

                    let is_default_fg = cell.fgcolor() == vt100::Color::Default;

                    let mut fg = if is_default_fg {
                        // Semantic theme mapping when no explicit ANSI color is forced
                        match theme_id {
                            1 => {
                                // Tokyo Night palette
                                if cell.bold() {
                                    0xBB9 // Bright lavender
                                } else if cell.italic() {
                                    0x7AF // Sky blue
                                } else {
                                    match ch {
                                        'A'..='Z' => 0x9AB, // Light slate blue
                                        '0'..='9' => 0xF97, // Orange
                                        '%' | '$' | '>' | '❯' | '#' | 'λ' | '➜' => 0xBB9,
                                        '/' | '.' | '_' | '-' | ':' | '@' | '~' | '=' | '+'
                                        | '*' | '&' | '|' => 0x569,
                                        _ => default_fg,
                                    }
                                }
                            }
                            2 => {
                                // Matrix Green palette
                                if cell.bold() {
                                    0x5F8 // Ultra bright lime
                                } else if cell.italic() {
                                    0x0C3 // Dim green
                                } else {
                                    default_fg
                                }
                            }
                            3 => {
                                // Synthwave '84 palette
                                if cell.bold() {
                                    0x0FF // Bright cyan
                                } else if cell.italic() {
                                    0xF08 // Neon magenta
                                } else {
                                    match ch {
                                        '0'..='9' => 0x0FF,                               // Cyan
                                        '%' | '$' | '>' | '❯' | '#' | 'λ' | '➜' => 0xF08, // Pink
                                        _ => default_fg,
                                    }
                                }
                            }
                            _ => {
                                // Cyber-Neon palette
                                if cell.bold() {
                                    if ch.is_ascii_uppercase() {
                                        0x235 // Bold uppercase: Slate-Navy (#223355)
                                    } else {
                                        0xF08 // Bold text: Vibrant Neon Magenta (#FF007F)
                                    }
                                } else if cell.italic() {
                                    if ch.is_ascii_uppercase() {
                                        0x124 // Italic uppercase: Deep Dark Slate (#112244)
                                    } else {
                                        0xA5F // Italic text: Deep Neon Purple (#A855F7)
                                    }
                                } else {
                                    match ch {
                                        'A'..='Z' => 0x124, // Caps / Uppercase: Deep Dark Slate-Cobalt (#112244)
                                        '0'..='9' => 0xA5F, // Numbers: Deep Neon Purple (#A855F7)
                                        '%' | '$' | '>' | '❯' | '#' | 'λ' | '➜' => 0xF08, // Prompt symbols: Electric Neon Magenta (#FF007F)
                                        '/' | '.' | '_' | '-' | ':' | '@' | '~' | '=' | '+'
                                        | '*' | '&' | '|' | '!' | '?' | ',' | ';' | '(' | ')'
                                        | '[' | ']' | '{' | '}' => 0x89A, // Punctuation & path separators: Slate Grey (#88929A)
                                        'a'..='z' => 0x3F1, // Normal lowercase: High-Voltage Neon Green (#39FF14)
                                        _ => 0x3BF, // Special symbols: Electric Light Blue (#38BDF8)
                                    }
                                }
                            }
                        }
                    } else {
                        // Explicit ANSI color from program (e.g. ls, git, nvim)
                        let base_fg = vt100_color_to_rgb(cell.fgcolor(), default_fg);
                        if cell.bold() {
                            // Bold ANSI highlights
                            match cell.fgcolor() {
                                vt100::Color::Idx(1) => 0xF29, // Bright Electric Magenta
                                vt100::Color::Idx(2) => 0x0F4, // Bright Neon Green
                                vt100::Color::Idx(4) => 0x6DF, // Bright Ice Light Blue
                                vt100::Color::Idx(5) => 0xC8F, // Bright Neon Purple
                                _ => 0xF08, // Default bold color: Vibrant Neon Magenta
                            }
                        } else {
                            base_fg
                        }
                    };

                    let mut bg = vt100_color_to_rgb(cell.bgcolor(), default_bg);

                    if cell.inverse() {
                        std::mem::swap(&mut fg, &mut bg);
                    }

                    if is_selected {
                        bg = 0x26E; // Vivid Royal / Electric Blue selection highlight (RGB: 34, 102, 238)
                        fg = 0xFFF; // High-contrast crisp white text
                    }

                    let mut instance = CellInstance::new(col, row, ch, fg, bg);

                    let mut effects = 0u32;
                    if cell.underline() {
                        effects |= 1;
                    }
                    if cell.bold() {
                        effects |= 2;
                    }

                    instance.glyph_id_and_effects |= effects << 13;
                    cells.push(instance);
                }
            }
        }

        // Draw cursor if not hidden by application, current blink state is visible,
        // and viewport is at bottom (screen.scrollback() == 0).
        if !screen.hide_cursor() && cursor_visible && screen.scrollback() == 0 {
            let (cursor_row, cursor_col) = screen.cursor_position();
            let cr = cursor_row as u32;
            let cc = cursor_col as u32;
            if cr < max_rows && cc < max_cols {
                let cell_under = screen.cell(cursor_row, cursor_col);
                let char_under = cell_under
                    .and_then(|c| c.contents().chars().next())
                    .unwrap_or(' ');

                match cursor_style {
                    1 => {
                        // Beam (|) cursor: draw vertical bar
                        cells.push(CellInstance::new(cc, cr as u32, '│', cursor_color, 0x000));
                    }
                    2 => {
                        // Underline (_) cursor: draw character with underline or '_'
                        let ch = if char_under == ' ' { '_' } else { char_under };
                        let mut inst = CellInstance::new(cc, cr as u32, ch, cursor_color, 0x000);
                        inst.glyph_id_and_effects |= 1 << 13; // Underline bit
                        cells.push(inst);
                    }
                    _ => {
                        // Block (█) cursor:
                        if char_under != ' ' {
                            // Invert character: black text over bright cursor block for complete readability!
                            cells.push(CellInstance::new(
                                cc,
                                cr as u32,
                                char_under,
                                0x000,
                                cursor_color,
                            ));
                        } else {
                            // Solid block
                            cells.push(CellInstance::new(cc, cr as u32, '█', cursor_color, 0x000));
                        }
                    }
                }
            }
        }
    }
}

/// Converts a vt100::Color attribute into 12-bit packed RGB with the Cyber-Neon palette
pub fn vt100_color_to_rgb(color: vt100::Color, default_color: u32) -> u32 {
    match color {
        vt100::Color::Default => default_color,
        vt100::Color::Rgb(r, g, b) => {
            let r4 = (r as u32 >> 4) & 0xF;
            let g4 = (g as u32 >> 4) & 0xF;
            let b4 = (b as u32 >> 4) & 0xF;
            (r4 << 8) | (g4 << 4) | b4
        }
        vt100::Color::Idx(idx) => {
            // Cyber-Neon Palette: Neon Green, Grey, Black, Magenta, Light Blue, Purple
            const ANSI_COLORS: [u32; 16] = [
                0x000, // 0: Obsidian Black (#050508)
                0xF08, // 1: Neon Magenta (#FF007F)
                0x3F1, // 2: Neon Green (#39FF14)
                0xFC1, // 3: Neon Amber/Yellow (#FACC15)
                0x3BF, // 4: Electric Light Blue (#38BDF8)
                0xA5F, // 5: Neon Purple (#A855F7)
                0x0FF, // 6: Neon Cyan (#00F0FF)
                0x89A, // 7: Slate Grey (#88929A)
                0x456, // 8: Dark Slate Grey (#475569)
                0xF29, // 9: Bright Electric Magenta (#FF3399)
                0x0F4, // 10: Bright High-Voltage Neon Green (#00FF66)
                0xFF4, // 11: Bright Yellow (#FFFF44)
                0x6DF, // 12: Bright Ice Light Blue (#67E8F9)
                0xC8F, // 13: Bright Neon Purple (#C084FC)
                0x4FF, // 14: Bright Neon Cyan (#38E8FF)
                0xFFA, // 15: Crisp White-Grey (#F1F5F9)
            ];
            if (idx as usize) < ANSI_COLORS.len() {
                ANSI_COLORS[idx as usize]
            } else if idx >= 16 && idx <= 231 {
                let c = idx - 16;
                let r = (c / 36) * 3;
                let g = ((c % 36) / 6) * 3;
                let b = (c % 6) * 3;
                ((r as u32) << 8) | ((g as u32) << 4) | (b as u32)
            } else if idx >= 232 {
                let gray = ((idx - 232) * 15) / 23;
                let g4 = gray as u32;
                (g4 << 8) | (g4 << 4) | g4
            } else {
                default_color
            }
        }
    }
}

// ── WGSL Shading Pipeline ──────────────────────────────────────────────
pub const WGSL_SHADER_SOURCE: &str = r#"
struct Uniforms {
    projection_matrix: mat4x4<f32>,
    cell_width: f32,
    cell_height: f32,
    grid_cols: u32,
    grid_rows: u32,
    offset_x: f32,
    offset_y: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var atlas_textures: texture_2d_array<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coord: vec2<f32>,
};

struct InstanceInput {
    @location(2) grid_pos: vec2<u32>,
    @location(3) glyph_id_and_effects: u32,
    @location(4) packed_colors: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec3<f32>,
    @location(1) @interpolate(flat) fg_color: vec4<f32>,
    @location(2) @interpolate(flat) bg_color: vec4<f32>,
    @location(3) @interpolate(flat) effects: u32,
};

fn unpack_rgba(packed: u32) -> vec4<f32> {
    let r = f32((packed >> 16u) & 0xFFu) / 255.0;
    let g = f32((packed >> 8u) & 0xFFu) / 255.0;
    let b = f32(packed & 0xFFu) / 255.0;
    return vec4<f32>(r, g, b, 1.0);
}

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    // 1. Unpack colors from instance attributes
    let fg_packed = instance.packed_colors >> 12u;
    let bg_packed = instance.packed_colors & 0xFFFu; // Packed split-matrix colors

    // Scale standard 12-bit colors to full 24-bit floats
    let r_fg = f32((fg_packed >> 8u) & 0xFu) / 15.0;
    let g_fg = f32((fg_packed >> 4u) & 0xFu) / 15.0;
    let b_fg = f32(fg_packed & 0xFu) / 15.0;
    out.fg_color = vec4<f32>(r_fg, g_fg, b_fg, 1.0);

    let r_bg = f32((bg_packed >> 8u) & 0xFu) / 15.0;
    let g_bg = f32((bg_packed >> 4u) & 0xFu) / 15.0;
    let b_bg = f32(bg_packed & 0xFu) / 15.0;
    // Default background (0x000) is transparent; custom backgrounds have alpha 1.0
    let bg_alpha = select(0.0, 1.0, bg_packed != 0u);
    out.bg_color = vec4<f32>(r_bg, g_bg, b_bg, bg_alpha);

    // 2. Decode glyph slot allocations
    let slot_id = instance.glyph_id_and_effects & 0x1FFFu; // 13 bits (Max 8192 slots)
    let layer = f32(slot_id / 32u);
    let col = f32(slot_id % 32u);

    // Map corner texture coordinates into the layer subgrid offsets
    let u = (col + vertex.tex_coord.x) / 32.0;
    let v = vertex.tex_coord.y; // Symmetrical mapping
    out.tex_coord = vec3<f32>(u, v, layer);

    // 3. Compute orthographic window transformation
    let cell_x = uniforms.offset_x + f32(instance.grid_pos.x) * uniforms.cell_width;
    let cell_y = uniforms.offset_y + f32(instance.grid_pos.y) * uniforms.cell_height;

    let local_pos = vec2<f32>(
        vertex.position.x * uniforms.cell_width,
        vertex.position.y * uniforms.cell_height
    );

    let world_pos = vec2<f32>(cell_x + local_pos.x, cell_y + local_pos.y);
    out.clip_position = uniforms.projection_matrix * vec4<f32>(world_pos, 0.0, 1.0);
    out.effects = instance.glyph_id_and_effects >> 13u; // Extract underline, strikethrough, emoji flags

    return out;
}

fn packed_bits_to_bg(packed: u32) -> f32 {
    return f32(packed & 0xFFu); // Fallback color conversion helper
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 1. Sample from our GPU Texture Array
    let sampled_color = textureSample(atlas_textures, atlas_sampler, in.tex_coord.xy, u32(in.tex_coord.z));

    // 2. Separate rendering paths for Emoji vs Standard subpixel text
    let is_emoji = (in.effects & 4u) != 0u; // Bit 15 extracted (15 - 13 = 2)
    if (is_emoji) {
        return sampled_color; // Color emoji directly bypassed
    }

    // Composite glyph alpha with cell background (transparent by default so no black boxes)
    let glyph_alpha = sampled_color.a;
    let bg_alpha = in.bg_color.a * (1.0 - glyph_alpha);
    let total_alpha = glyph_alpha + bg_alpha;

    var rgb = in.fg_color.rgb * glyph_alpha + in.bg_color.rgb * bg_alpha;
    if (total_alpha > 0.001) {
        rgb = rgb / total_alpha;
    }

    var final_text_color = vec4<f32>(rgb, total_alpha);

    // Apply inline underline/strikethrough effects directly inside the pixel shader
    let has_underline = (in.effects & 1u) != 0u;   // Bit 13 (13 - 13 = 0)
    let has_strikethrough = (in.effects & 2u) != 0u; // Bit 14 (14 - 13 = 1)
    let pixel_y = in.tex_coord.y;
    if (has_underline && pixel_y > 0.9) {
        final_text_color = in.fg_color;
    }
    if (has_strikethrough && pixel_y > 0.45 && pixel_y < 0.55) {
        final_text_color = in.fg_color;
    }

    return final_text_color;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vt100_to_cells() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        parser.process(b"Hello world\r\n");
        let (screen_rows, screen_cols) = parser.screen().size();
        for row in 0..screen_rows {
            for col in 0..screen_cols {
                if let Some(cell) = parser.screen().cell(row, col) {
                    if !cell.contents().is_empty() && cell.contents() != " " {
                        println!("Cell ({}, {}): {:?}", row, col, cell.contents());
                    }
                }
            }
        }
    }

    #[test]
    fn test_opendyslexic_fontdue_rasterization() {
        let font_data = include_bytes!("../../../assets/fonts/OpenDyslexicNerdFont-Regular.otf");
        let font = fontdue::Font::from_bytes(&font_data[..], fontdue::FontSettings::default())
            .expect("Failed to parse OpenDyslexic font with fontdue");
        let (metrics, bitmap) = font.rasterize('A', 32.0);
        println!(
            "Glyph 'A' metrics: width={}, height={}, bounds={:?}, bitmap len={}",
            metrics.width,
            metrics.height,
            metrics.bounds,
            bitmap.len()
        );
        assert!(metrics.width > 0 && metrics.height > 0);
        assert!(!bitmap.is_empty());
    }

    #[test]
    fn test_scrollback_cell_generation() {
        let mut parser = vt100::Parser::new(5, 20, 100);
        // Write 10 lines of distinct output
        for i in 1..=10 {
            parser.process(format!("Line {}\r\n", i).as_bytes());
        }

        // At scrollback = 0 (bottom view): rows should show the newest lines (e.g. 6 to 10)
        assert_eq!(parser.screen().scrollback(), 0);
        let bottom_first_row = parser
            .screen()
            .cell(0, 0)
            .map(|c| c.contents().to_string())
            .unwrap_or_default();
        assert_eq!(bottom_first_row, "L");

        // Scroll back by 4 rows into history
        parser.screen_mut().set_scrollback(4);
        assert_eq!(parser.screen().scrollback(), 4);

        // Reset scrollback to 0 (scroll to bottom)
        parser.screen_mut().set_scrollback(0);
        assert_eq!(parser.screen().scrollback(), 0);
    }

    #[test]
    fn test_cursor_styles_and_blinking() {
        let mut parser = vt100::Parser::new(5, 20, 100);
        parser.process(b"hello world");

        let mut cells = Vec::new();

        // 1. Visible cursor Block (0)
        OrpheusRenderer::build_cells(parser.screen(), &mut cells, 5, 20, true, 0, 0, 0);
        let cursor_cell = cells.last().expect("Must have cursor cell");
        assert_eq!(cursor_cell.grid_position[0], 11); // After "hello world" (len 11)
        assert_eq!(cursor_cell.grid_position[1], 0);

        // 2. Hidden cursor (blink off phase)
        let mut cells_hidden = Vec::new();
        OrpheusRenderer::build_cells(parser.screen(), &mut cells_hidden, 5, 20, false, 0, 0, 0);
        assert_eq!(cells_hidden.len(), cells.len() - 1); // No cursor cell added!

        // 3. Beam cursor (1)
        let mut cells_beam = Vec::new();
        OrpheusRenderer::build_cells(parser.screen(), &mut cells_beam, 5, 20, true, 1, 0, 0);
        let beam_cell = cells_beam.last().unwrap();
        assert_eq!(beam_cell.grid_position[0], 11);

        // 4. Underline cursor (2)
        let mut cells_underline = Vec::new();
        OrpheusRenderer::build_cells(parser.screen(), &mut cells_underline, 5, 20, true, 2, 0, 0);
        let underline_cell = cells_underline.last().unwrap();
        assert_eq!(underline_cell.grid_position[0], 11);
        assert_ne!(underline_cell.glyph_id_and_effects & (1 << 13), 0); // Underline effect flag is set!
    }

    #[test]
    fn test_selection_range_and_extraction() {
        let mut parser = vt100::Parser::new(5, 30, 100);
        parser.process(b"echo \"Hello, Well!\"\r\nsecond line here\r\n");

        // 1. Test SelectionRange normalization and containment
        let sel = SelectionRange::new(5, 0, 0, 0);
        let ((c1, r1), (c2, r2)) = sel.normalized();
        assert_eq!((c1, r1), (0, 0));
        assert_eq!((c2, r2), (5, 0));
        assert!(sel.contains(2, 0));
        assert!(!sel.contains(6, 0));

        // 2. Test extracting single line slice
        let extracted_single =
            extract_text_from_screen(parser.screen(), SelectionRange::new(0, 0, 3, 0));
        assert_eq!(extracted_single, "echo");

        // 3. Test extracting across multiple lines
        let extracted_multi =
            extract_text_from_screen(parser.screen(), SelectionRange::new(0, 0, 10, 1));
        assert_eq!(extracted_multi, "echo \"Hello, Well!\"\nsecond line");

        // 4. Test find_word_bounds
        let word_sel =
            find_word_bounds(parser.screen(), 1, 0).expect("Should find word bounds for echo");
        assert_eq!(word_sel.start_col, 0);
        assert_eq!(word_sel.end_col, 3);
        assert_eq!(extract_text_from_screen(parser.screen(), word_sel), "echo");
    }

    #[test]
    fn test_build_cells_with_selection() {
        let mut parser = vt100::Parser::new(5, 30, 100);
        parser.process(b"hello world\r\n");

        let mut cells = Vec::new();
        OrpheusRenderer::build_cells_ext(
            parser.screen(),
            &mut cells,
            5,
            30,
            false,
            0,
            0,
            0,
            Some(SelectionRange::new(0, 0, 4, 0)), // "hello"
        );

        // Find cells that have selection background 0x26E
        let selected_cells: Vec<_> = cells
            .iter()
            .filter(|c| (c.packed_colors & 0xFFF) == 0x26E)
            .collect();
        println!("Selected cells count: {}", selected_cells.len());
        assert_eq!(selected_cells.len(), 5);

        let mut cells_empty = Vec::new();
        OrpheusRenderer::build_cells_ext(
            parser.screen(),
            &mut cells_empty,
            5,
            30,
            false,
            0,
            0,
            0,
            Some(SelectionRange::new(15, 2, 25, 2)),
        );
        let sel_empty: Vec<_> = cells_empty
            .iter()
            .filter(|c| (c.packed_colors & 0xFFF) == 0x26E)
            .collect();
        println!("Selected empty cells count: {}", sel_empty.len());
        assert_eq!(sel_empty.len(), 11);
    }

    #[test]
    fn test_orpheus_crt_shader_syntax() {
        let shader_src = include_str!("../../../orpheus-crt-shader.wgsl");
        assert!(!shader_src.is_empty(), "Shader source should not be empty");
        assert!(
            shader_src.contains("@vertex"),
            "Shader must define a @vertex entry"
        );
        assert!(
            shader_src.contains("@fragment"),
            "Shader must define a @fragment entry"
        );
        assert!(shader_src.contains("fs_main"), "Shader must export fs_main");
    }
}

pub mod scrollbar;
