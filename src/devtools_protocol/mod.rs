// Chrome DevTools Protocol (CDP) compatible server for AI agents

pub mod types;
pub mod handlers;

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::rc::Rc;

use serde_json;

use crate::frame::Frame;
use crate::dom::DomElement;
use types::{Request, Response, ERROR_INTERNAL};

/// Node registry - maps stable u64 IDs to DOM elements
/// Allows protocol to reference nodes across multiple calls
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

/// DevTools Protocol server
pub struct DevtoolsServer {
    /// Node registry for stable node IDs
    pub registry: NodeRegistry,
    /// Document node ID (always 1)
    pub document_node_id: u64,
    /// Domain enable states
    enabled_domains: HashMap<String, bool>,
    /// Currently highlighted node (for overlay)
    highlighted_node_id: Option<u64>,
}

impl DevtoolsServer {
    pub fn new() -> Self {
        DevtoolsServer {
            registry: NodeRegistry::new(),
            document_node_id: 1,
            enabled_domains: HashMap::new(),
            highlighted_node_id: None,
        }
    }

    /// Get or create a node ID for an element
    pub fn get_or_create_node_id(&mut self, element: &Rc<RefCell<DomElement>>) -> u64 {
        self.registry.get_or_create_id(element)
    }

    /// Get an element by its node ID
    pub fn get_element_by_id(&self, id: u64) -> Option<Rc<RefCell<DomElement>>> {
        self.registry.get_by_id(id)
    }

    /// Clear the node registry (on page navigation)
    pub fn clear_registry(&mut self) {
        self.registry.clear();
        self.highlighted_node_id = None;
    }

    /// Set the highlighted node for overlay
    pub fn set_highlighted_node(&mut self, node_id: Option<u64>) {
        self.highlighted_node_id = node_id;
    }

    /// Get the currently highlighted node
    pub fn get_highlighted_node(&self) -> Option<Rc<RefCell<DomElement>>> {
        self.highlighted_node_id.and_then(|id| self.get_element_by_id(id))
    }

    /// Handle a single CDP request
    pub fn handle_request(&mut self, request: &Request, frame: &mut Frame) -> Response {
        handlers::dispatch(self, frame, request.id, &request.method, &request.params)
    }

    /// Run the server in stdio mode (for AI agent integration)
    /// Reads JSON commands from stdin, writes responses to stdout
    pub fn run_stdio(&mut self, frame: &mut Frame) -> io::Result<()> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        for line in stdin.lock().lines() {
            let line = line?;
            let line = line.trim();

            if line.is_empty() {
                continue;
            }

            let response = match serde_json::from_str::<Request>(line) {
                Ok(request) => self.handle_request(&request, frame),
                Err(e) => Response::error(0, ERROR_INTERNAL, &format!("JSON parse error: {}", e)),
            };

            let response_json = serde_json::to_string(&response)?;
            writeln!(stdout, "{}", response_json)?;
            stdout.flush()?;
        }

        Ok(())
    }

    /// Handle a single line of input (for testing/embedding)
    pub fn handle_line(&mut self, line: &str, frame: &mut Frame) -> String {
        let response = match serde_json::from_str::<Request>(line) {
            Ok(request) => self.handle_request(&request, frame),
            Err(e) => Response::error(0, ERROR_INTERNAL, &format!("JSON parse error: {}", e)),
        };

        serde_json::to_string(&response).unwrap_or_else(|_| r#"{"error":"serialization failed"}"#.to_string())
    }
}

impl Default for DevtoolsServer {
    fn default() -> Self {
        Self::new()
    }
}
