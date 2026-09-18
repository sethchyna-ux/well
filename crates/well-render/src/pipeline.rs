use bytemuck::{Pod, Zeroable};

// Packed per-cell instance data (64 bits / 8 bytes)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ImageQuad {
    pub position_min: [f32; 2],
    pub position_max: [f32; 2],
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub texture_index: u32,
    pub z_index: u32,
}

// TODO: Pipeline abstractions for WGSL
