use crate::devtools::{DevtoolsAgent, ElementInspectorHandler};
use crate::events::EventSink;
use crate::frame::Frame;
use crate::layout::Size;
use crate::renderer::HybridRenderer;
use crate::ui::browser_window::RenderFrameState;
use crate::ui::overlay_frame::OverlayFrame;

use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// Element inspector - manages the highlight overlay for element inspection.
pub struct ElementInspector {
    overlay: OverlayFrame,
    agent: Rc<RefCell<DevtoolsAgent>>,
    main_frame: Weak<RefCell<Frame>>,
    main_sink: Weak<RefCell<EventSink>>,
    // Change detection for overlay rebuild
    last_selected_id: Option<u64>,
    last_scroll_y: f32,
}

impl ElementInspector {
    /// Install inspector on main frame with a DevtoolsAgent
    pub fn install(main: &mut RenderFrameState, agent: Rc<RefCell<DevtoolsAgent>>) -> Rc<RefCell<Self>> {
        // Add inspector handler to main frame (intercepts events when devtools enabled)
        main.prepend_handler(Box::new(
            ElementInspectorHandler::new(agent.clone())
        ));

        // Get references to main frame state
        let main_frame = main.frame_weak();
        let main_sink = Rc::downgrade(&main.sink_rc());

        // Set the frame in the agent
        agent.borrow_mut().set_frame(main_frame.clone());

        Rc::new(RefCell::new(Self {
            overlay: OverlayFrame::new(),
            agent,
            main_frame,
            main_sink,
            last_selected_id: None,
            last_scroll_y: 0.0,
        }))
    }

    /// Get the DevtoolsAgent
    pub fn agent(&self) -> &Rc<RefCell<DevtoolsAgent>> {
        &self.agent
    }

    /// Render the overlay
    pub fn render_overlay(&self, renderer: &mut HybridRenderer, frame_id: usize) {
        self.overlay.render(renderer, frame_id);
    }

    /// Auto-update overlay based on current state
    pub fn auto_update(&mut self) {
        // Skip update if frame or sink is currently borrowed
        let Some(viewport) = self.main_viewport() else { return };
        let Some(scroll_y) = self.main_scroll_y() else { return };

        // Sync overlay viewport with main frame
        self.overlay.set_viewport(viewport.width, viewport.height);

        // Check if anything changed that requires a rebuild
        let agent = self.agent.borrow();
        let selected_id = agent.get_selected_node_id();
        let selected_element = agent.get_selected_element();
        drop(agent);

        let selection_changed = self.last_selected_id != selected_id;
        let scroll_changed = (scroll_y - self.last_scroll_y).abs() > 0.5;

        if !selection_changed && !scroll_changed {
            return;
        }

        // Rebuild overlay content
        self.overlay.rebuild(selected_element.as_ref(), viewport, scroll_y);

        // Update tracking
        self.last_selected_id = selected_id;
        self.last_scroll_y = scroll_y;
    }

    fn main_viewport(&self) -> Option<Size> {
        self.main_frame
            .upgrade()
            .and_then(|f| f.try_borrow().ok().map(|f| f.viewport.clone()))
    }

    fn main_scroll_y(&self) -> Option<f32> {
        self.main_sink
            .upgrade()
            .and_then(|sink| sink.try_borrow().ok().map(|s| s.scroll_y()))
    }
}
