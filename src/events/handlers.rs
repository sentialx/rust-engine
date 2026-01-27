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

        // Debug: print what element is being hovered
        if let Some(ref el) = new_hover {
            let el_ref = el.borrow();
            eprintln!("handle_hover: target=<{}> classes={:?}", el_ref.tag_name, el_ref.class_list);
        }

        // Check if hover changed
        let changed = match (&self.hovered_element, &new_hover) {
            (Some(old), Some(new)) => !Rc::ptr_eq(old, new),
            (None, None) => false,
            _ => true,
        };

        if changed {
            eprintln!("handle_hover: hover CHANGED");
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

/// Shared state between DevtoolsOverlayHandler and DevtoolsManager
#[derive(Default)]
pub struct DevtoolsSelection {
    pub visible: bool,
    pub hover: Option<Rc<RefCell<DomElement>>>,
    pub pinned: Option<Rc<RefCell<DomElement>>>,
}

impl DevtoolsSelection {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the element to display - pinned takes priority over hover
    pub fn selected(&self) -> Option<Rc<RefCell<DomElement>>> {
        self.pinned.clone().or_else(|| self.hover.clone())
    }

    pub fn is_pinned(&self) -> bool {
        self.pinned.is_some()
    }

    pub fn clear(&mut self) {
        self.hover = None;
        self.pinned = None;
    }
}

/// Handler for devtools element picking and hover overlay.
/// Installed on the main frame's EventSink.
pub struct DevtoolsOverlayHandler {
    state: Rc<RefCell<DevtoolsSelection>>,
}

impl DevtoolsOverlayHandler {
    pub fn new(state: Rc<RefCell<DevtoolsSelection>>) -> Self {
        Self { state }
    }
}

impl EventHandler for DevtoolsOverlayHandler {
    fn handle(&mut self, event: &mut InputEvent, _frame: &Rc<RefCell<Frame>>) -> EventResult {
        let state = self.state.borrow();
        if !state.visible {
            return EventResult::default();
        }
        drop(state);

        match event.kind {
            InputEventKind::Click => {
                let mut state = self.state.borrow_mut();
                let clicked = event.target.clone();

                // Toggle pin: if clicking same element, unpin; otherwise pin new element
                let should_unpin = match (&state.pinned, &clicked) {
                    (Some(pinned), Some(clicked)) => Rc::ptr_eq(pinned, clicked),
                    _ => false,
                };

                if should_unpin {
                    state.pinned = None;
                } else {
                    state.pinned = clicked;
                }

                // Intercept the click - don't let it through to default behavior
                event.stop_propagation();
                event.prevent_default();

                EventResult::handled()
            }
            InputEventKind::MouseMove => {
                let mut state = self.state.borrow_mut();

                // Only update hover if not pinned
                if state.pinned.is_none() {
                    let new_hover = event.target.clone();
                    let changed = match (&state.hover, &new_hover) {
                        (Some(old), Some(new)) => !Rc::ptr_eq(old, new),
                        (None, None) => false,
                        _ => true,
                    };

                    if changed {
                        state.hover = new_hover;
                        return EventResult::handled();
                    }
                }

                EventResult::default()
            }
            _ => EventResult::default(),
        }
    }
}
