# 🎶 well-render: Orpheus GPU Text Shaper & WGPU Pipeline

The `well-render` crate implements the single-pass hardware-accelerated text rendering pipeline for Well using `wgpu` and Metal.

---

## Architectural Highlights

### 1. Single-Pass Instanced Draw Call (`OrpheusRenderer`)

* Rather than issuing draw calls per character or line, Orpheus assembles the entire terminal grid into a single contiguous vertex buffer of `CellInstance` primitives.
* Renders the full screen via `draw_indexed` in one GPU pass at up to 250 FPS.

### 2. Typographic Atlas & Rasterization

* Uses `fontdue` for high-performance glyph rasterization with subpixel antialiasing and OpenDyslexic / custom font fallbacks.
* Layered 2D texture array partitions ASCII, dynamic LRU bold/italic glyphs, double-wide CJK scripts, and color emojis.

### 3. Cyber-Neon Aesthetic & Typographic Contrast

* **Uppercase Shading**: Standard uppercase ASCII characters (`'A'..='Z'`) are shaded in deep midnight slate-navy (`#112244`), producing sleek contrast against electric neon green lowercase letters (`#39FF14`).
* **Semantic Highlighting**: Distinct visual tiers for prompts (`#FF007F`), punctuation (`#88929A`), and numbers (`#A855F7`).

### 4. CRT Post-Processing Shaders (`orpheus-crt-shader.wgsl`)

* Hardware-accelerated barrel distortion (screen curvature).
* Real-time scanline sinusoids.
* Phosphor chromatic fringing and bloom glow.

---

## Testing

```bash
cargo test -p well-render
```
