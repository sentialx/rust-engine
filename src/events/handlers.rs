use std::cell::RefCell;
use std::rc::Rc;

use crate::frame::Frame;
use crate::dom::DomElement;

use super::{EventHandler, EventResult, InputEvent, InputEventKind};

/// Default event handler that manages hover state and dispatches click events
pub struct DefaultEventHandler {
    hovered_element: Option<Rc<RefCell<DomElement>>>,
}

impl DefaultEventHandler {
    pub fn new() -> Self {
        Self {
            hovered_element: None,
        }
    }

    fn handle_hover(&mut self, event: &mut InputEvent) -> EventResult {
        let new_hover = event.target.clone();

        // Check if hover changed
        let changed = match (&self.hovered_element, &new_hover) {
            (Some(old), Some(new)) => !Rc::ptr_eq(old, new),
            (None, None) => false,
            _ => true,
        };

        if changed {
            // Update pseudo-class state: old element leaves, new element enters
            // set_hover() marks elements as style_dirty automatically
            let old_hover = self.hovered_element.take();
            if let Some(ref old) = old_hover {
                Self::set_hover_recursive(old, false);
            }
            if let Some(ref new) = new_hover {
                Self::set_hover_recursive(new, true);
            }
            self.hovered_element = new_hover;

            return EventResult::handled();
        }

        EventResult::default()
    }

    fn set_hover_recursive(element: &Rc<RefCell<DomElement>>, hover: bool) {
        element.borrow_mut().set_hover(hover);
        if let Some(ref parent) = element.borrow().parent_node.clone() {
            Self::set_hover_recursive(&parent, hover);
        }
    }

    /// Get the currently hovered element
    pub fn hovered_element(&self) -> Option<Rc<RefCell<DomElement>>> {
        self.hovered_element.clone()
    }

    /// Clear hover state (e.g., on page refresh)
    pub fn clear_hover(&mut self) {
        if let Some(ref old) = self.hovered_element {
            Self::set_hover_recursive(old, false);
        }
        self.hovered_element = None;
    }
}

impl Default for DefaultEventHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl EventHandler for DefaultEventHandler {
    fn handle(&mut self, event: &mut InputEvent, _frame: &Rc<RefCell<Frame>>) -> EventResult {
        match event.kind {
            InputEventKind::MouseMove => self.handle_hover(event),
            // Click is handled by EventSink's run_default
            _ => EventResult::default(),
        }
    }
}
