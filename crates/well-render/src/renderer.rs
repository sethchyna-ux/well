// renderer.rs – GPU glyph‑atlas terminal rendering

use bytemuck::{Pod, Zeroable};
use std::borrow::Cow;

/// Layout for a single character cell sent to the GPU instance buffer.
/// Packed tightly to fit in a minimal memory footprint per glyph.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GlyphInstance {
    pub position: [f32; 2], // screen cell position (col, row)
    pub size: [f32; 2],     // cell width / height
    pub uv_min: [f32; 2],   // top‑left in glyph atlas
    pub uv_max: [f32; 2],   // bottom‑right in glyph atlas
    pub fg_color: [f32; 4], // RGBA text colour
    pub bg_color: [f32; 4], // RGBA background colour
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceCountError {
    pub capacity: usize,
    pub requested: usize,
}

impl std::fmt::Display for InstanceCountError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "render batch contains {} glyphs; buffer capacity is {}",
            self.requested, self.capacity
        )
    }
}

impl std::error::Error for InstanceCountError {}

fn check_instance_count(requested: usize, capacity: usize) -> Result<(), InstanceCountError> {
    if requested > capacity {
        Err(InstanceCountError {
            capacity,
            requested,
        })
    } else {
        Ok(())
    }
}

impl GlyphInstance {
    /// Returns a ``wgpu::VertexBufferLayout`` suitable for an **instance** buffer.
    /// Each instance encodes a full glyph quad – the vertex shader expands it to four
    /// screen‑space vertices via a ``TriangleStrip``.
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GlyphInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // position – shader location 0
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // size – shader location 1
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // uv_min – shader location 2
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // uv_max – shader location 3
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // fg_color – shader location 4
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // bg_color – shader location 5
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// The core renderer for the terminal glyph grid.
pub struct TerminalRenderer {
    render_pipeline: wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,
    max_instances: usize,
    atlas_bind_group: wgpu::BindGroup,
}

impl TerminalRenderer {
    /// Creates a new renderer.
    ///
    /// * `device` – GPU device.
    /// * `config` – Surface configuration (provides the colour format).
    /// * `atlas_texture_view` – View into the glyph‑atlas texture.
    /// * `atlas_sampler` – Sampler for the atlas (normally *linear*).
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        atlas_texture_view: &wgpu::TextureView,
        atlas_sampler: &wgpu::Sampler,
    ) -> Self {
        // Bind‑group layout for the glyph atlas (texture + sampler)
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Atlas Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Atlas Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(atlas_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(atlas_sampler),
                },
            ],
        });

        // Pipeline layout – currently only the atlas bind‑group.
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Terminal Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Load the shader – ``cell_shader.wgsl`` lives alongside this file.
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Cell Shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("cell_shader.wgsl"))),
        });

        // Render pipeline – draws a textured quad per instance.
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Terminal Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[GlyphInstance::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // Instance buffer – pre‑allocate space for a large grid.
        let max_instances = 10_000usize;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Glyph Instance Buffer"),
            size: (max_instances * std::mem::size_of::<GlyphInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            render_pipeline,
            instance_buffer,
            max_instances,
            atlas_bind_group,
        }
    }

    /// Draw a batch of glyph instances.
    pub fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        queue: &wgpu::Queue,
        instances: &[GlyphInstance],
    ) -> Result<(), InstanceCountError> {
        check_instance_count(instances.len(), self.max_instances)?;
        if instances.is_empty() {
            return Ok(());
        }
        // Upload glyph instance data.
        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(instances));

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Glyph Grid Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.atlas_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        // Four vertices per glyph quad.
        render_pass.draw(0..4, 0..instances.len() as u32);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::check_instance_count;

    #[test]
    fn instance_batches_must_fit_the_upload_buffer() {
        assert_eq!(check_instance_count(10_000, 10_000), Ok(()));
        assert_eq!(check_instance_count(0, 10_000), Ok(()));
        assert!(check_instance_count(10_001, 10_000).is_err());
    }
}
