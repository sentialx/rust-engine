use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::events::{EventHandler, EventSink};
use crate::frame::{Frame, RenderDelegate};
use crate::layout::Rect;
use crate::renderer::{RenderedBuffer, Renderer, SkiaRenderer};

pub(crate) struct RenderFrameState {
    frame: Rc<RefCell<Frame>>,
    sink: Option<Rc<RefCell<EventSink>>>,
    buffer: Option<RenderedBuffer>,
    /// True if viewport changed (requires re-render but doesn't mark styles dirty)
    viewport_changed: bool,
}

impl RenderFrameState {
    /// Create a render frame with event handling (for interactive frames)
    pub(crate) fn new(viewport: Rect) -> Self {
        let frame = Frame::new(viewport);
        let sink = EventSink::new(frame.clone());

        Self {
            frame,
            sink: Some(Rc::new(RefCell::new(sink))),
            buffer: None,
            viewport_changed: true, // Force initial render
        }
    }

    /// Create a render-only frame (no event handling, for overlays)
    pub(crate) fn new_render_only(viewport: Rect) -> Self {
        Self {
            frame: Frame::new(viewport),
            sink: None,
            buffer: None,
            viewport_changed: true, // Force initial render
        }
    }

    /// Set the render delegate for this frame
    pub(crate) fn set_render_delegate(&mut self, delegate: Weak<dyn RenderDelegate>) {
        self.frame.borrow_mut().set_render_delegate(delegate);
    }

    pub(crate) fn frame(&self) -> std::cell::Ref<'_, Frame> {
        self.frame.borrow()
    }

    pub(crate) fn frame_mut(&self) -> std::cell::RefMut<'_, Frame> {
        self.frame.borrow_mut()
    }

    pub(crate) fn sink_rc(&self) -> Rc<RefCell<EventSink>> {
        self.sink.clone().expect("sink_rc called on render-only frame")
    }

    /// Add a handler that runs before the default handler
    pub(crate) fn prepend_handler(&mut self, handler: Box<dyn EventHandler>) {
        if let Some(ref sink) = self.sink {
            sink.borrow_mut().prepend_handler(handler);
        }
    }

    pub(crate) fn scroll_y(&self) -> f32 {
        self.sink.as_ref().map(|s| s.borrow().scroll_y()).unwrap_or(0.0)
    }

    pub(crate) fn set_viewport(&mut self, width: f32, height: f32) {
        let frame = self.frame.borrow();
        let width_changed = (frame.viewport.width - width).abs() > 0.01;
        let height_changed = (frame.viewport.height - height).abs() > 0.01;
        if !width_changed && !height_changed {
            return;
        }
        drop(frame);

        self.frame.borrow_mut().set_viewport(width, height);
        self.viewport_changed = true;
    }

    /// Update viewport size without triggering relayout (for debouncing)
    pub(crate) fn set_viewport_size(&mut self, width: f32, height: f32) {
        let frame = self.frame.borrow();
        let width_changed = (frame.viewport.width - width).abs() > 0.01;
        let height_changed = (frame.viewport.height - height).abs() > 0.01;
        if !width_changed && !height_changed {
            return;
        }
        drop(frame);

        self.frame.borrow_mut().set_viewport_size(width, height);
        self.viewport_changed = true;
    }

    /// Trigger relayout with current viewport dimensions
    pub(crate) fn relayout(&mut self) {
        self.frame.borrow_mut().relayout();
        self.viewport_changed = true;
    }

    /// Render only if content has changed
    pub(crate) fn render(&mut self, renderer: &mut SkiaRenderer) {
        // Check if Frame has pending style changes (content changed)
        let styles_updated = self.frame.borrow_mut().update_styles_if_needed();

        let needs_render = styles_updated || self.viewport_changed || self.buffer.is_none();
        if !needs_render {
            return; // Skip rendering - buffer is still valid
        }

        self.buffer = Some(renderer.render(&self.frame.borrow()));
        self.viewport_changed = false;
    }

    pub(crate) fn buffer(&self) -> Option<&RenderedBuffer> {
        self.buffer.as_ref()
    }
}
