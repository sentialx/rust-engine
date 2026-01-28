use crate::dom::DomElement;
use crate::events::{DevtoolsOverlayHandler, DevtoolsSelection, EventSink};
use crate::frame::Frame;
use crate::layout::Size;
use crate::renderer::HybridRenderer;
use crate::ui::browser_window::RenderFrameState;
use crate::ui::overlay_frame::OverlayFrame;

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// Element inspector - manages element inspection on a target frame.
/// Installs event handler on main frame and coordinates overlay updates.
pub struct ElementInspector {
    overlay: OverlayFrame,
    selection: Rc<RefCell<DevtoolsSelection>>,
    main_frame: Weak<RefCell<Frame>>,
    main_sink: Weak<RefCell<EventSink>>,
    last_selected: Option<Weak<RefCell<DomElement>>>,
    last_scroll_y: f32,
    selection_dirty: Cell<bool>,
}

impl ElementInspector {
    /// Install inspector on main frame, returns inspector that auto-updates
    pub fn install(main: &mut RenderFrameState) -> Rc<RefCell<Self>> {
        // Create shared selection state (hidden by default until devtools opens)
        let selection = Rc::new(RefCell::new(DevtoolsSelection {
            visible: false,
            hover: None,
            pinned: None,
        }));

        // Add overlay handler to main frame (intercepts events when devtools visible)
        main.prepend_handler(Box::new(
            DevtoolsOverlayHandler::new(selection.clone())
        ));

        // Get references to main frame state
        let main_frame = main.frame_weak();
        let main_sink = Rc::downgrade(&main.sink_rc());

        Rc::new(RefCell::new(Self {
            overlay: OverlayFrame::new(),
            selection,
            main_frame,
            main_sink,
            last_selected: None,
            last_scroll_y: 0.0,
            selection_dirty: Cell::new(false),
        }))
    }

    /// Get selection state
    pub fn selection(&self) -> Rc<RefCell<DevtoolsSelection>> {
        self.selection.clone()
    }

    /// Returns the element to display - pinned takes priority over hover
    pub fn selected_element(&self) -> Option<Rc<RefCell<DomElement>>> {
        self.selection.borrow().selected()
    }

    pub fn is_pinned(&self) -> bool {
        self.selection.borrow().is_pinned()
    }

    /// Check and clear the selection dirty flag
    pub fn take_selection_dirty(&self) -> bool {
        self.selection_dirty.replace(false)
    }

    /// Clear selection (e.g., on page reload)
    pub fn clear_selection(&mut self) {
        self.selection.borrow_mut().clear();
        self.last_selected = None;
        self.overlay.reset();
    }

    /// Access overlay frame for rendering
    pub fn overlay_frame(&self) -> std::cell::Ref<'_, Frame> {
        self.overlay.frame()
    }

    /// Render the overlay
    pub fn render_overlay(&self, renderer: &mut HybridRenderer, frame_id: usize) {
        self.overlay.render(renderer, frame_id);
    }

    // --- Internal methods ---

    /// Get viewport from main frame (returns None if frame is currently borrowed)
    fn main_viewport(&self) -> Option<Size> {
        self.main_frame
            .upgrade()
            .and_then(|f| f.try_borrow().ok().map(|f| f.viewport.clone()))
    }

    /// Get scroll_y from main frame's event sink (returns None if sink is currently borrowed)
    fn main_scroll_y(&self) -> Option<f32> {
        self.main_sink
            .upgrade()
            .and_then(|sink| sink.try_borrow().ok().map(|s| s.scroll_y()))
    }

    /// Auto-update overlay based on current main frame state
    pub fn auto_update(&mut self) {
        // Skip update if frame or sink is currently borrowed (e.g., during event handling)
        let Some(viewport) = self.main_viewport() else { return };
        let Some(scroll_y) = self.main_scroll_y() else { return };

        // Sync overlay viewport with main frame
        self.overlay.set_viewport(viewport.width, viewport.height);

        // Check if anything changed that requires a rebuild
        let selected = self.selected_element();
        let selection_changed = self.selection_changed_with(&selected);
        let scroll_changed = (scroll_y - self.last_scroll_y).abs() > 0.5;

        if selection_changed {
            self.selection_dirty.set(true);
        }

        if !selection_changed && !scroll_changed {
            return;
        }

        // Rebuild overlay content
        self.overlay.rebuild(selected.as_ref(), viewport, scroll_y);

        // Update tracking
        self.last_selected = selected.map(|rc| Rc::downgrade(&rc));
        self.last_scroll_y = scroll_y;
    }

    /// Check if selection changed compared to a given current selection
    fn selection_changed_with(&self, current: &Option<Rc<RefCell<DomElement>>>) -> bool {
        match (&self.last_selected, current) {
            (None, None) => false,
            (Some(_), None) | (None, Some(_)) => true,
            (Some(weak), Some(strong)) => {
                weak.upgrade().map_or(true, |prev| !Rc::ptr_eq(&prev, strong))
            }
        }
    }
}

