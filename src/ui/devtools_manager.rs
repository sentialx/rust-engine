use crate::events::{DevtoolsOverlayHandler, DevtoolsSelection, EventSink};
use crate::dom::DomElement;
use crate::layout::Rect;
use crate::ui::browser_window::RenderFrameState;
use crate::ui::devtools::DevtoolsOverlay;
use crate::ui::devtools_panel::DevtoolsPanel;

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::frame::RenderDelegate;

/// Manages all devtools-related state and logic
pub struct DevtoolsManager {
    panel: DevtoolsPanel,
    overlay: DevtoolsOverlay,
    panel_width: f32,
    selection: Rc<RefCell<DevtoolsSelection>>,
    /// Track last selected element to avoid redundant updates
    last_selected: Option<Weak<RefCell<DomElement>>>,
}

impl DevtoolsManager {
    /// Create devtools manager and attach it to the target frame for inspection
    pub(crate) fn create_for_frame(target: &mut RenderFrameState) -> Self {
        let empty_rect = Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };

        // Create shared selection state
        let selection = Rc::new(RefCell::new(DevtoolsSelection {
            visible: true,
            hover: None,
            pinned: None,
        }));

        // Add overlay handler to target frame (intercepts events when devtools visible)
        target.prepend_handler(Box::new(
            DevtoolsOverlayHandler::new(selection.clone())
        ));

        Self {
            panel: DevtoolsPanel::new(empty_rect.clone()),
            overlay: DevtoolsOverlay::new(empty_rect),
            panel_width: 300.0,
            selection,
            last_selected: None,
        }
    }

    /// Check if the selected element has changed since last update
    fn selection_changed(&self) -> bool {
        let current = self.selected_element();
        match (&self.last_selected, &current) {
            (None, None) => false,
            (Some(_), None) | (None, Some(_)) => true,
            (Some(weak), Some(strong)) => {
                weak.upgrade().map_or(true, |prev| !Rc::ptr_eq(&prev, strong))
            }
        }
    }

    pub fn is_visible(&self) -> bool {
        self.selection.borrow().visible
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.selection.borrow_mut().visible = visible;
    }

    pub fn toggle_visible(&mut self) {
        let mut sel = self.selection.borrow_mut();
        sel.visible = !sel.visible;
    }

    /// Returns the width to reserve for devtools (0 if hidden)
    pub fn reserved_width(&self) -> f32 {
        if self.is_visible() {
            self.panel_width
        } else {
            0.0
        }
    }

    pub fn panel_viewport(&self) -> Rect {
        self.panel.viewport()
    }

    pub fn panel_sink_rc(&self) -> Rc<RefCell<EventSink>> {
        self.panel.sink_rc()
    }

    pub fn panel_scroll_y(&self) -> f32 {
        self.panel.scroll_y()
    }

    /// Returns the element to display in devtools - pinned element takes priority over hover
    pub fn selected_element(&self) -> Option<Rc<RefCell<DomElement>>> {
        self.selection.borrow().selected()
    }

    pub fn is_pinned(&self) -> bool {
        self.selection.borrow().is_pinned()
    }

    /// Clear all selection state (e.g., on page refresh)
    pub fn clear_selection(&mut self) {
        self.selection.borrow_mut().clear();
        self.last_selected = None;
        self.overlay.reset();
    }

    pub fn load(&mut self) {
        self.panel.load();
    }

    pub fn set_viewport(&mut self, panel_width: f32, height: f32, main_viewport_width: f32) {
        self.panel.set_viewport(panel_width, height);
        self.overlay.set_viewport(main_viewport_width, height);
    }

    /// Update viewport size without triggering relayout (for debouncing)
    pub fn set_viewport_size(&mut self, panel_width: f32, height: f32, main_viewport_width: f32) {
        self.panel.set_viewport_size(panel_width, height);
        self.overlay.set_viewport_size(main_viewport_width, height);
    }

    /// Trigger relayout with current viewport dimensions
    pub fn relayout(&mut self) {
        self.panel.relayout();
        self.overlay.relayout();
    }

    /// Set render delegate for panel and overlay frames
    pub fn set_render_delegate(&mut self, delegate: Weak<dyn RenderDelegate>) {
        self.panel.set_render_delegate(delegate.clone());
        self.overlay.set_render_delegate(delegate);
    }


    pub fn update_content(&mut self, main_dom_tree: &Vec<Rc<RefCell<DomElement>>>) {
        let selected = self.selected_element();
        let is_pinned = self.is_pinned();
        self.panel.update(selected.as_ref(), main_dom_tree, is_pinned);
    }

    pub fn rebuild_overlay(&mut self, main_viewport: Rect, scroll_y: f32) {
        let selected = self.selected_element();
        self.overlay.rebuild(selected.as_ref(), main_viewport, scroll_y);
    }

    /// Update both panel content and overlay (only if selection changed)
    pub fn update(&mut self, main_dom_tree: &Vec<Rc<RefCell<DomElement>>>, main_viewport: Rect, scroll_y: f32) {
        if !self.selection_changed() {
            return;
        }

        // Update last_selected tracking
        self.last_selected = self.selected_element().map(|rc| Rc::downgrade(&rc));

        self.update_content(main_dom_tree);
        self.rebuild_overlay(main_viewport, scroll_y);
    }

    /// Access panel for rendering
    pub fn panel_mut(&mut self) -> &mut DevtoolsPanel {
        &mut self.panel
    }

    /// Access overlay for rendering
    pub fn overlay_mut(&mut self) -> &mut DevtoolsOverlay {
        &mut self.overlay
    }
}
