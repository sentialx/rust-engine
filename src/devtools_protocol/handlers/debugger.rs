// Debugger domain handlers (stub)
// Returns empty responses so Chrome DevTools doesn't error

use serde_json::{json, Value};

use crate::devtools_protocol::types::Response;

/// Handle Debugger domain commands (mostly stubs)
pub fn handle(id: u64, command: &str, _params: &Value) -> Response {
    match command {
        "enable" => Response::success(id, json!({
            "debuggerId": "graviton-debugger"
        })),
        "disable" => Response::success(id, json!({})),
        "setAsyncCallStackDepth" => Response::success(id, json!({})),
        "setBlackboxPatterns" => Response::success(id, json!({})),
        "setPauseOnExceptions" => Response::success(id, json!({})),
        _ => Response::success(id, json!({})), // Accept unknown methods silently
    }
}
