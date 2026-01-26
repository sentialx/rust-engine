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
            Response::success(id, json!({}))
        }

        "disable" => {
            server.set_highlighted_node(None);
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

        _ => Response::error(id, ERROR_METHOD_NOT_FOUND, &format!("Unknown Overlay method: {}", command)),
    }
}
