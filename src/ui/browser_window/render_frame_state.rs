use crate::frame::Frame;
use crate::layout::Rect;
use crate::renderer::{RenderedBuffer, Renderer, SkiaRenderer};

pub(crate) struct RenderFrameState {
    frame: Frame,
    buffer: Option<RenderedBuffer>,
    dirty: bool,
}

impl RenderFrameState {
    pub(crate) fn new(viewport: Rect) -> Self {
        Self {
            frame: Frame::new(viewport),
            buffer: None,
            dirty: true,
        }
    }

    pub(crate) fn frame(&self) -> &Frame {
        &self.frame
    }

    pub(crate) fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }

    pub(crate) fn invalidate(&mut self) {
        self.buffer = None;
        self.dirty = true;
    }

    pub(crate) fn set_viewport(&mut self, width: f32, height: f32) {
        self.frame.set_viewport(width, height);
        self.invalidate();
    }

    pub(crate) fn render_if_needed(&mut self, renderer: &mut SkiaRenderer) {
        if self.dirty || self.buffer.is_none() {
            self.buffer = Some(renderer.render(&self.frame));
            self.dirty = false;
        }
    }

    pub(crate) fn buffer(&self) -> Option<&RenderedBuffer> {
        self.buffer.as_ref()
    }
}
