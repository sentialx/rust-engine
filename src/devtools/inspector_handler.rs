//! Element inspector event handler.
//!
//! Handles mouse events when element inspection is enabled:
//! - MouseMove: highlights hovered element
//! - Click: toggles pin on clicked element

use std::cell::RefCell;
use std::rc::Rc;

use crate::events::{EventHandler, EventResult, InputEvent, InputEventKind};
use crate::frame::Frame;

use super::DevtoolsAgent;

/// Handler for element inspector interactions.
/// Installed on the main frame's EventSink when devtools is active.
pub struct ElementInspectorHandler {
    agent: Rc<RefCell<DevtoolsAgent>>,
}

impl ElementInspectorHandler {
    pub fn new(agent: Rc<RefCell<DevtoolsAgent>>) -> Self {
        Self { agent }
    }
}

impl EventHandler for ElementInspectorHandler {
    fn handle(&mut self, event: &mut InputEvent, _frame: &Rc<RefCell<Frame>>) -> EventResult {
        let agent = self.agent.borrow();
        if !agent.is_enabled() {
            return EventResult::default();
        }
        drop(agent);

        match event.kind {
            InputEventKind::Click => {
                if let Some(ref target) = event.target {
                    let mut agent = self.agent.borrow_mut();
                    agent.toggle_pin_element(target);
                }

                // Intercept the click - don't let it through to default behavior
                event.stop_propagation();
                event.prevent_default();

                EventResult::handled()
            }
            InputEventKind::MouseMove => {
                let mut agent = self.agent.borrow_mut();

                // Only update hover if not pinned
                if !agent.is_pinned() {
                    if let Some(ref target) = event.target {
                        agent.highlight_element(target);
                    } else {
                        agent.hide_highlight();
                    }
                    return EventResult::handled();
                }

                EventResult::default()
            }
            _ => EventResult::default(),
        }
    }
}
