// Renderer abstraction layer

mod skia;

pub use skia::SkiaRenderer;

use crate::frame::Frame;
use tiny_skia::Pixmap;

pub struct RenderedBuffer {
    pub pixmap: Pixmap,
}

pub struct CompositeRegion<'a> {
    pub buffer: &'a RenderedBuffer,
    pub dest_x: f32,
    pub scroll_y: f32,
    /// If true, use fast copy. If false, use alpha blending.
    pub opaque: bool,
}

/// Abstract renderer trait - implement for different backends
pub trait Renderer {
    /// Initialize/resize the renderer for given dimensions
    fn resize(&mut self, width: u32, height: u32, scale_factor: f32);

    /// Render a frame's content to an off-screen buffer
    fn render(&mut self, frame: &Frame) -> RenderedBuffer;

    /// Composite multiple rendered buffers into the display buffer
    fn composite(&mut self, regions: &[CompositeRegion]);

    /// Get the display buffer as raw pixels (ARGB format for softbuffer)
    fn get_display_buffer(&self) -> &[u32];

    /// Clear caches (e.g., when scale factor changes)
    fn clear_caches(&mut self);
}
