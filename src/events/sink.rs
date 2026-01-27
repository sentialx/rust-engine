use std::cell::RefCell;
use std::rc::Rc;

use crate::frame::Frame;
use crate::dom::DomEvent;

use super::{DefaultEventHandler, EventResult, InputEvent, InputEventKind};

pub trait EventHandler {
    /// Handle an event. The frame is passed as Rc<RefCell<Frame>> so handlers
    /// can borrow only when needed, allowing mark_dirty() to work during DOM modifications.
    fn handle(&mut self, event: &mut InputEvent, frame: &Rc<RefCell<Frame>>) -> EventResult;
}

pub struct EventSink {
    /// Handlers that run before the default handler (e.g., DevtoolsOverlayHandler)
    pre_handlers: Vec<Box<dyn EventHandler>>,
    /// Default handler for CSS hover state
    default_handler: DefaultEventHandler,
    frame: Rc<RefCell<Frame>>,
    scroll_y: f32,
}

impl EventSink {
    pub fn new(frame: Rc<RefCell<Frame>>) -> Self {
        Self {
            pre_handlers: Vec::new(),
            default_handler: DefaultEventHandler::new(),
            frame,
            scroll_y: 0.0,
        }
    }

    /// Insert a handler at the beginning (runs before default handler)
    pub fn prepend_handler(&mut self, handler: Box<dyn EventHandler>) {
        self.pre_handlers.insert(0, handler);
    }

    /// Access the frame
    pub fn frame(&self) -> &Rc<RefCell<Frame>> {
        &self.frame
    }

    /// Get current scroll offset
    pub fn scroll_y(&self) -> f32 {
        self.scroll_y
    }

    /// Clear hover state (e.g., on page refresh)
    pub fn clear_hover(&mut self) {
        self.default_handler.clear_hover();
    }

    /// Dispatch event through handlers, then default behavior
    pub fn dispatch(&mut self, kind: InputEventKind, x: f32, y: f32) -> EventResult {
        // Handle scroll events directly
        if let InputEventKind::Scroll { delta } = kind {
            return self.handle_scroll(delta);
        }

        let target = self.frame.borrow().hit_test(x, y);
        let mut event = InputEvent::new(kind, target, x, y);
        let mut result = EventResult::default();

        // Run pre-handlers (like DevtoolsOverlayHandler)
        for handler in &mut self.pre_handlers {
            let handler_result = handler.handle(&mut event, &self.frame);
            result.merge(handler_result);
            if event.is_stopped() {
                return result;
            }
        }

        // Run default handler (CSS hover state)
        let default_handler_result = self.default_handler.handle(&mut event, &self.frame);
        result.merge(default_handler_result);

        if event.is_stopped() {
            return result;
        }

        // Default behavior (ElementKind.on_click, etc.)
        if !event.is_default_prevented() {
            let default_result = self.run_default(&mut event);
            result.merge(default_result);
        }

        result
    }

    /// Handle scroll event
    fn handle_scroll(&mut self, delta: f32) -> EventResult {
        let frame = self.frame.borrow();
        let max_scroll = (frame.page_height - frame.viewport.height).max(0.0);
        drop(frame);

        let old_scroll = self.scroll_y;
        self.scroll_y = (self.scroll_y - delta).clamp(0.0, max_scroll);

        if (self.scroll_y - old_scroll).abs() > 0.001 {
            EventResult::handled()
        } else {
            EventResult::default()
        }
    }

    /// Run default element behavior (on_click for ElementKind)
    fn run_default(&mut self, event: &mut InputEvent) -> EventResult {
        if event.kind != InputEventKind::Click {
            return EventResult::default();
        }

        let Some(ref target) = event.target else {
            return EventResult::default();
        };

        let mut dom_event = DomEvent::new();

        {
            let mut node_ref = target.borrow_mut();
            let mut element_kind = std::mem::take(&mut node_ref.element_kind);
            element_kind.on_click(&mut node_ref, &mut dom_event);
            node_ref.element_kind = element_kind;
        }

        EventResult::handled()
    }
}
