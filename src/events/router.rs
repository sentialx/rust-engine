use std::cell::RefCell;
use std::rc::Rc;

use crate::layout::Rect;

use super::{EventResult, EventSink, InputEventKind};

pub struct FrameRegion {
    pub bounds: Rect,
    pub sink: Rc<RefCell<EventSink>>,
}

impl FrameRegion {
    pub fn new(bounds: Rect, sink: Rc<RefCell<EventSink>>) -> Self {
        Self { bounds, sink }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.bounds.x
            && x < self.bounds.x + self.bounds.width
            && y >= self.bounds.y
            && y < self.bounds.y + self.bounds.height
    }

    pub fn to_local(&self, x: f32, y: f32) -> (f32, f32) {
        let local_x = x - self.bounds.x;
        let local_y = y - self.bounds.y;
        (local_x, local_y)
    }
}

pub struct EventRouter {
    regions: Vec<FrameRegion>,
}

impl EventRouter {
    pub fn new() -> Self {
        Self { regions: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.regions.clear();
    }

    pub fn add_region(&mut self, region: FrameRegion) {
        self.regions.push(region);
    }

    pub fn remove_region(&mut self, sink: &Rc<RefCell<EventSink>>) {
        self.regions.retain(|r| !Rc::ptr_eq(&r.sink, sink));
    }

    /// Routes event to appropriate frame based on coordinates
    pub fn dispatch(&mut self, kind: InputEventKind, x: f32, y: f32) -> EventResult {
        for region in &self.regions {
            if region.contains(x, y) {
                let (local_x, local_y) = region.to_local(x, y);

                // Add scroll offset for non-scroll events (scroll is in screen space)
                let scroll_y = region.sink.borrow().scroll_y();
                let page_y = match kind {
                    InputEventKind::Scroll { .. } => local_y,
                    _ => local_y + scroll_y,
                };

                return region.sink.borrow_mut().dispatch(kind, local_x, page_y);
            }
        }
        EventResult::default()
    }

    /// Update bounds for a specific region by sink reference
    pub fn update_bounds(&mut self, sink: &Rc<RefCell<EventSink>>, bounds: Rect) {
        for region in &mut self.regions {
            if Rc::ptr_eq(&region.sink, sink) {
                region.bounds = bounds;
                return;
            }
        }
    }
}

impl Default for EventRouter {
    fn default() -> Self {
        Self::new()
    }
}
