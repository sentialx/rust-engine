// Chrome DevTools Protocol (CDP) compatible server
//
// The DevtoolsServer acts as a CDP adapter that translates CDP JSON requests
// into method calls on DevtoolsAgent. This provides a clean separation between
// the protocol layer and the underlying devtools functionality.

pub mod types;
pub mod handlers;
pub mod websocket;

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::rc::Rc;

use serde_json;

use crate::devtools::DevtoolsAgent;
use crate::frame::Frame;
use crate::dom::DomElement;
use crate::renderer::HybridRenderer;
use types::{Request, Response, ERROR_INTERNAL};

/// DevTools Protocol server - CDP adapter for DevtoolsAgent
///
/// Translates CDP JSON requests into method calls on the agent.
/// Internal components should use DevtoolsAgent directly; this is for
/// external clients (Chrome DevTools, AI agents) that speak CDP.
pub struct DevtoolsServer {
    /// Reference to the DevtoolsAgent
    agent: Rc<RefCell<DevtoolsAgent>>,
    /// Document node ID (always 1)
    pub document_node_id: u64,
    /// Domain enable states
    enabled_domains: HashMap<String, bool>,
}

impl DevtoolsServer {
    /// Create a new DevtoolsServer that wraps a DevtoolsAgent
    pub fn new(agent: Rc<RefCell<DevtoolsAgent>>) -> Self {
        DevtoolsServer {
            agent,
            document_node_id: 1,
            enabled_domains: HashMap::new(),
        }
    }

    /// Get the DevtoolsAgent
    pub fn agent(&self) -> &Rc<RefCell<DevtoolsAgent>> {
        &self.agent
    }

    /// Get or create a node ID for an element (delegates to agent)
    pub fn get_or_create_node_id(&mut self, element: &Rc<RefCell<DomElement>>) -> u64 {
        self.agent.borrow_mut().get_or_create_node_id(element)
    }

    /// Get an element by its node ID (delegates to agent)
    pub fn get_element_by_id(&self, id: u64) -> Option<Rc<RefCell<DomElement>>> {
        self.agent.borrow().get_element_by_id(id)
    }

    /// Clear the node registry (on page navigation)
    pub fn clear_registry(&mut self) {
        self.agent.borrow_mut().clear_registry();
    }

    /// Set the highlighted node for overlay (delegates to agent)
    pub fn set_highlighted_node(&mut self, node_id: Option<u64>) {
        let mut agent = self.agent.borrow_mut();
        if let Some(id) = node_id {
            agent.highlight_node(id);
        } else {
            agent.hide_highlight();
        }
    }

    /// Get the currently selected/highlighted node
    pub fn get_highlighted_node(&self) -> Option<Rc<RefCell<DomElement>>> {
        let agent = self.agent.borrow();
        agent.get_selected_node_id()
            .and_then(|id| agent.get_element_by_id(id))
    }

    /// Handle a single CDP request
    pub fn handle_request(&mut self, request: &Request, frame: &mut Frame, renderer: Option<&mut HybridRenderer>) -> Response {
        handlers::dispatch(self, frame, renderer, request.id, &request.method, &request.params)
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
                Ok(request) => self.handle_request(&request, frame, None),
                Err(e) => Response::error(0, ERROR_INTERNAL, &format!("JSON parse error: {}", e)),
            };

            let response_json = serde_json::to_string(&response)?;
            writeln!(stdout, "{}", response_json)?;
            stdout.flush()?;
        }

        Ok(())
    }

    /// Handle a single line of input (for testing/embedding)
    pub fn handle_line(&mut self, line: &str, frame: &mut Frame, renderer: Option<&mut HybridRenderer>) -> String {
        let response = match serde_json::from_str::<Request>(line) {
            Ok(request) => self.handle_request(&request, frame, renderer),
            Err(e) => Response::error(0, ERROR_INTERNAL, &format!("JSON parse error: {}", e)),
        };

        serde_json::to_string(&response).unwrap_or_else(|_| r#"{"error":"serialization failed"}"#.to_string())
    }
}
