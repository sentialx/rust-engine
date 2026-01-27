// DOM domain handlers

use std::cell::RefCell;
use std::rc::Rc;
use serde_json::{json, Value};

use crate::devtools_protocol::{DevtoolsServer, types::*};
use crate::frame::Frame;
use crate::dom::{DomElement, NodeType};

/// Convert a DomElement to a CDP Node
fn element_to_node(
    server: &mut DevtoolsServer,
    element: &Rc<RefCell<DomElement>>,
    depth: i32,
    current_depth: i32,
) -> Node {
    let node_id = server.get_or_create_node_id(element);
    let el = element.borrow();

    let node_type = match el.node_type {
        NodeType::Element => NODE_TYPE_ELEMENT,
        NodeType::Text => NODE_TYPE_TEXT,
        NodeType::Comment => NODE_TYPE_COMMENT,
        NodeType::DocumentType => NODE_TYPE_DOCUMENT_TYPE,
    };

    let node_name = match el.node_type {
        NodeType::Element => el.tag_name.clone(),
        NodeType::Text => "#text".to_string(),
        NodeType::Comment => "#comment".to_string(),
        NodeType::DocumentType => el.node_value.clone(),
    };

    let local_name = match el.node_type {
        NodeType::Element => el.tag_name.to_lowercase(),
        _ => String::new(),
    };

    let node_value = match el.node_type {
        NodeType::Text | NodeType::Comment => el.node_value.clone(),
        _ => String::new(),
    };

    // Build flat attributes array [name1, val1, name2, val2, ...]
    let attributes = if el.node_type == NodeType::Element {
        let mut attrs = Vec::new();
        for (key, value) in &el.attributes {
            attrs.push(key.clone());
            attrs.push(value.clone());
        }
        Some(attrs)
    } else {
        None
    };

    let child_count = el.children.len() as u32;

    // Build children if within depth
    let children = if depth < 0 || current_depth < depth {
        let children_refs: Vec<Rc<RefCell<DomElement>>> = el.children.clone();
        drop(el); // Release borrow before recursing

        let child_nodes: Vec<Node> = children_refs
            .iter()
            .map(|child| element_to_node(server, child, depth, current_depth + 1))
            .collect();

        if child_nodes.is_empty() {
            None
        } else {
            Some(child_nodes)
        }
    } else {
        None
    };

    Node {
        node_id,
        backend_node_id: node_id,
        node_type,
        node_name,
        local_name,
        node_value,
        child_node_count: child_count,
        children,
        attributes,
    }
}

/// Create a document node wrapping the DOM tree
fn create_document_node(
    server: &mut DevtoolsServer,
    frame: &Frame,
    depth: i32,
) -> Node {
    let doc_id = server.document_node_id;

    let children = if depth != 0 {
        let child_nodes: Vec<Node> = frame.dom_tree
            .iter()
            .map(|el| element_to_node(server, el, depth, 1))
            .collect();

        if child_nodes.is_empty() {
            None
        } else {
            Some(child_nodes)
        }
    } else {
        None
    };

    Node {
        node_id: doc_id,
        backend_node_id: doc_id,
        node_type: NODE_TYPE_DOCUMENT,
        node_name: "#document".to_string(),
        local_name: String::new(),
        node_value: String::new(),
        child_node_count: frame.dom_tree.len() as u32,
        children,
        attributes: None,
    }
}

/// Handle DOM domain commands
pub fn handle(
    server: &mut DevtoolsServer,
    frame: &mut Frame,
    id: u64,
    command: &str,
    params: &Value,
) -> Response {
    match command {
        "enable" => {
            // DOM domain enabled
            Response::success(id, json!({}))
        }

        "disable" => {
            // DOM domain disabled
            Response::success(id, json!({}))
        }

        "getDocument" => {
            let depth = params.get("depth")
                .and_then(|v| v.as_i64())
                .map(|d| d as i32)
                .unwrap_or(1);

            let root = create_document_node(server, frame, depth);
            Response::success(id, json!({ "root": root }))
        }

        "querySelector" => {
            let node_id = match params.get("nodeId").and_then(|v| v.as_u64()) {
                Some(id) => id,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Missing nodeId parameter"),
            };

            let selector = match params.get("selector").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Missing selector parameter"),
            };

            // Get the starting node
            let search_roots = if node_id == server.document_node_id {
                frame.dom_tree.clone()
            } else {
                match server.get_element_by_id(node_id) {
                    Some(el) => el.borrow().children.clone(),
                    None => return Response::error(id, ERROR_INVALID_PARAMS, "Node not found"),
                }
            };

            // Simple selector matching (supports tag, .class, #id)
            if let Some(element) = find_element_by_selector(&search_roots, selector) {
                let found_id = server.get_or_create_node_id(&element);
                Response::success(id, json!({ "nodeId": found_id }))
            } else {
                Response::success(id, json!({ "nodeId": 0 }))
            }
        }

        "querySelectorAll" => {
            let node_id = match params.get("nodeId").and_then(|v| v.as_u64()) {
                Some(id) => id,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Missing nodeId parameter"),
            };

            let selector = match params.get("selector").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Missing selector parameter"),
            };

            // Get the starting node
            let search_roots = if node_id == server.document_node_id {
                frame.dom_tree.clone()
            } else {
                match server.get_element_by_id(node_id) {
                    Some(el) => el.borrow().children.clone(),
                    None => return Response::error(id, ERROR_INVALID_PARAMS, "Node not found"),
                }
            };

            let elements = find_all_elements_by_selector(&search_roots, selector);
            let node_ids: Vec<u64> = elements
                .iter()
                .map(|el| server.get_or_create_node_id(el))
                .collect();

            Response::success(id, json!({ "nodeIds": node_ids }))
        }

        "getBoxModel" => {
            let node_id = match params.get("nodeId").and_then(|v| v.as_u64()) {
                Some(id) => id,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Missing nodeId parameter"),
            };

            let element = match server.get_element_by_id(node_id) {
                Some(el) => el,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Node not found"),
            };

            let el = element.borrow();

            let (x, y, width, height) = if let Some(ref flow) = el.computed_flow {
                (flow.x as f64, flow.y as f64, flow.width as f64, flow.height as f64)
            } else {
                (0.0, 0.0, 0.0, 0.0)
            };

            let (margin, padding, border_widths) = if let Some(ref style) = el.computed_style {
                let m = &style.margin;
                let p = &style.padding;
                let b = &style.border;
                (
                    (m.top as f64, m.right as f64, m.bottom as f64, m.left as f64),
                    (p.top as f64, p.right as f64, p.bottom as f64, p.left as f64),
                    (b.top.width as f64, b.right.width as f64, b.bottom.width as f64, b.left.width as f64),
                )
            } else {
                ((0.0, 0.0, 0.0, 0.0), (0.0, 0.0, 0.0, 0.0), (0.0, 0.0, 0.0, 0.0))
            };

            // Calculate box boundaries
            let content_x = x;
            let content_y = y;
            let content_w = width;
            let content_h = height;

            let padding_x = content_x - padding.3;
            let padding_y = content_y - padding.0;
            let padding_w = content_w + padding.3 + padding.1;
            let padding_h = content_h + padding.0 + padding.2;

            let border_x = padding_x - border_widths.3;
            let border_y = padding_y - border_widths.0;
            let border_w = padding_w + border_widths.3 + border_widths.1;
            let border_h = padding_h + border_widths.0 + border_widths.2;

            let margin_x = border_x - margin.3;
            let margin_y = border_y - margin.0;
            let margin_w = border_w + margin.3 + margin.1;
            let margin_h = border_h + margin.0 + margin.2;

            // Create quads [x1,y1,x2,y2,x3,y3,x4,y4] (clockwise from top-left)
            let content_quad = vec![
                content_x, content_y,
                content_x + content_w, content_y,
                content_x + content_w, content_y + content_h,
                content_x, content_y + content_h,
            ];
            let padding_quad = vec![
                padding_x, padding_y,
                padding_x + padding_w, padding_y,
                padding_x + padding_w, padding_y + padding_h,
                padding_x, padding_y + padding_h,
            ];
            let border_quad = vec![
                border_x, border_y,
                border_x + border_w, border_y,
                border_x + border_w, border_y + border_h,
                border_x, border_y + border_h,
            ];
            let margin_quad = vec![
                margin_x, margin_y,
                margin_x + margin_w, margin_y,
                margin_x + margin_w, margin_y + margin_h,
                margin_x, margin_y + margin_h,
            ];

            let box_model = BoxModel {
                content: content_quad,
                padding: padding_quad,
                border: border_quad,
                margin: margin_quad,
                width: width as i32,
                height: height as i32,
            };

            Response::success(id, json!({ "model": box_model }))
        }

        "getAttributes" => {
            let node_id = match params.get("nodeId").and_then(|v| v.as_u64()) {
                Some(id) => id,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Missing nodeId parameter"),
            };

            let element = match server.get_element_by_id(node_id) {
                Some(el) => el,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Node not found"),
            };

            let el = element.borrow();
            let mut attrs = Vec::new();
            for (key, value) in &el.attributes {
                attrs.push(key.clone());
                attrs.push(value.clone());
            }

            Response::success(id, json!({ "attributes": attrs }))
        }

        "getOuterHTML" => {
            let node_id = match params.get("nodeId").and_then(|v| v.as_u64()) {
                Some(id) => id,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Missing nodeId parameter"),
            };

            let element = match server.get_element_by_id(node_id) {
                Some(el) => el,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Node not found"),
            };

            let outer_html = generate_outer_html(&element);
            Response::success(id, json!({ "outerHTML": outer_html }))
        }

        "describeNode" => {
            let node_id = params.get("nodeId").and_then(|v| v.as_u64());
            let backend_node_id = params.get("backendNodeId").and_then(|v| v.as_u64());

            let lookup_id = node_id.or(backend_node_id);
            let lookup_id = match lookup_id {
                Some(id) => id,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Missing nodeId or backendNodeId"),
            };

            let depth = params.get("depth")
                .and_then(|v| v.as_i64())
                .map(|d| d as i32)
                .unwrap_or(0);

            if lookup_id == server.document_node_id {
                let node = create_document_node(server, frame, depth);
                return Response::success(id, json!({ "node": node }));
            }

            let element = match server.get_element_by_id(lookup_id) {
                Some(el) => el,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Node not found"),
            };

            let node = element_to_node(server, &element, depth, 0);
            Response::success(id, json!({ "node": node }))
        }

        "getTextSegments" => {
            let node_id = match params.get("nodeId").and_then(|v| v.as_u64()) {
                Some(id) => id,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Missing nodeId parameter"),
            };

            let element = match server.get_element_by_id(node_id) {
                Some(el) => el,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Node not found"),
            };

            let el = element.borrow();
            let mut segments = Vec::new();

            // Collect text segments from this element's text children
            collect_text_segments_recursive(&el, &mut segments);

            Response::success(id, json!({ "textSegments": segments }))
        }

        _ => Response::error(id, ERROR_METHOD_NOT_FOUND, &format!("Unknown DOM method: {}", command)),
    }
}

/// Find first element matching a simple selector
fn find_element_by_selector(
    tree: &[Rc<RefCell<DomElement>>],
    selector: &str,
) -> Option<Rc<RefCell<DomElement>>> {
    for element in tree {
        let el = element.borrow();

        if el.node_type == NodeType::Element && matches_selector(&el, selector) {
            return Some(element.clone());
        }

        let children = el.children.clone();
        drop(el);

        if let Some(found) = find_element_by_selector(&children, selector) {
            return Some(found);
        }
    }
    None
}

/// Find all elements matching a simple selector
fn find_all_elements_by_selector(
    tree: &[Rc<RefCell<DomElement>>],
    selector: &str,
) -> Vec<Rc<RefCell<DomElement>>> {
    let mut results = Vec::new();
    find_all_elements_recursive(tree, selector, &mut results);
    results
}

fn find_all_elements_recursive(
    tree: &[Rc<RefCell<DomElement>>],
    selector: &str,
    results: &mut Vec<Rc<RefCell<DomElement>>>,
) {
    for element in tree {
        let el = element.borrow();

        if el.node_type == NodeType::Element && matches_selector(&el, selector) {
            results.push(element.clone());
        }

        let children = el.children.clone();
        drop(el);

        find_all_elements_recursive(&children, selector, results);
    }
}

/// Check if an element matches a simple selector (tag, .class, #id)
fn matches_selector(el: &DomElement, selector: &str) -> bool {
    let selector = selector.trim();

    if selector.starts_with('#') {
        // ID selector
        let id = &selector[1..];
        el.attributes.get("id").map(|v| v == id).unwrap_or(false)
    } else if selector.starts_with('.') {
        // Class selector
        let class = &selector[1..];
        el.class_list.contains(&class.to_string())
    } else {
        // Tag selector
        el.tag_name.eq_ignore_ascii_case(selector)
    }
}

/// Generate outer HTML for an element
fn generate_outer_html(element: &Rc<RefCell<DomElement>>) -> String {
    let el = element.borrow();

    match el.node_type {
        NodeType::Text => el.node_value.clone(),
        NodeType::Comment => format!("<!--{}-->", el.node_value),
        NodeType::DocumentType => format!("<!DOCTYPE {}>", el.node_value),
        NodeType::Element => {
            let tag = el.tag_name.to_lowercase();
            let mut html = format!("<{}", tag);

            for (key, value) in &el.attributes {
                html.push_str(&format!(" {}=\"{}\"", key, value));
            }
            html.push('>');

            let children = el.children.clone();
            drop(el);

            for child in &children {
                html.push_str(&generate_outer_html(child));
            }

            html.push_str(&format!("</{}>", tag));
            html
        }
    }
}

/// Collect text segments recursively from element and its children
fn collect_text_segments_recursive(el: &DomElement, out: &mut Vec<Value>) {
    // Add segments from text nodes
    if el.node_type == NodeType::Text {
        for seg in &el.text_segments {
            out.push(json!({
                "text": seg.text,
                "x": seg.x,
                "y": seg.y,
                "width": seg.width,
                "height": seg.height,
                "ascent": seg.ascent
            }));
        }
    }

    // Recurse into children
    for child_rc in &el.children {
        let child = child_rc.borrow();
        collect_text_segments_recursive(&child, out);
    }
}
