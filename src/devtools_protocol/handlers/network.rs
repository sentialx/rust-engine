// Network domain handlers (stub)
// Returns empty responses so Chrome DevTools doesn't error

use serde_json::{json, Value};

use crate::devtools_protocol::types::{Response, ERROR_INTERNAL};

/// Handle Network domain commands (mostly stubs)
pub fn handle(id: u64, command: &str, _params: &Value) -> Response {
    match command {
        "enable" => Response::success(id, json!({})),
        "disable" => Response::success(id, json!({})),
        "setCacheDisabled" => Response::success(id, json!({})),
        "setUserAgentOverride" => Response::success(id, json!({})),
        "setExtraHTTPHeaders" => Response::success(id, json!({})),
        "getResponseBody" => Response::error(id, ERROR_INTERNAL, "Not implemented"),
        _ => Response::success(id, json!({})), // Accept unknown methods silently
    }
}
