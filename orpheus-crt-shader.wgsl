// Orpheus WebGPU CRT & Text-Glow Shader (WGSL)
// Statically compiled for the "Well" terminal rendering engine.
// Implements physical coordinate curvature, dynamic scanline simulation,
// edge vignettes, and subpixel color bleeding.

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @flat color: vec4<f32>,
};

struct CustomShaderUniforms {
    u_time: f32,                 // System runtime clock in seconds
    u_cursor_pos: vec2<f32>,     // Current subpixel cursor grid coordinate
    u_prev_cursor_pos: vec2<f32>,// Previous frame cursor grid coordinate
    u_time_since_blink: f32,    // Interpolation phase factor for animations
    u_grid_dims: vec2<u32>,      // Total columns and rows in active viewport
};

@group(0) @binding(0) var<uniform> uniforms: CustomShaderUniforms;
@group(0) @binding(1) var texture_atlas: texture_2d_array<f32>;
@group(0) @binding(2) var texture_sampler: sampler;

// Configurable constants for retro monitor modeling
const CURVATURE_X: f32 = 0.04;      // Horizontal radial screen warping intensity
const CURVATURE_Y: f32 = 0.05;      // Vertical radial screen warping intensity
const SCANLINE_DENSITY: f32 = 2.0;  // Multiplier relative to screen height
const SCANLINE_BRIGHTNESS: f32 = 0.15; // Scanline darkness drop factor
const PHOSPHOR_BLEED: f32 = 0.0015; // Horizontal RGB shift for subpixel color bleeding
const VIGNETTE_SHARPNESS: f32 = 12.0; // Outer corner fading falloff sharpness
const VIGNETTE_SCALE: f32 = 0.95;     // Vignette diameter scale

// Radial Barrel Distortion (Curvature calculation)
// Translates flat texture coordinates into spherical screen coordinates
fn apply_barrel_distortion(uv: vec2<f32>) -> vec2<f32> {
    // Translate coordinate origin to center of viewport [-1.0, 1.0]
    let center = uv - 0.5;
    let dist_sq = dot(center, center);
    
    // Compute polynomial radial displacement
    let radial_factor = 1.0 + dist_sq * (CURVATURE_X + CURVATURE_Y * dist_sq);
    
    // Scale back to normalized coordinate space [0.0, 1.0]
    let warped_uv = center * radial_factor + 0.5;
    return warped_uv;
}

// Procedural Vignette Generator
// Dims the pixels near the physical borders of the curved display
fn compute_vignette(uv: vec2<f32>) -> f32 {
    let border = uv * (1.0 - uv);
    let intensity = border.x * border.y * VIGNETTE_SHARPNESS;
    return clamp(pow(intensity, VIGNETTE_SCALE), 0.0, 1.0);
}

// Main Fragment Shader
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // 1. Calculate warped texture coordinate
    let curved_uv = apply_barrel_distortion(input.uv);
    
    // 2. Reject out-of-bounds pixels (the screen bezel area)
    if curved_uv.x < 0.0 || curved_uv.x > 1.0 || curved_uv.y < 0.0 || curved_uv.y > 1.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0); // Render black bezel
    }
    
    // 3. Subpixel Phosphor Color Bleeding (RGB Aberration)
    // Shift texture fetch coordinates by minor pixel offsets for Red, Green, and Blue
    let coord_r = curved_uv + vec2<f32>(-PHOSPHOR_BLEED, 0.0);
    let coord_g = curved_uv;
    let coord_b = curved_uv + vec2<f32>(PHOSPHOR_BLEED, 0.0);
    
    // Sample texture layers (using layer 0 for monochrome text glyph-atlas index)
    let sample_r = textureSample(texture_atlas, texture_sampler, coord_r, 0u).r;
    let sample_g = textureSample(texture_atlas, texture_sampler, coord_g, 0u).r;
    let sample_b = textureSample(texture_atlas, texture_sampler, coord_b, 0u).r;
    
    var final_color = vec3<f32>(
        sample_r * input.color.r,
        sample_g * input.color.g,
        sample_b * input.color.b
    );
    
    // 4. Procedural Scanline Generator
    // Tracks horizontal rows to insert cathode ray tube scanlines
    let scanline_phase = curved_uv.y * f32(uniforms.grid_dims.y) * SCANLINE_DENSITY * 3.14159265;
    let scanline_brightness = 1.0 - (abs(sin(scanline_phase)) * SCANLINE_BRIGHTNESS);
    final_color = final_color * scanline_brightness;
    
    // 5. Cathode Screen Flickering Simulation
    // Subtle, low-amplitude global brightness modulation over time
    let flicker = 1.0 - (0.015 * sin(uniforms.u_time * 45.0) * cos(uniforms.u_time * 12.0));
    final_color = final_color * flicker;
    
    // 6. Subpixel Cursor Wave & Hover Halos
    // Renders active neon cursor glow over coordinates utilizing time variables
    let cursor_dist = distance(curved_uv * vec2<f32>(uniforms.grid_dims), uniforms.u_cursor_pos);
    if cursor_dist < 1.8 {
        let glow_intensity = (1.8 - cursor_dist) / 1.8;
        let pulse = sin(uniforms.u_time * 8.0) * 0.15 + 0.85;
        // Injects neon green glow around active typing boundary
        final_color = final_color + vec3<f32>(0.0, glow_intensity * 0.45 * pulse, 0.0);
    }
    
    // 7. Bezel Reflection Vignette
    let vignette = compute_vignette(curved_uv);
    final_color = final_color * vignette;
    
    // Return composed frame buffer
    return vec4<f32>(final_color, input.color.a);
}
