use std::cell::RefCell;
use std::rc::Rc;

use crate::events::{EventHandler, EventSink};
use crate::frame::Frame;
use crate::layout::Rect;
use crate::renderer::{RenderedBuffer, Renderer, SkiaRenderer};

pub(crate) struct RenderFrameState {
    frame: Rc<RefCell<Frame>>,
    sink: Option<Rc<RefCell<EventSink>>>,
    buffer: Option<RenderedBuffer>,
    dirty: bool,
}

impl RenderFrameState {
    /// Create a render frame with event handling (for interactive frames)
    pub(crate) fn new(viewport: Rect) -> Self {
        let frame = Frame::new(viewport);
        // EventSink now includes DefaultEventHandler automatically
        let sink = EventSink::new(frame.clone());

        Self {
            frame,
            sink: Some(Rc::new(RefCell::new(sink))),
            buffer: None,
            dirty: true,
        }
    }

    /// Create a render-only frame (no event handling, for overlays)
    pub(crate) fn new_render_only(viewport: Rect) -> Self {
        Self {
            frame: Frame::new(viewport),
            sink: None,
            buffer: None,
            dirty: true,
        }
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

    pub(crate) fn invalidate(&mut self) {
        self.buffer = None;
        self.dirty = true;
    }

    pub(crate) fn set_viewport(&mut self, width: f32, height: f32) {
        let frame = self.frame.borrow();
        let width_changed = (frame.viewport.width - width).abs() > 0.01;
        let height_changed = (frame.viewport.height - height).abs() > 0.01;
        if !width_changed && !height_changed {
            return;
        }
        drop(frame);

        let mut frame = self.frame.borrow_mut();
        frame.set_viewport(width, height);
        if frame.cached_layout_tree.is_some() {
            frame.fast_reflow();
        }
        drop(frame);

        self.invalidate();
    }

    pub(crate) fn render_if_needed(&mut self, renderer: &mut SkiaRenderer) {
        // Process any pending style changes before rendering
        if self.frame.borrow_mut().update_styles_if_needed() {
            eprintln!("render_if_needed: styles updated, invalidating");
            self.invalidate();
        }

        if self.dirty || self.buffer.is_none() {
            eprintln!("render_if_needed: RE-RENDERING");
            self.buffer = Some(renderer.render(&self.frame.borrow()));
            self.dirty = false;
        }
    }

    pub(crate) fn buffer(&self) -> Option<&RenderedBuffer> {
        self.buffer.as_ref()
    }
}
