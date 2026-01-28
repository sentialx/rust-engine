// Handler dispatch for CDP commands

pub mod dom;
pub mod css;
pub mod page;
pub mod input;
pub mod runtime;
pub mod overlay;

use serde_json::Value;

use crate::devtools_protocol::{DevtoolsServer, types::{Response, ERROR_METHOD_NOT_FOUND}};
use crate::frame::Frame;
use crate::renderer::HybridRenderer;

/// Dispatch a CDP method to the appropriate handler
pub fn dispatch(
    server: &mut DevtoolsServer,
    frame: &mut Frame,
    renderer: Option<&mut HybridRenderer>,
    id: u64,
    method: &str,
    params: &Value,
) -> Response {
    let parts: Vec<&str> = method.splitn(2, '.').collect();
    if parts.len() != 2 {
        return Response::error(id, ERROR_METHOD_NOT_FOUND, &format!("Unknown method: {}", method));
    }

    let domain = parts[0];
    let command = parts[1];

    match domain {
        "DOM" => dom::handle(server, frame, id, command, params),
        "CSS" => css::handle(server, frame, id, command, params),
        "Page" => page::handle(server, frame, renderer, id, command, params),
        "Input" => input::handle(server, frame, id, command, params),
        "Runtime" => runtime::handle(server, frame, id, command, params),
        "Overlay" => overlay::handle(server, frame, id, command, params),
        _ => Response::error(id, ERROR_METHOD_NOT_FOUND, &format!("Unknown domain: {}", domain)),
    }
}
