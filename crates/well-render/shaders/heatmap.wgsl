struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    // Generate a fullscreen quad / right-edge strip based on vertex index
    // 0: top-left, 1: bottom-left, 2: top-right, 3: bottom-right
    // We want a strip on the right side of the screen (e.g., x from 0.95 to 1.0)
    var x = 0.95;
    if ((in_vertex_index & 2u) != 0u) {
        x = 1.0;
    }

    var y = 1.0; // top
    if ((in_vertex_index & 1u) != 0u) {
        y = -1.0; // bottom
    }

    var out: VertexOutput;
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    // UV maps y from 0 (top) to 1 (bottom)
    out.uv = vec2<f32>((x - 0.95) * 20.0, (1.0 - y) * 0.5);
    return out;
}

@group(0) @binding(0) var<storage, read> histogram: array<u32>;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 4096 bins total (max)
    // Map in.uv.y [0.0, 1.0] to bucket index [0, 4095]
    let bucket_f = in.uv.y * 4096.0;
    let bucket_idx = u32(clamp(bucket_f, 0.0, 4095.0));

    let density = f32(histogram[bucket_idx]);

    if (density == 0.0) {
        // Transparent if no hits
        return vec4<f32>(0.0, 0.0, 0.0, 0.1);
    }

    // Non-linear color map: green -> yellow -> alert red
    // Suppose threshold is 5 hits per block for yellow, 15 for red
    var color = vec3<f32>(0.0, 1.0, 0.0); // green
    if (density >= 15.0) {
        color = vec3<f32>(1.0, 0.0, 0.0); // red
    } else if (density >= 5.0) {
        color = vec3<f32>(1.0, 1.0, 0.0); // yellow
    }

    return vec4<f32>(color, 0.8); // 80% opacity overlay
}
