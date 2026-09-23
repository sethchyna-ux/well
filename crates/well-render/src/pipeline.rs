use bytemuck::{Pod, Zeroable};
use std::sync::Arc;

/// Packed per-image quad instance data (32 bytes).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable, PartialEq)]
pub struct ImageQuad {
    pub position_min: [f32; 2],
    pub position_max: [f32; 2],
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub texture_index: u32,
    pub z_index: u32,
}

impl ImageQuad {
    pub fn new(
        pos_min: [f32; 2],
        pos_max: [f32; 2],
        uv_min: [f32; 2],
        uv_max: [f32; 2],
        texture_index: u32,
        z_index: u32,
    ) -> Self {
        Self {
            position_min: pos_min,
            position_max: pos_max,
            uv_min,
            uv_max,
            texture_index,
            z_index,
        }
    }
}

/// Static corner vertex for image quads.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ImageVertex {
    pub corner: [f32; 2],
}

pub const IMAGE_WGSL: &str = r#"
struct VertexInput {
    @location(0) corner: vec2<f32>,
    @location(1) pos_min: vec2<f32>,
    @location(2) pos_max: vec2<f32>,
    @location(3) uv_min: vec2<f32>,
    @location(4) uv_max: vec2<f32>,
    @location(5) texture_idx: u32,
    @location(6) z_idx: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let pos = mix(in.pos_min, in.pos_max, in.corner);
    let uv = mix(in.uv_min, in.uv_max, in.corner);
    
    // pos is expected in NDC [-1.0, 1.0]
    out.clip_position = vec4<f32>(pos, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.uv);
}
"#;

pub struct ImagePipeline {
    pub render_pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub instance_buffer: wgpu::Buffer,
    pub sampler: wgpu::Sampler,
}

impl ImagePipeline {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        use wgpu::util::DeviceExt;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ImageQuadShader"),
            source: wgpu::ShaderSource::Wgsl(IMAGE_WGSL.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ImageQuadBindGroupLayout"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ImageQuadPipelineLayout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // 4 corner vertices: [0,0], [1,0], [1,1], [0,1]
        let unit_corners = [
            ImageVertex { corner: [0.0, 0.0] },
            ImageVertex { corner: [1.0, 0.0] },
            ImageVertex { corner: [1.0, 1.0] },
            ImageVertex { corner: [0.0, 1.0] },
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ImageUnitVertexBuffer"),
            contents: bytemuck::cast_slice(&unit_corners),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let indices: [u16; 6] = [0, 1, 2, 2, 3, 0];
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ImageUnitIndexBuffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        });

        // Preallocate space for up to 256 active image quads per frame
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ImageQuadInstanceBuffer"),
            size: (256 * std::mem::size_of::<ImageQuad>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("KittyImageFilteringSampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ImageQuadRenderPipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[
                    // Buffer 0: static unit corner vertex
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<ImageVertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        }],
                    },
                    // Buffer 1: per-quad instance data
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<ImageQuad>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 0,
                                shader_location: 1, // pos_min
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 8,
                                shader_location: 2, // pos_max
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 16,
                                shader_location: 3, // uv_min
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 24,
                                shader_location: 4, // uv_max
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Uint32,
                                offset: 32,
                                shader_location: 5, // texture_idx
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Uint32,
                                offset: 36,
                                shader_location: 6, // z_idx
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

        Self {
            render_pipeline,
            bind_group_layout,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            sampler,
        }
    }

    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ImageQuadBindGroup"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    pub fn create_texture_from_rgba(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        rgba_bytes: &[u8],
    ) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
        let size = wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("KittyImageTexture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba_bytes,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width.max(1)),
                rows_per_image: Some(height.max(1)),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.create_bind_group(device, &view);
        (texture, view, bind_group)
    }

    pub fn draw_images(
        &self,
        render_pass: &mut wgpu::RenderPass,
        queue: &wgpu::Queue,
        images: &[(&wgpu::BindGroup, ImageQuad)],
    ) {
        if images.is_empty() {
            return;
        }

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        for (bind_group, quad) in images {
            queue.write_buffer(&self.instance_buffer, 0, bytemuck::bytes_of(quad));
            render_pass.set_bind_group(0, bind_group, &[]);
            render_pass.set_vertex_buffer(
                1,
                self.instance_buffer
                    .slice(..std::mem::size_of::<ImageQuad>() as u64),
            );
            render_pass.draw_indexed(0..6, 0, 0..1);
        }
    }
}

/// Stores decoded in-memory image textures and manages GPU textures for Kitty graphics.
pub struct KittyImageEntry {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub texture: Arc<wgpu::Texture>,
    pub view: Arc<wgpu::TextureView>,
    pub bind_group: Arc<wgpu::BindGroup>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_quad_layout() {
        assert_eq!(std::mem::size_of::<ImageQuad>(), 40);
        let quad = ImageQuad::new([-0.5, -0.5], [0.5, 0.5], [0.0, 0.0], [1.0, 1.0], 0, 1);
        assert_eq!(quad.position_min, [-0.5, -0.5]);
        assert_eq!(quad.position_max, [0.5, 0.5]);
        assert_eq!(quad.texture_index, 0);
        assert_eq!(quad.z_index, 1);
    }

    #[test]
    fn test_image_wgsl_syntax() {
        let module = naga::front::wgsl::parse_str(IMAGE_WGSL);
        assert!(
            module.is_ok(),
            "Image WGSL shader failed syntax parsing: {:?}",
            module.err()
        );
    }
}
