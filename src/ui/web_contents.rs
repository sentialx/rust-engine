use std::cell::{Ref, RefCell, RefMut};
use std::rc::{Rc, Weak};

use crate::events::DevtoolsSelection;
use crate::frame::{Frame, RenderDelegate};
use crate::layout::Size;
use crate::renderer::{CompositeFrame, HybridRenderer};
use crate::ui::browser_window::RenderFrameState;
use crate::ui::element_inspector::ElementInspector;

/// WebContents - wraps a RenderFrameState and its associated ElementInspector.
/// Provides a unified interface for rendering main content + overlay.
pub struct WebContents {
    inner: RenderFrameState,
    inspector: Rc<RefCell<ElementInspector>>,
    frame_id: usize,
    overlay_frame_id: usize,
}

impl WebContents {
    pub fn new(viewport: Size, frame_id: usize, overlay_frame_id: usize) -> Self {
        let mut inner = RenderFrameState::new(viewport);
        let inspector = ElementInspector::install(&mut inner);
        Self { inner, inspector, frame_id, overlay_frame_id }
    }

    /// Render main frame and overlay
    pub fn render(&mut self, renderer: &mut HybridRenderer) {
        // Render main frame
        self.inner.render(renderer, self.frame_id);

        // Update and render overlay
        self.inspector.borrow_mut().auto_update();
        self.inspector.borrow().render_overlay(renderer, self.overlay_frame_id);
    }

    /// Check and clear the selection dirty flag
    pub fn take_selection_dirty(&self) -> bool {
        self.inspector.borrow().take_selection_dirty()
    }

    /// Get composite frames (main + overlay on top)
    pub fn get_composite_frames<'a>(&self, renderer: &'a HybridRenderer, dest_x: f32) -> Vec<CompositeFrame<'a>> {
        let mut frames = Vec::new();
        if let Some(tex) = renderer.get_texture(self.frame_id) {
            frames.push(CompositeFrame { texture: tex, dest_x, scroll_y: 0.0 });
        }
        if let Some(tex) = renderer.get_texture(self.overlay_frame_id) {
            frames.push(CompositeFrame { texture: tex, dest_x, scroll_y: 0.0 });
        }
        frames
    }

    // --- Delegate methods to inner RenderFrameState ---

    pub fn frame(&self) -> Ref<'_, Frame> {
        self.inner.frame()
    }

    pub fn frame_mut(&self) -> RefMut<'_, Frame> {
        self.inner.frame_mut()
    }

    pub fn scroll_y(&self) -> f32 {
        self.inner.scroll_y()
    }

    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.inner.set_viewport(width, height);
    }

    pub fn viewport_width(&self) -> f32 {
        self.inner.frame().viewport.width
    }

    pub fn set_render_delegate(&mut self, delegate: Weak<dyn RenderDelegate>) {
        self.inner.set_render_delegate(delegate);
    }

    pub fn sink_rc(&self) -> Rc<RefCell<crate::events::EventSink>> {
        self.inner.sink_rc()
    }

    // --- Inspector access ---

    pub fn inspector(&self) -> &Rc<RefCell<ElementInspector>> {
        &self.inspector
    }

    pub fn selection(&self) -> Rc<RefCell<DevtoolsSelection>> {
        self.inspector.borrow().selection()
    }

}
