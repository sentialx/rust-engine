# Renderer Architecture

## Current Architecture

The renderer module has a clean separation of concerns:

1. **WgpuRenderer** (`wgpu_renderer.rs`, 236 lines)
   - Renders quads (backgrounds, borders, box shadows) to GPU texture
   - Uses instanced rendering with `quad.wgsl` shader
   - Output: `QuadTexture`

2. **SkiaRenderer** (`skia.rs`, 785 lines)
   - Renders text using tiny-skia
   - Has glyph atlas for caching
   - Output: `RenderedBuffer` (Pixmap)

3. **HybridRenderer** (`hybrid.rs`, 342 lines)
   - Holds WgpuRenderer + SkiaRenderer
   - Renders quads with WgpuRenderer
   - Renders text with SkiaRenderer
   - Composites quads + text using `text_blend.wgsl`
   - Caches output by frame ID
   - Output: `FrameTexture`

4. **WgpuCompositor** (`wgpu_compositor.rs`, 361 lines)
   - Composites FrameTextures to screen
   - Handles surface management
   - Uses `compositor.wgsl` shader

## Data Flow

```
Frame → HybridRenderer {
    GpuQuad[] → WgpuRenderer → QuadTexture
    Frame → SkiaRenderer → RenderedBuffer (text only)
    [QuadTexture + text] → composite → FrameTexture
}

FrameTexture[] → WgpuCompositor → Screen
```

## Shaders
- `quad.wgsl` - Instanced quad rendering
- `text_blend.wgsl` - Blends text over quads
- `compositor.wgsl` - Screen compositing

## Key Types
- `GpuQuad` - Quad data for GPU rendering (mod.rs)
- `QuadTexture` - Output from WgpuRenderer
- `FrameTexture` - Output from HybridRenderer
- `CompositeFrame` - Input to WgpuCompositor
