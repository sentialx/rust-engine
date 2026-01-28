// Renderer abstraction layer

mod skia;
mod wgpu_renderer;
mod wgpu_compositor;
mod hybrid;

pub use skia::SkiaRenderer;
pub use wgpu_compositor::WgpuCompositor;
pub use hybrid::{HybridRenderer, FrameTexture};

/// Frame to composite with position and scroll
pub struct CompositeFrame<'a> {
    pub texture: &'a FrameTexture,
    pub dest_x: f32,
    pub scroll_y: f32,
}

/// GPU-rendered colored quad
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct GpuQuad {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// RGB (0-255), Alpha (0-1)
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl GpuQuad {
    pub fn new(x: f32, y: f32, width: f32, height: f32, r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { x, y, width, height, r, g, b, a }
    }

    /// Convert RenderItems to GpuQuads with scale factor applied
    pub fn from_render_items_scaled(items: &[crate::layout::RenderItem], scale: f32) -> Vec<GpuQuad> {
        let mut quads = Vec::with_capacity(items.len() * 10); // shadow layers + bg + 4 borders

        for item in items {
            // Box shadow (before background) - approximate blur with layers
            let shadow = &item.box_shadow;
            if shadow.is_visible() && !shadow.inset {
                let blur = shadow.blur_radius;
                let layers = if blur > 0.0 { (blur as i32).min(6).max(2) } else { 1 };
                let step = if blur > 0.0 { blur / layers as f32 } else { 0.0 };

                for i in 0..layers {
                    let offset = step * (layers - i) as f32;
                    let alpha_factor = (i + 1) as f32 / (layers + 1) as f32;

                    let c = shadow.color;
                    quads.push(GpuQuad {
                        x: (item.x + shadow.offset_x - offset) * scale,
                        y: (item.y + shadow.offset_y - offset) * scale,
                        width: (item.width + shadow.spread_radius * 2.0 + offset * 2.0) * scale,
                        height: (item.height + shadow.spread_radius * 2.0 + offset * 2.0) * scale,
                        r: c.0, g: c.1, b: c.2,
                        a: c.3 * alpha_factor * 0.5,
                    });
                }
            }

            // Background
            let bg = item.background_color;
            if bg.3 > 0.001 {
                quads.push(GpuQuad {
                    x: item.x * scale,
                    y: item.y * scale,
                    width: item.width * scale,
                    height: item.height * scale,
                    r: bg.0, g: bg.1, b: bg.2, a: bg.3,
                });
            }

            // Borders (rendered as solid - dotted/dashed not yet supported on GPU)
            let border = &item.border;

            // Top
            if border.top.is_visible() {
                let c = border.top.color;
                quads.push(GpuQuad {
                    x: item.x * scale,
                    y: item.y * scale,
                    width: item.width * scale,
                    height: border.top.width * scale,
                    r: c.0, g: c.1, b: c.2, a: c.3,
                });
            }

            // Bottom
            if border.bottom.is_visible() {
                let c = border.bottom.color;
                quads.push(GpuQuad {
                    x: item.x * scale,
                    y: (item.y + item.height - border.bottom.width) * scale,
                    width: item.width * scale,
                    height: border.bottom.width * scale,
                    r: c.0, g: c.1, b: c.2, a: c.3,
                });
            }

            // Left
            if border.left.is_visible() {
                let c = border.left.color;
                quads.push(GpuQuad {
                    x: item.x * scale,
                    y: item.y * scale,
                    width: border.left.width * scale,
                    height: item.height * scale,
                    r: c.0, g: c.1, b: c.2, a: c.3,
                });
            }

            // Right
            if border.right.is_visible() {
                let c = border.right.color;
                quads.push(GpuQuad {
                    x: (item.x + item.width - border.right.width) * scale,
                    y: item.y * scale,
                    width: border.right.width * scale,
                    height: item.height * scale,
                    r: c.0, g: c.1, b: c.2, a: c.3,
                });
            }
        }

        quads
    }
}
