// cell_shader.wgsl – Glyph cell rendering

// Vertex input: instance data describing a glyph cell.
struct GlyphInstance {
    @location(0) position: vec2<f32>, // cell top‑left position (pixel or NDC space)
    @location(1) size: vec2<f32>,     // cell width and height
    @location(2) uv_min: vec2<f32>,   // top‑left UV in the atlas
    @location(3) uv_max: vec2<f32>,   // bottom‑right UV in the atlas
    @location(4) fg_color: vec4<f32>, // foreground colour
    @location(5) bg_color: vec4<f32>, // background colour
};

// Vertex shader outputs geometry and interpolated data.
struct VertexOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) fg_color: vec4<f32>,
    @location(2) bg_color: vec4<f32>,
};

// Simple helper to pick a corner offset based on the vertex index.
fn corner_offset(idx: u32) -> vec2<f32> {
    // (0,0), (1,0), (0,1), (1,1)
    switch idx {
        case 0u { return vec2<f32>(0.0, 0.0); }
        case 1u { return vec2<f32>(1.0, 0.0); }
        case 2u { return vec2<f32>(0.0, 1.0); }
        default { return vec2<f32>(1.0, 1.0); }
    }
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: GlyphInstance
) -> VertexOut {
    // Compute per‑vertex position for the quad.
    let offset = corner_offset(vertex_index);
    // Position in whatever coordinate system the CPU supplied – we forward directly.
    let pos = instance.position + offset * instance.size;

    // Interpolate UV coordinates across the quad.
    let uv = mix(instance.uv_min, instance.uv_max, offset);

    var out: VertexOut;
    out.pos = vec4<f32>(pos, 0.0, 1.0);
    out.uv = uv;
    out.fg_color = instance.fg_color;
    out.bg_color = instance.bg_color;
    return out;
}

// Bindings for the glyph atlas texture and sampler (group 0, bindings 0‑1).
@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // Sample the glyph atlas – we assume the glyph’s alpha is stored in the red channel.
    let tex = textureSample(atlas, atlas_sampler, in.uv);
    // Blend foreground over background using the sampled alpha.
    return mix(in.bg_color, in.fg_color, tex.r);
}
