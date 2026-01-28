mod router;
mod sink;
mod handlers;

pub use router::{EventRouter, FrameRegion};
pub use sink::{EventSink, EventHandler};
pub use handlers::DefaultEventHandler;

use std::cell::RefCell;
use std::rc::Rc;

use crate::dom::DomElement;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputEventKind {
    Click,
    MouseMove,
    MouseDown,
    MouseUp,
    Scroll { delta: f32 },
}

pub struct InputEvent {
    pub kind: InputEventKind,
    pub target: Option<Rc<RefCell<DomElement>>>,
    pub x: f32,
    pub y: f32,
    stopped: bool,
    default_prevented: bool,
}

impl InputEvent {
    pub fn new(kind: InputEventKind, target: Option<Rc<RefCell<DomElement>>>, x: f32, y: f32) -> Self {
        Self {
            kind,
            target,
            x,
            y,
            stopped: false,
            default_prevented: false,
        }
    }

    pub fn stop_propagation(&mut self) {
        self.stopped = true;
    }

    pub fn prevent_default(&mut self) {
        self.default_prevented = true;
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    pub fn is_default_prevented(&self) -> bool {
        self.default_prevented
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EventResult {
    pub handled: bool,
}

impl EventResult {
    pub fn handled() -> Self {
        Self { handled: true }
    }

    pub fn merge(&mut self, other: EventResult) {
        self.handled = self.handled || other.handled;
    }
}
