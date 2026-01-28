// Target domain handlers (stub)
// Returns empty responses so Chrome DevTools doesn't error

use serde_json::{json, Value};

use crate::devtools_protocol::types::Response;

/// Handle Target domain commands (mostly stubs)
pub fn handle(id: u64, command: &str, params: &Value) -> Response {
    println!("Target.{}: {:?}", command, params);
    match command {
        "setAutoAttach" => {
            // DevTools expects us to auto-attach to targets
            // We only have one target, so just acknowledge
            Response::success(id, json!({}))
        }
        "setDiscoverTargets" => Response::success(id, json!({})),
        "setRemoteLocations" => Response::success(id, json!({})),
        "getTargetInfo" => Response::success(id, json!({
            "targetInfo": {
                "targetId": "graviton-main",
                "type": "page",
                "title": "Graviton",
                "url": "about:blank",
                "attached": true,
                "canAccessOpener": false
            }
        })),
        "getTargets" => Response::success(id, json!({
            "targetInfos": [{
                "targetId": "graviton-main",
                "type": "page",
                "title": "Graviton",
                "url": "about:blank",
                "attached": true,
                "canAccessOpener": false
            }]
        })),
        "attachToTarget" => {
            // Return a session ID for the target
            Response::success(id, json!({
                "sessionId": "graviton-session-1"
            }))
        }
        _ => Response::success(id, json!({})), // Accept unknown methods silently
    }
}
