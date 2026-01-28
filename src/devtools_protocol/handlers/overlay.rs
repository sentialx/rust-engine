// Overlay domain handlers - element highlighting

use serde_json::{json, Value};

use crate::devtools_protocol::{DevtoolsServer, types::*};
use crate::frame::Frame;

/// Handle Overlay domain commands
pub fn handle(
    server: &mut DevtoolsServer,
    _frame: &mut Frame,
    id: u64,
    command: &str,
    params: &Value,
) -> Response {
    match command {
        "enable" => {
            // Enable the devtools agent for overlay highlighting
            println!("Overlay.enable called");
            server.agent().borrow_mut().enable();
            Response::success(id, json!({}))
        }

        "disable" => {
            server.agent().borrow_mut().disable();
            Response::success(id, json!({}))
        }

        "highlightNode" => {
            // Get nodeId from highlightConfig or directly
            let node_id = params.get("nodeId")
                .and_then(|v| v.as_u64())
                .or_else(|| params.get("backendNodeId").and_then(|v| v.as_u64()));

            match node_id {
                Some(nid) => {
                    server.set_highlighted_node(Some(nid));
                    Response::success(id, json!({}))
                }
                None => {
                    // No nodeId means clear highlight
                    server.set_highlighted_node(None);
                    Response::success(id, json!({}))
                }
            }
        }

        "hideHighlight" => {
            server.set_highlighted_node(None);
            Response::success(id, json!({}))
        }

        "setInspectMode" => {
            // mode: "searchForNode" enables picker, "none" disables
            let mode = params.get("mode").and_then(|v| v.as_str()).unwrap_or("none");
            println!("Overlay.setInspectMode: mode={}", mode);
            let mut agent = server.agent().borrow_mut();

            let active = mode == "searchForNode" || mode == "searchForUAShadowDOM";
            agent.set_inspect_mode(active);
            Response::success(id, json!({}))
        }

        "setShowViewportSizeOnResize" |
        "setShowGridOverlays" |
        "setShowFlexOverlays" |
        "setShowScrollSnapOverlays" |
        "setShowContainerQueryOverlays" |
        "setShowIsolatedElements" |
        "setShowHinge" => {
            // Stub handlers for overlay features we don't support yet
            Response::success(id, json!({}))
        }

        _ => Response::error(id, ERROR_METHOD_NOT_FOUND, &format!("Unknown Overlay method: {}", command)),
    }
}
