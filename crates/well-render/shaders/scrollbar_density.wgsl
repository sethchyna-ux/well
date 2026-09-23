struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct ScrollbarUniforms {
    viewport_height: f32,
    total_log_lines: u32,
    track_width_px: f32,
    error_threshold: f32, // Density value that maps to maximum warning red
};

@group(0) @binding(0) var<uniform> u_config: ScrollbarUniforms;
// 4MB resident storage buffer: 1,048,576 32-bit integer bins across the file
@group(0) @binding(1) var<storage, read> s_density_bins: array<u32>;

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Generate fullscreen-track quad from 6 indices [0..5]
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(0.96, -1.0), // Bound strictly to the right 4% of viewport
        vec2<f32>(1.00, -1.0),
        vec2<f32>(0.96,  1.0),
        vec2<f32>(0.96,  1.0),
        vec2<f32>(1.00, -1.0),
        vec2<f32>(1.00,  1.0)
    );

    let pos = positions[in_vertex_index];
    out.clip_position = vec4<f32>(pos, 0.0, 1.0);
    // Transform clip space [-1.0..1.0] to normalized UV [0.0..1.0] (top to bottom)
    out.uv = vec2<f32>((pos.x - 0.96) / 0.04, 1.0 - (pos.y * 0.5 + 0.5));
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let num_bins = arrayLength(&s_density_bins);
    let bin_idx = u32(clamp(in.uv.y * f32(num_bins), 0.0, f32(num_bins - 1u)));
    let density_count = f32(s_density_bins[bin_idx]);

    // Transparent dark gutter if no markers are found
    if (density_count == 0.0) {
        return vec4<f32>(0.08, 0.08, 0.10, 0.40);
    }

    // Map density count to thermal alert spectrum: Green (1) -> Orange -> Hot Red
    let factor = clamp(density_count / u_config.error_threshold, 0.0, 1.0);
    var color = vec3<f32>(0.0);

    if (factor < 0.5) {
        // Green to Yellow
        color = mix(vec3<f32>(0.2, 0.8, 0.2), vec3<f32>(0.9, 0.8, 0.1), factor * 2.0);
    } else {
        // Yellow to Crimson Alert
        color = mix(vec3<f32>(0.9, 0.8, 0.1), vec3<f32>(1.0, 0.05, 0.05), (factor - 0.5) * 2.0);
    }

    return vec4<f32>(color, 0.85);
}
