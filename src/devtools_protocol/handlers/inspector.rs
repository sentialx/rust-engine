// Inspector domain handlers (stub)

use serde_json::{json, Value};

use crate::devtools_protocol::types::Response;

/// Handle Inspector domain commands (stubs)
pub fn handle(id: u64, command: &str, _params: &Value) -> Response {
    match command {
        "enable" => Response::success(id, json!({})),
        "disable" => Response::success(id, json!({})),
        _ => Response::success(id, json!({})),
    }
}
