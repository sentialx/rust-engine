// Input domain handlers

use serde_json::{json, Value};

use crate::devtools_protocol::{DevtoolsServer, types::*};
use crate::frame::Frame;
use crate::html::DomElement;

/// Handle Input domain commands
pub fn handle(
    server: &mut DevtoolsServer,
    frame: &mut Frame,
    id: u64,
    command: &str,
    params: &Value,
) -> Response {
    match command {
        "enable" => {
            Response::success(id, json!({}))
        }

        "disable" => {
            Response::success(id, json!({}))
        }

        "dispatchMouseEvent" => {
            let event_type = match params.get("type").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Missing type parameter"),
            };

            let x = params.get("x")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as f32;

            let y = params.get("y")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as f32;

            match event_type {
                "mousePressed" | "mouseReleased" | "click" => {
                    // Perform hit test to find element at coordinates
                    if let Some(element) = frame.hit_test(x, y) {
                        let mut el = element.borrow_mut();

                        // Update hover state
                        if event_type == "mousePressed" {
                            el.pseudo_classes.hover = true;
                        }

                        // For click, we could trigger events if we had an event system
                        // For now, just acknowledge the input
                    }
                }
                "mouseMoved" => {
                    // Update hover state using frame.set_hover which triggers restyle
                    let element = frame.hit_test(x, y);
                    frame.set_hover(element.clone());

                    // Return the hovered element's nodeId so caller can use Overlay.highlightNode
                    if let Some(ref el) = element {
                        let node_id = server.get_or_create_node_id(el);
                        return Response::success(id, json!({ "nodeId": node_id }));
                    }
                }
                _ => {}
            }

            Response::success(id, json!({}))
        }

        "dispatchKeyEvent" => {
            let event_type = match params.get("type").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Missing type parameter"),
            };

            let key = params.get("key")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let code = params.get("code")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let text = params.get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // Key event handling
            // Currently no-op since we don't have a focus/text input system
            // but we acknowledge the event

            Response::success(id, json!({}))
        }

        _ => Response::error(id, ERROR_METHOD_NOT_FOUND, &format!("Unknown Input method: {}", command)),
    }
}
