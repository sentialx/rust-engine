// Runtime domain handlers

use serde_json::{json, Value};

use crate::devtools_protocol::{DevtoolsServer, types::*};
use crate::frame::Frame;

/// Handle Runtime domain commands
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

        "evaluate" => {
            let expression = match params.get("expression").and_then(|v| v.as_str()) {
                Some(e) => e,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Missing expression parameter"),
            };

            // We don't have a JavaScript engine, so we can only handle
            // simple document queries that map to our DOM API

            let result = evaluate_simple_expression(server, frame, expression);

            Response::success(id, json!({
                "result": {
                    "type": result.0,
                    "value": result.1
                }
            }))
        }

        "getProperties" => {
            // Return empty properties since we don't have JS objects
            Response::success(id, json!({
                "result": []
            }))
        }

        _ => Response::error(id, ERROR_METHOD_NOT_FOUND, &format!("Unknown Runtime method: {}", command)),
    }
}

/// Evaluate simple expressions without a JS engine
/// Supports basic document queries
fn evaluate_simple_expression(
    server: &mut DevtoolsServer,
    frame: &Frame,
    expression: &str,
) -> (&'static str, Value) {
    let expr = expression.trim();

    // Handle document.title
    if expr == "document.title" {
        // Look for <title> tag
        for element in &frame.dom_tree {
            if let Some(title) = find_title_text(element) {
                return ("string", json!(title));
            }
        }
        return ("string", json!(""));
    }

    // Handle document.URL or document.location.href
    if expr == "document.URL" || expr == "document.location.href" || expr == "location.href" {
        return ("string", json!(frame.url));
    }

    // Handle document.body
    if expr == "document.body" {
        for element in &frame.dom_tree {
            let el = element.borrow();
            if el.tag_name == "BODY" {
                let node_id = server.get_or_create_node_id(element);
                return ("object", json!({
                    "type": "node",
                    "nodeId": node_id,
                    "tagName": "BODY"
                }));
            }
            // Search in children
            for child in &el.children {
                let child_el = child.borrow();
                if child_el.tag_name == "BODY" {
                    drop(child_el);
                    let node_id = server.get_or_create_node_id(child);
                    return ("object", json!({
                        "type": "node",
                        "nodeId": node_id,
                        "tagName": "BODY"
                    }));
                }
            }
        }
        return ("undefined", json!(null));
    }

    // Handle document.getElementById
    if expr.starts_with("document.getElementById(") {
        if let Some(id) = extract_string_arg(expr, "document.getElementById(") {
            if let Some(element) = frame.get_element_by_id(&id) {
                let node_id = server.get_or_create_node_id(&element);
                let tag = element.borrow().tag_name.clone();
                return ("object", json!({
                    "type": "node",
                    "nodeId": node_id,
                    "tagName": tag
                }));
            }
        }
        return ("undefined", json!(null));
    }

    // Handle simple number/string/boolean literals
    if let Ok(num) = expr.parse::<f64>() {
        return ("number", json!(num));
    }

    if expr == "true" {
        return ("boolean", json!(true));
    }

    if expr == "false" {
        return ("boolean", json!(false));
    }

    if expr == "null" {
        return ("object", json!(null));
    }

    if expr == "undefined" {
        return ("undefined", json!(null));
    }

    // String literal
    if (expr.starts_with('"') && expr.ends_with('"')) || (expr.starts_with('\'') && expr.ends_with('\'')) {
        let s = &expr[1..expr.len()-1];
        return ("string", json!(s));
    }

    // Unknown expression
    ("undefined", json!(null))
}

/// Extract a string argument from a function call like getElementById("foo")
fn extract_string_arg(expr: &str, prefix: &str) -> Option<String> {
    let rest = expr.strip_prefix(prefix)?;
    let rest = rest.strip_suffix(')')?;
    let rest = rest.trim();

    if (rest.starts_with('"') && rest.ends_with('"')) || (rest.starts_with('\'') && rest.ends_with('\'')) {
        Some(rest[1..rest.len()-1].to_string())
    } else {
        None
    }
}

/// Find the text content of a <title> element
fn find_title_text(element: &std::rc::Rc<std::cell::RefCell<crate::html::DomElement>>) -> Option<String> {
    let el = element.borrow();

    if el.tag_name == "TITLE" {
        // Get text content from children
        for child in &el.children {
            let child_el = child.borrow();
            if child_el.node_type == crate::html::NodeType::Text {
                return Some(child_el.node_value.clone());
            }
        }
        return Some(String::new());
    }

    // Search children
    for child in &el.children {
        if let Some(title) = find_title_text(child) {
            return Some(title);
        }
    }

    None
}
