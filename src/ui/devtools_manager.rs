use crate::html::DomElement;
use crate::layout::Rect;
use crate::renderer::{CompositeRegion, SkiaRenderer};
use crate::ui::browser_window::RenderFrameState;
use crate::ui::devtools::DevtoolsOverlay;
use crate::ui::devtools_panel::DevtoolsPanel;

use std::cell::RefCell;
use std::rc::Rc;

/// Manages all devtools-related state and logic
pub struct DevtoolsManager {
    panel: DevtoolsPanel,
    overlay: DevtoolsOverlay,
    visible: bool,
    panel_width: f32,
    hover_info: Option<Rc<RefCell<DomElement>>>,
    pinned_element: Option<Rc<RefCell<DomElement>>>,
}

impl DevtoolsManager {
    pub fn new() -> Self {
        let empty_rect = Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };
        Self {
            panel: DevtoolsPanel::new(empty_rect.clone()),
            overlay: DevtoolsOverlay::new(empty_rect),
            visible: true,
            panel_width: 300.0,
            hover_info: None,
            pinned_element: None,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn toggle_visible(&mut self) {
        self.visible = !self.visible;
    }

    /// Returns the width to reserve for devtools (0 if hidden)
    pub fn reserved_width(&self) -> f32 {
        if self.visible {
            self.panel_width
        } else {
            0.0
        }
    }

    pub fn panel_viewport(&self) -> &Rect {
        self.panel.viewport()
    }

    /// Access the panel's render frame state for generic frame operations
    pub(crate) fn panel_frame(&self) -> &RenderFrameState {
        self.panel.render_state()
    }

    /// Access the panel's render frame state mutably for generic frame operations
    pub(crate) fn panel_frame_mut(&mut self) -> &mut RenderFrameState {
        self.panel.render_state_mut()
    }

    /// Returns the element to display in devtools - pinned element takes priority over hover
    pub fn selected_element(&self) -> Option<Rc<RefCell<DomElement>>> {
        self.pinned_element.clone().or_else(|| self.hover_info.clone())
    }

    pub fn is_pinned(&self) -> bool {
        self.pinned_element.is_some()
    }

    pub fn set_hover(&mut self, element: Option<Rc<RefCell<DomElement>>>) {
        self.hover_info = element;
    }

    /// Handle a click on an element - pins/unpins the element
    /// Returns true if the pin state changed
    pub fn handle_element_click(&mut self, clicked_element: Option<Rc<RefCell<DomElement>>>) -> bool {
        // Check if clicking the same element - toggle pin
        let should_unpin = match (&self.pinned_element, &clicked_element) {
            (Some(pinned), Some(clicked)) => Rc::ptr_eq(pinned, clicked),
            _ => false,
        };

        if should_unpin {
            self.pinned_element = None;
        } else {
            self.pinned_element = clicked_element;
        }
        true
    }

    /// Clear all selection state (e.g., on page refresh)
    pub fn clear_selection(&mut self) {
        self.hover_info = None;
        self.pinned_element = None;
    }

    pub fn load(&mut self) {
        self.panel.load();
    }

    pub fn set_viewport(&mut self, panel_width: f32, height: f32, main_viewport_width: f32) {
        self.panel.set_viewport(panel_width, height);
        self.overlay.set_viewport(main_viewport_width, height);
    }

    pub fn invalidate(&mut self) {
        self.panel.invalidate();
        self.overlay.invalidate();
    }

    pub fn update_content(&mut self, main_dom_tree: &Vec<Rc<RefCell<DomElement>>>) {
        let selected = self.selected_element();
        let is_pinned = self.is_pinned();
        self.panel.update(selected.as_ref(), main_dom_tree, is_pinned);
    }

    pub fn rebuild_overlay(&mut self, main_viewport: Rect, page_height: f32) {
        let selected = self.selected_element();
        self.overlay.rebuild(selected.as_ref(), main_viewport, page_height);
    }

    /// Update both panel content and overlay
    pub fn update(&mut self, main_dom_tree: &Vec<Rc<RefCell<DomElement>>>, main_viewport: Rect, page_height: f32) {
        self.update_content(main_dom_tree);
        self.rebuild_overlay(main_viewport, page_height);
    }

    pub fn render_if_needed(&mut self, renderer: &mut SkiaRenderer) {
        if self.visible {
            self.panel.render_if_needed(renderer);
            self.overlay.render_if_needed(renderer);
        }
    }

    /// Add devtools regions to the composite list
    pub fn add_composite_regions<'a>(
        &'a self,
        regions: &mut Vec<CompositeRegion<'a>>,
        main_viewport_width: f32,
        scroll_y: f32,
    ) {
        if !self.visible {
            return;
        }

        if let Some(panel_buffer) = self.panel.buffer() {
            regions.push(CompositeRegion {
                buffer: panel_buffer,
                dest_x: main_viewport_width,
                scroll_y: 0.0,
            });
        }

        if let Some(overlay_buffer) = self.overlay.buffer() {
            regions.push(CompositeRegion {
                buffer: overlay_buffer,
                dest_x: 0.0,
                scroll_y,
            });
        }
    }

    /// Handle click in devtools panel area
    pub fn dispatch_panel_click(&mut self, x: f32, y: f32) -> bool {
        self.panel.dispatch_click_at(x, y)
    }
}
