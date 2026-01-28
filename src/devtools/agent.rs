//! DevtoolsAgent - Central coordination point for all devtools functionality.
//!
//! The agent owns:
//! - NodeRegistry for stable node IDs
//! - Overlay state (enabled, highlighted, pinned)
//!
//! Internal components call agent methods directly.
//! External clients (Chrome DevTools) access via DevtoolsServer which translates CDP JSON.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use crate::dom::DomElement;
use crate::frame::Frame;

/// Observer trait for selection changes.
/// Implement this to be notified when the agent's selection state changes.
/// Uses `&self` so observers can use interior mutability (Cell) without RefCell.
pub trait SelectionObserver {
    /// Called when the selected element changes (highlight or pin).
    fn on_selection_changed(&self);
}

/// Node registry - maps stable u64 IDs to DOM elements.
/// Allows components to reference nodes across multiple calls.
pub struct NodeRegistry {
    /// Map from node ID to element reference
    id_to_element: HashMap<u64, Rc<RefCell<DomElement>>>,
    /// Map from element pointer to node ID (for reverse lookup)
    element_to_id: HashMap<*const RefCell<DomElement>, u64>,
    /// Next available node ID
    next_id: u64,
}

impl NodeRegistry {
    pub fn new() -> Self {
        NodeRegistry {
            id_to_element: HashMap::new(),
            element_to_id: HashMap::new(),
            next_id: 2, // Start at 2, document node is 1
        }
    }

    /// Get or create a node ID for an element
    pub fn get_or_create_id(&mut self, element: &Rc<RefCell<DomElement>>) -> u64 {
        let ptr = Rc::as_ptr(element);
        if let Some(&id) = self.element_to_id.get(&ptr) {
            return id;
        }

        let id = self.next_id;
        self.next_id += 1;
        self.id_to_element.insert(id, element.clone());
        self.element_to_id.insert(ptr, id);
        id
    }

    /// Get an element by its node ID
    pub fn get_by_id(&self, id: u64) -> Option<Rc<RefCell<DomElement>>> {
        self.id_to_element.get(&id).cloned()
    }

    /// Clear the registry (e.g., on page navigation)
    pub fn clear(&mut self) {
        self.id_to_element.clear();
        self.element_to_id.clear();
        self.next_id = 2;
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// DevtoolsAgent - Central coordination point for devtools functionality.
///
/// Owns all devtools state:
/// - Node registry for stable IDs
/// - Overlay state (enabled, highlighted node, pinned node)
///
/// Provides clean Rust API for:
/// - Overlay control (enable/disable, highlight, pin)
/// - Node registry operations
/// - DOM/CSS queries (for panel)
pub struct DevtoolsAgent {
    /// Weak reference to the inspected frame
    frame: Weak<RefCell<Frame>>,

    /// Node registry for stable node IDs
    registry: NodeRegistry,

    /// Document node ID (always 1)
    document_node_id: u64,

    /// Whether devtools overlay is enabled (visible)
    enabled: bool,

    /// Currently highlighted node ID (from hover or CDP)
    highlighted_node_id: Option<u64>,

    /// Pinned node ID (click to lock selection)
    pinned_node_id: Option<u64>,

    /// Selection change observers
    observers: Vec<Weak<dyn SelectionObserver>>,
}

impl DevtoolsAgent {
    /// Create a new DevtoolsAgent for inspecting a frame.
    pub fn new(frame: Weak<RefCell<Frame>>) -> Self {
        Self {
            frame,
            registry: NodeRegistry::new(),
            document_node_id: 1,
            enabled: false,
            highlighted_node_id: None,
            pinned_node_id: None,
            observers: Vec::new(),
        }
    }

    // --- Observer API ---

    /// Add a selection observer. The observer is held weakly to avoid cycles.
    pub fn add_observer(&mut self, observer: Weak<dyn SelectionObserver>) {
        self.observers.push(observer);
    }

    /// Notify all observers of a selection change.
    fn notify_selection_changed(&mut self) {
        // Clean up dead observers and notify live ones
        self.observers.retain(|weak| {
            if let Some(observer) = weak.upgrade() {
                observer.on_selection_changed();
                true
            } else {
                false
            }
        });
    }

    // --- Overlay API ---

    /// Enable the devtools overlay (show element highlights).
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable the devtools overlay.
    pub fn disable(&mut self) {
        let had_selection = self.get_selected_node_id().is_some();
        self.enabled = false;
        self.highlighted_node_id = None;
        // Note: pinned state is preserved so it can be restored on re-enable
        if had_selection {
            self.notify_selection_changed();
        }
    }

    /// Check if devtools overlay is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Highlight a node by its ID (from hover or CDP).
    pub fn highlight_node(&mut self, node_id: u64) {
        let old = self.get_selected_node_id();
        self.highlighted_node_id = Some(node_id);
        if self.get_selected_node_id() != old {
            self.notify_selection_changed();
        }
    }

    /// Highlight a node by element reference.
    pub fn highlight_element(&mut self, element: &Rc<RefCell<DomElement>>) {
        let node_id = self.registry.get_or_create_id(element);
        let old = self.get_selected_node_id();
        self.highlighted_node_id = Some(node_id);
        if self.get_selected_node_id() != old {
            self.notify_selection_changed();
        }
    }

    /// Hide the highlight (clear highlighted node).
    pub fn hide_highlight(&mut self) {
        let old = self.get_selected_node_id();
        self.highlighted_node_id = None;
        if self.get_selected_node_id() != old {
            self.notify_selection_changed();
        }
    }

    /// Pin a node by its ID.
    pub fn pin_node(&mut self, node_id: u64) {
        let old = self.get_selected_node_id();
        self.pinned_node_id = Some(node_id);
        if self.get_selected_node_id() != old {
            self.notify_selection_changed();
        }
    }

    /// Pin a node by element reference.
    pub fn pin_element(&mut self, element: &Rc<RefCell<DomElement>>) {
        let node_id = self.registry.get_or_create_id(element);
        let old = self.get_selected_node_id();
        self.pinned_node_id = Some(node_id);
        if self.get_selected_node_id() != old {
            self.notify_selection_changed();
        }
    }

    /// Unpin the currently pinned node.
    pub fn unpin(&mut self) {
        let old = self.get_selected_node_id();
        self.pinned_node_id = None;
        if self.get_selected_node_id() != old {
            self.notify_selection_changed();
        }
    }

    /// Toggle pin: pin if different node, unpin if same node.
    pub fn toggle_pin(&mut self, node_id: u64) {
        let old = self.get_selected_node_id();
        if self.pinned_node_id == Some(node_id) {
            self.pinned_node_id = None;
        } else {
            self.pinned_node_id = Some(node_id);
        }
        if self.get_selected_node_id() != old {
            self.notify_selection_changed();
        }
    }

    /// Toggle pin by element reference.
    pub fn toggle_pin_element(&mut self, element: &Rc<RefCell<DomElement>>) {
        let node_id = self.registry.get_or_create_id(element);
        self.toggle_pin(node_id);
    }

    /// Returns the element to highlight (pinned takes priority over highlighted).
    /// Returns None if devtools is not enabled.
    pub fn get_selected_element(&self) -> Option<Rc<RefCell<DomElement>>> {
        if !self.enabled {
            return None;
        }
        // Pinned takes priority
        self.pinned_node_id
            .and_then(|id| self.registry.get_by_id(id))
            .or_else(|| {
                self.highlighted_node_id
                    .and_then(|id| self.registry.get_by_id(id))
            })
    }

    /// Get the currently selected node ID (pinned or highlighted).
    pub fn get_selected_node_id(&self) -> Option<u64> {
        if !self.enabled {
            return None;
        }
        self.pinned_node_id.or(self.highlighted_node_id)
    }

    /// Check if a node is currently pinned.
    pub fn is_pinned(&self) -> bool {
        self.pinned_node_id.is_some()
    }

    /// Get the pinned node ID.
    pub fn pinned_node_id(&self) -> Option<u64> {
        self.pinned_node_id
    }

    /// Get the highlighted node ID.
    pub fn highlighted_node_id(&self) -> Option<u64> {
        self.highlighted_node_id
    }

    // --- Node Registry API ---

    /// Get or create a stable node ID for an element.
    pub fn get_or_create_node_id(&mut self, element: &Rc<RefCell<DomElement>>) -> u64 {
        self.registry.get_or_create_id(element)
    }

    /// Get an element by its node ID.
    pub fn get_element_by_id(&self, node_id: u64) -> Option<Rc<RefCell<DomElement>>> {
        self.registry.get_by_id(node_id)
    }

    /// Clear the node registry (on page navigation).
    pub fn clear_registry(&mut self) {
        let had_selection = self.get_selected_node_id().is_some();
        self.registry.clear();
        self.highlighted_node_id = None;
        self.pinned_node_id = None;
        if had_selection {
            self.notify_selection_changed();
        }
    }

    /// Get the document node ID (always 1).
    pub fn document_node_id(&self) -> u64 {
        self.document_node_id
    }

    // --- Frame Access ---

    /// Get a reference to the inspected frame.
    pub fn frame(&self) -> Option<Rc<RefCell<Frame>>> {
        self.frame.upgrade()
    }

    /// Set the frame to inspect.
    pub fn set_frame(&mut self, frame: Weak<RefCell<Frame>>) {
        self.frame = frame;
    }

    // --- Selection clearing ---

    /// Clear all selection state (hover and pin).
    pub fn clear_selection(&mut self) {
        let had_selection = self.get_selected_node_id().is_some();
        self.highlighted_node_id = None;
        self.pinned_node_id = None;
        if had_selection {
            self.notify_selection_changed();
        }
    }
}
