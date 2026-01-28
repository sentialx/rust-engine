//! DevtoolsAgent - Central coordination point for all devtools functionality.
//!
//! The agent owns:
//! - NodeRegistry for stable node IDs
//! - Overlay state (enabled, selected element)
//! - Inspect mode (whether browser intercepts events for picking)
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
    /// Called when the selected element changes.
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
/// - Overlay state (enabled, highlight, selection)
/// - Inspect mode (whether browser events are intercepted for picking)
///
/// Highlight vs Selection:
/// - Highlight: temporary, shown during hover in inspect mode, clears on mouse leave
/// - Selection: persistent, set when user clicks to select an element
///
/// Provides clean Rust API for:
/// - Overlay control (enable/disable, highlight/select)
/// - Inspect mode control (for element picker)
/// - Node registry operations
pub struct DevtoolsAgent {
    /// Weak reference to the inspected frame
    frame: Weak<RefCell<Frame>>,

    /// Node registry for stable node IDs
    registry: NodeRegistry,

    /// Document node ID (always 1)
    document_node_id: u64,

    /// Whether devtools overlay is enabled (visible)
    enabled: bool,

    /// Whether inspect mode is active (browser intercepts events for element picking)
    inspect_mode: bool,

    /// Temporary highlight during hover (clears on mouse leave)
    highlighted_node_id: Option<u64>,

    /// Persistent selection (set when user clicks)
    selected_node_id: Option<u64>,

    /// Selection change observers
    observers: Vec<Weak<dyn SelectionObserver>>,

    /// Pending notification (deferred to avoid borrow conflicts)
    notification_pending: bool,

    /// Last selection sent to CDP (for change detection)
    last_cdp_selection: Option<u64>,
}

impl DevtoolsAgent {
    /// Create a new DevtoolsAgent for inspecting a frame.
    pub fn new(frame: Weak<RefCell<Frame>>) -> Self {
        Self {
            frame,
            registry: NodeRegistry::new(),
            document_node_id: 1,
            enabled: false,
            inspect_mode: false,
            highlighted_node_id: None,
            selected_node_id: None,
            observers: Vec::new(),
            notification_pending: false,
            last_cdp_selection: None,
        }
    }

    // --- Observer API ---

    /// Add a selection observer. The observer is held weakly to avoid cycles.
    pub fn add_observer(&mut self, observer: Weak<dyn SelectionObserver>) {
        self.observers.push(observer);
    }

    /// Mark that observers need to be notified (deferred to avoid borrow conflicts).
    fn notify_selection_changed(&mut self) {
        self.notification_pending = true;
    }

    /// Check if there are pending notifications.
    pub fn take_pending_notification(&mut self) -> bool {
        let pending = self.notification_pending;
        self.notification_pending = false;
        pending
    }

    /// Get observers to notify (removes dead ones). Call outside of agent borrow.
    pub fn collect_observers(&mut self) -> Vec<Rc<dyn SelectionObserver>> {
        let mut live = Vec::new();
        self.observers.retain(|weak| {
            if let Some(observer) = weak.upgrade() {
                live.push(observer);
                true
            } else {
                false
            }
        });
        live
    }

    /// Check if selection changed since last CDP notification.
    /// Returns Some(node_id) if changed, None if same.
    pub fn take_cdp_selection_change(&mut self) -> Option<u64> {
        let current = self.get_selected_node_id();
        if current != self.last_cdp_selection {
            self.last_cdp_selection = current;
            current
        } else {
            None
        }
    }

    // --- Overlay API ---

    /// Enable the devtools overlay (show element highlights).
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable the devtools overlay.
    pub fn disable(&mut self) {
        let had_highlight = self.highlighted_node_id.is_some();
        self.enabled = false;
        self.highlighted_node_id = None;
        // Keep selected_node_id so it can be restored on re-enable
        if had_highlight {
            self.notify_selection_changed();
        }
    }

    /// Check if devtools overlay is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    // --- Inspect Mode API ---

    /// Enable inspect mode (element picker - browser intercepts events).
    pub fn set_inspect_mode(&mut self, active: bool) {
        self.inspect_mode = active;
        // When entering inspect mode, also enable overlay
        if active {
            self.enabled = true;
        }
    }

    /// Check if inspect mode is active (browser intercepts events for picking).
    pub fn is_inspect_mode(&self) -> bool {
        self.inspect_mode
    }

    // --- Highlight API (temporary visual, during hover) ---

    /// Highlight a node temporarily (hover). Clears on hide_highlight.
    pub fn highlight_node(&mut self, node_id: u64) {
        let old = self.highlighted_node_id;
        self.highlighted_node_id = Some(node_id);
        if self.highlighted_node_id != old {
            self.notify_selection_changed();
        }
    }

    /// Highlight a node by element reference.
    pub fn highlight_element(&mut self, element: &Rc<RefCell<DomElement>>) {
        let node_id = self.registry.get_or_create_id(element);
        self.highlight_node(node_id);
    }

    /// Clear the temporary highlight (mouse leave).
    pub fn hide_highlight(&mut self) {
        if self.highlighted_node_id.is_some() {
            self.highlighted_node_id = None;
            self.notify_selection_changed();
        }
    }

    // --- Selection API (persistent state for DevTools, not visible) ---

    /// Select a node persistently (click to select). Updates DevTools state.
    pub fn select_node(&mut self, node_id: u64) {
        self.selected_node_id = Some(node_id);
        // Clear highlight when selecting (mouse click ends hover)
        self.highlighted_node_id = None;
        self.notify_selection_changed();
    }

    /// Select an element by reference.
    pub fn select_element(&mut self, element: &Rc<RefCell<DomElement>>) {
        let node_id = self.registry.get_or_create_id(element);
        self.select_node(node_id);
    }

    /// Clear the persistent selection.
    pub fn clear_selection(&mut self) {
        if self.selected_node_id.is_some() {
            self.selected_node_id = None;
            self.notify_selection_changed();
        }
    }

    // --- Visibility API ---

    /// Get the element to show in the overlay (only the hover highlight).
    /// Selection is just state for DevTools, not visible in the browser.
    pub fn get_highlighted_element(&self) -> Option<Rc<RefCell<DomElement>>> {
        if !self.enabled {
            return None;
        }
        self.highlighted_node_id
            .and_then(|id| self.registry.get_by_id(id))
    }

    /// Alias for get_highlighted_element (for overlay rendering).
    pub fn get_selected_element(&self) -> Option<Rc<RefCell<DomElement>>> {
        self.get_highlighted_element()
    }

    /// Get the currently selected node ID (persistent state for DevTools).
    pub fn get_selected_node_id(&self) -> Option<u64> {
        if !self.enabled {
            return None;
        }
        // Return selection if set, otherwise current highlight
        self.selected_node_id.or(self.highlighted_node_id)
    }

    /// Get just the highlight node ID (for change detection).
    pub fn get_highlighted_node_id(&self) -> Option<u64> {
        if !self.enabled {
            return None;
        }
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
        let had_state = self.highlighted_node_id.is_some() || self.selected_node_id.is_some();
        self.registry.clear();
        self.highlighted_node_id = None;
        self.selected_node_id = None;
        if had_state {
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

}
