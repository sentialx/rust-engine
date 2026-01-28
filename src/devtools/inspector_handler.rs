//! Element inspector event handler.
//!
//! Handles mouse events when element inspection is enabled:
//! - MouseMove: highlights hovered element
//! - Click: selects element and exits inspect mode

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
        {
            let agent = self.agent.borrow();
            // Only intercept events when inspect mode is active (picker enabled)
            if !agent.is_inspect_mode() {
                return EventResult::default();
            }
        }

        let result = match event.kind {
            InputEventKind::Click => {
                if let Some(ref target) = event.target {
                    let mut agent = self.agent.borrow_mut();
                    // Select the clicked element and exit inspect mode
                    agent.select_element(target);
                    agent.set_inspect_mode(false);
                }

                // Intercept the click - don't let it through to default behavior
                event.stop_propagation();
                event.prevent_default();

                EventResult::handled()
            }
            InputEventKind::MouseMove => {
                {
                    let mut agent = self.agent.borrow_mut();
                    // Update highlight on hover
                    if let Some(ref target) = event.target {
                        agent.highlight_element(target);
                    } else {
                        agent.hide_highlight();
                    }
                }
                // Borrow released, now flush
                self.flush_pending_notifications();
                EventResult::handled()
            }
            _ => EventResult::default(),
        };

        self.flush_pending_notifications();
        result
    }
}

impl ElementInspectorHandler {
    /// Flush pending notifications outside of agent borrow.
    fn flush_pending_notifications(&self) {
        let (pending, observers) = {
            let mut agent = self.agent.borrow_mut();
            (agent.take_pending_notification(), agent.collect_observers())
        };
        // Agent borrow released, safe to notify
        if pending {
            for observer in observers {
                observer.on_selection_changed();
            }
        }
    }
}
