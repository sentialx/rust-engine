// DevTools Protocol integration tests

use graviton::devtools_protocol::DevtoolsServer;
use graviton::frame::Frame;
use graviton::layout::Rect;
use serde_json::{json, Value};

fn create_test_frame(html_file: &str) -> Frame {
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };
    let mut frame = Frame::new(viewport);
    frame.load_url(html_file);
    frame
}

fn send_command(server: &mut DevtoolsServer, frame: &mut Frame, method: &str, params: Value) -> Value {
    let request_json = json!({
        "id": 1,
        "method": method,
        "params": params
    });
    let response_str = server.handle_line(&request_json.to_string(), frame);
    serde_json::from_str(&response_str).expect("Failed to parse response")
}

// ============================================================================
// DOM Domain Tests
// ============================================================================

#[test]
fn test_dom_get_document() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let response = send_command(&mut server, &mut frame, "DOM.getDocument", json!({"depth": 2}));

    assert!(response.get("error").is_none(), "Expected success, got error: {:?}", response);

    let root = &response["result"]["root"];
    assert_eq!(root["nodeType"], 9, "Document should have nodeType 9");
    assert_eq!(root["nodeName"], "#document");
    assert!(root["childNodeCount"].as_u64().unwrap() > 0, "Document should have children");
    assert!(root["children"].is_array(), "Children should be present at depth 2");
}

#[test]
fn test_dom_get_document_depth_zero() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let response = send_command(&mut server, &mut frame, "DOM.getDocument", json!({"depth": 0}));

    let root = &response["result"]["root"];
    assert_eq!(root["nodeType"], 9);
    assert!(root.get("children").is_none() || root["children"].is_null(),
        "Children should not be present at depth 0");
}

#[test]
fn test_dom_query_selector_by_tag() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let response = send_command(&mut server, &mut frame, "DOM.querySelector", json!({
        "nodeId": 1,
        "selector": "div"
    }));

    assert!(response.get("error").is_none());
    let node_id = response["result"]["nodeId"].as_u64().unwrap();
    assert!(node_id > 0, "Should find a div element");
}

#[test]
fn test_dom_query_selector_by_class() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let response = send_command(&mut server, &mut frame, "DOM.querySelector", json!({
        "nodeId": 1,
        "selector": ".border-all"
    }));

    assert!(response.get("error").is_none());
    let node_id = response["result"]["nodeId"].as_u64().unwrap();
    assert!(node_id > 0, "Should find element with class border-all");
}

#[test]
fn test_dom_query_selector_not_found() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let response = send_command(&mut server, &mut frame, "DOM.querySelector", json!({
        "nodeId": 1,
        "selector": ".nonexistent-class"
    }));

    assert!(response.get("error").is_none());
    let node_id = response["result"]["nodeId"].as_u64().unwrap();
    assert_eq!(node_id, 0, "Should return 0 for not found");
}

#[test]
fn test_dom_query_selector_all() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let response = send_command(&mut server, &mut frame, "DOM.querySelectorAll", json!({
        "nodeId": 1,
        "selector": "div"
    }));

    assert!(response.get("error").is_none());
    let node_ids = response["result"]["nodeIds"].as_array().unwrap();
    assert_eq!(node_ids.len(), 7, "Should find 7 div elements");
}

#[test]
fn test_dom_get_box_model() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    // First get the element
    let query_response = send_command(&mut server, &mut frame, "DOM.querySelector", json!({
        "nodeId": 1,
        "selector": ".border-all"
    }));
    let node_id = query_response["result"]["nodeId"].as_u64().unwrap();

    // Then get its box model
    let response = send_command(&mut server, &mut frame, "DOM.getBoxModel", json!({
        "nodeId": node_id
    }));

    assert!(response.get("error").is_none());
    let model = &response["result"]["model"];

    assert_eq!(model["width"], 100, "Width should be 100px");
    assert_eq!(model["height"], 50, "Height should be 50px");

    // Content quad should have 8 values (4 points x 2 coords)
    assert_eq!(model["content"].as_array().unwrap().len(), 8);
    assert_eq!(model["padding"].as_array().unwrap().len(), 8);
    assert_eq!(model["border"].as_array().unwrap().len(), 8);
    assert_eq!(model["margin"].as_array().unwrap().len(), 8);
}

#[test]
fn test_dom_get_attributes() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    // First get the element
    let query_response = send_command(&mut server, &mut frame, "DOM.querySelector", json!({
        "nodeId": 1,
        "selector": ".border-all"
    }));
    let node_id = query_response["result"]["nodeId"].as_u64().unwrap();

    // Then get its attributes
    let response = send_command(&mut server, &mut frame, "DOM.getAttributes", json!({
        "nodeId": node_id
    }));

    assert!(response.get("error").is_none());
    let attrs = response["result"]["attributes"].as_array().unwrap();

    // Attributes should be flat array [name1, value1, name2, value2, ...]
    assert!(attrs.len() >= 2, "Should have at least one attribute (class)");
    assert!(attrs.contains(&json!("class")), "Should have class attribute");
    assert!(attrs.contains(&json!("border-all")), "Class value should be border-all");
}

#[test]
fn test_dom_get_outer_html() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    // First get the element
    let query_response = send_command(&mut server, &mut frame, "DOM.querySelector", json!({
        "nodeId": 1,
        "selector": ".border-all"
    }));
    let node_id = query_response["result"]["nodeId"].as_u64().unwrap();

    // Then get its outer HTML
    let response = send_command(&mut server, &mut frame, "DOM.getOuterHTML", json!({
        "nodeId": node_id
    }));

    assert!(response.get("error").is_none());
    let html = response["result"]["outerHTML"].as_str().unwrap();

    assert!(html.starts_with("<div"), "Should start with <div");
    assert!(html.contains("class=\"border-all\""), "Should contain class attribute");
    assert!(html.ends_with("</div>"), "Should end with </div>");
}

#[test]
fn test_dom_describe_node() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let response = send_command(&mut server, &mut frame, "DOM.describeNode", json!({
        "nodeId": 1,
        "depth": 1
    }));

    assert!(response.get("error").is_none());
    let node = &response["result"]["node"];

    assert_eq!(node["nodeType"], 9, "Should be document node");
    assert_eq!(node["nodeName"], "#document");
    assert!(node["children"].is_array(), "Should have children at depth 1");
}

#[test]
fn test_dom_node_not_found() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let response = send_command(&mut server, &mut frame, "DOM.getBoxModel", json!({
        "nodeId": 99999
    }));

    assert!(response.get("error").is_some(), "Should return error for invalid nodeId");
    assert_eq!(response["error"]["code"], -32602);
}

// ============================================================================
// CSS Domain Tests
// ============================================================================

#[test]
fn test_css_get_computed_style() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    // First get the element
    let query_response = send_command(&mut server, &mut frame, "DOM.querySelector", json!({
        "nodeId": 1,
        "selector": ".border-all"
    }));
    let node_id = query_response["result"]["nodeId"].as_u64().unwrap();

    // Then get computed styles
    let response = send_command(&mut server, &mut frame, "CSS.getComputedStyleForNode", json!({
        "nodeId": node_id
    }));

    assert!(response.get("error").is_none());
    let styles = response["result"]["computedStyle"].as_array().unwrap();

    // Convert to map for easier lookup
    let style_map: std::collections::HashMap<&str, &str> = styles
        .iter()
        .map(|s| (s["name"].as_str().unwrap(), s["value"].as_str().unwrap()))
        .collect();

    assert_eq!(style_map.get("display"), Some(&"block"));
    assert_eq!(style_map.get("width"), Some(&"100px"));
    assert_eq!(style_map.get("height"), Some(&"50px"));
    assert!(style_map.contains_key("border-top-width"));
    assert!(style_map.contains_key("background-color"));
}

#[test]
fn test_css_get_matched_styles() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    // First get the element
    let query_response = send_command(&mut server, &mut frame, "DOM.querySelector", json!({
        "nodeId": 1,
        "selector": ".border-all"
    }));
    let node_id = query_response["result"]["nodeId"].as_u64().unwrap();

    // Then get matched styles
    let response = send_command(&mut server, &mut frame, "CSS.getMatchedStylesForNode", json!({
        "nodeId": node_id
    }));

    assert!(response.get("error").is_none());
    assert!(response["result"].get("matchedCSSRules").is_some());
}

// ============================================================================
// Page Domain Tests
// ============================================================================

#[test]
fn test_page_get_layout_metrics() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let response = send_command(&mut server, &mut frame, "Page.getLayoutMetrics", json!({}));

    assert!(response.get("error").is_none());

    let metrics = &response["result"];
    assert!(metrics.get("layoutViewport").is_some());
    assert!(metrics.get("visualViewport").is_some());
    assert!(metrics.get("contentSize").is_some());

    assert_eq!(metrics["layoutViewport"]["clientWidth"], 800);
    assert_eq!(metrics["layoutViewport"]["clientHeight"], 600);
}

#[test]
fn test_page_capture_screenshot() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let response = send_command(&mut server, &mut frame, "Page.captureScreenshot", json!({
        "format": "png"
    }));

    assert!(response.get("error").is_none());

    let data = response["result"]["data"].as_str().unwrap();
    assert!(!data.is_empty(), "Screenshot data should not be empty");

    // Verify it's valid base64 by decoding
    let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data);
    assert!(decoded.is_ok(), "Should be valid base64");

    // PNG magic bytes: 89 50 4E 47
    let bytes = decoded.unwrap();
    assert!(bytes.len() > 8, "PNG should have header");
    assert_eq!(&bytes[0..4], &[0x89, 0x50, 0x4E, 0x47], "Should have PNG magic bytes");
}

#[test]
fn test_page_navigate() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    // Navigate to a different file
    let response = send_command(&mut server, &mut frame, "Page.navigate", json!({
        "url": "test_fixtures/borders.html"
    }));

    assert!(response.get("error").is_none());
    assert!(response["result"].get("frameId").is_some());
}

#[test]
fn test_page_reload() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let response = send_command(&mut server, &mut frame, "Page.reload", json!({}));

    assert!(response.get("error").is_none());
}

// ============================================================================
// Input Domain Tests
// ============================================================================

#[test]
fn test_input_dispatch_mouse_event() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let response = send_command(&mut server, &mut frame, "Input.dispatchMouseEvent", json!({
        "type": "mouseMoved",
        "x": 50,
        "y": 50
    }));

    assert!(response.get("error").is_none());
}

#[test]
fn test_input_dispatch_mouse_click() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let response = send_command(&mut server, &mut frame, "Input.dispatchMouseEvent", json!({
        "type": "mousePressed",
        "x": 50,
        "y": 50,
        "button": "left",
        "clickCount": 1
    }));

    assert!(response.get("error").is_none());
}

#[test]
fn test_input_dispatch_key_event() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let response = send_command(&mut server, &mut frame, "Input.dispatchKeyEvent", json!({
        "type": "keyDown",
        "key": "a",
        "code": "KeyA"
    }));

    assert!(response.get("error").is_none());
}

// ============================================================================
// Runtime Domain Tests
// ============================================================================

#[test]
fn test_runtime_evaluate_document_url() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let response = send_command(&mut server, &mut frame, "Runtime.evaluate", json!({
        "expression": "document.URL"
    }));

    assert!(response.get("error").is_none());
    assert_eq!(response["result"]["result"]["type"], "string");
    assert!(response["result"]["result"]["value"].as_str().unwrap().contains("borders.html"));
}

#[test]
fn test_runtime_evaluate_literals() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    // Test number
    let response = send_command(&mut server, &mut frame, "Runtime.evaluate", json!({
        "expression": "42"
    }));
    assert_eq!(response["result"]["result"]["type"], "number");
    assert_eq!(response["result"]["result"]["value"], 42.0);

    // Test boolean
    let response = send_command(&mut server, &mut frame, "Runtime.evaluate", json!({
        "expression": "true"
    }));
    assert_eq!(response["result"]["result"]["type"], "boolean");
    assert_eq!(response["result"]["result"]["value"], true);

    // Test string
    let response = send_command(&mut server, &mut frame, "Runtime.evaluate", json!({
        "expression": "\"hello\""
    }));
    assert_eq!(response["result"]["result"]["type"], "string");
    assert_eq!(response["result"]["result"]["value"], "hello");
}

// ============================================================================
// Enable/Disable Domain Tests
// ============================================================================

#[test]
fn test_domain_enable_disable() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    // Test enable/disable for each domain
    for domain in &["DOM", "CSS", "Page", "Input", "Runtime"] {
        let enable_response = send_command(&mut server, &mut frame, &format!("{}.enable", domain), json!({}));
        assert!(enable_response.get("error").is_none(), "{}.enable should succeed", domain);

        let disable_response = send_command(&mut server, &mut frame, &format!("{}.disable", domain), json!({}));
        assert!(disable_response.get("error").is_none(), "{}.disable should succeed", domain);
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_unknown_method() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let response = send_command(&mut server, &mut frame, "Unknown.method", json!({}));

    assert!(response.get("error").is_some());
    assert_eq!(response["error"]["code"], -32601); // Method not found
}

#[test]
fn test_unknown_domain() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let response = send_command(&mut server, &mut frame, "FakeDomain.fakeMethod", json!({}));

    assert!(response.get("error").is_some());
    assert_eq!(response["error"]["code"], -32601); // Method not found
}

#[test]
fn test_missing_required_param() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    // querySelector requires nodeId and selector
    let response = send_command(&mut server, &mut frame, "DOM.querySelector", json!({}));

    assert!(response.get("error").is_some());
    assert_eq!(response["error"]["code"], -32602); // Invalid params
}

#[test]
fn test_invalid_json() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let response_str = server.handle_line("not valid json", &mut frame);
    let response: Value = serde_json::from_str(&response_str).unwrap();

    assert!(response.get("error").is_some());
    assert_eq!(response["error"]["code"], -32603); // Internal error (JSON parse)
}

// ============================================================================
// Overlay Domain Tests
// ============================================================================

#[test]
fn test_overlay_enable_disable() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    let enable = send_command(&mut server, &mut frame, "Overlay.enable", json!({}));
    assert!(enable.get("error").is_none());

    let disable = send_command(&mut server, &mut frame, "Overlay.disable", json!({}));
    assert!(disable.get("error").is_none());
}

#[test]
fn test_overlay_highlight_node() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    // First get an element
    let query = send_command(&mut server, &mut frame, "DOM.querySelector", json!({
        "nodeId": 1,
        "selector": ".border-all"
    }));
    let node_id = query["result"]["nodeId"].as_u64().unwrap();

    // Highlight it
    let result = send_command(&mut server, &mut frame, "Overlay.highlightNode", json!({
        "nodeId": node_id
    }));
    assert!(result.get("error").is_none());

    // Verify highlight is set
    assert!(server.get_highlighted_node().is_some());
}

#[test]
fn test_overlay_hide_highlight() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    // Get and highlight an element
    let query = send_command(&mut server, &mut frame, "DOM.querySelector", json!({
        "nodeId": 1,
        "selector": ".border-all"
    }));
    let node_id = query["result"]["nodeId"].as_u64().unwrap();
    send_command(&mut server, &mut frame, "Overlay.highlightNode", json!({"nodeId": node_id}));

    // Hide highlight
    let result = send_command(&mut server, &mut frame, "Overlay.hideHighlight", json!({}));
    assert!(result.get("error").is_none());

    // Verify highlight is cleared
    assert!(server.get_highlighted_node().is_none());
}

#[test]
fn test_screenshot_with_overlay() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    // Get screenshot without overlay
    let screenshot1 = send_command(&mut server, &mut frame, "Page.captureScreenshot", json!({"format": "png"}));
    let data1 = screenshot1["result"]["data"].as_str().unwrap();

    // Highlight an element
    let query = send_command(&mut server, &mut frame, "DOM.querySelector", json!({
        "nodeId": 1,
        "selector": ".border-padding"
    }));
    let node_id = query["result"]["nodeId"].as_u64().unwrap();
    send_command(&mut server, &mut frame, "Overlay.highlightNode", json!({"nodeId": node_id}));

    // Get screenshot with overlay
    let screenshot2 = send_command(&mut server, &mut frame, "Page.captureScreenshot", json!({"format": "png"}));
    let data2 = screenshot2["result"]["data"].as_str().unwrap();

    // The screenshots should be different (overlay adds content)
    assert_ne!(data1, data2, "Screenshot with overlay should be different");

    // Hide and verify screenshot matches original
    send_command(&mut server, &mut frame, "Overlay.hideHighlight", json!({}));
    let screenshot3 = send_command(&mut server, &mut frame, "Page.captureScreenshot", json!({"format": "png"}));
    let data3 = screenshot3["result"]["data"].as_str().unwrap();

    assert_eq!(data1, data3, "Screenshot after hiding overlay should match original");
}

// ============================================================================
// Node Registry Tests
// ============================================================================

#[test]
fn test_node_ids_stable_across_queries() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    // Query the same element twice
    let response1 = send_command(&mut server, &mut frame, "DOM.querySelector", json!({
        "nodeId": 1,
        "selector": ".border-all"
    }));
    let node_id1 = response1["result"]["nodeId"].as_u64().unwrap();

    let response2 = send_command(&mut server, &mut frame, "DOM.querySelector", json!({
        "nodeId": 1,
        "selector": ".border-all"
    }));
    let node_id2 = response2["result"]["nodeId"].as_u64().unwrap();

    assert_eq!(node_id1, node_id2, "Same element should have same nodeId");
}

#[test]
fn test_node_registry_clears_on_navigation() {
    let mut frame = create_test_frame("test_fixtures/borders.html");
    let mut server = DevtoolsServer::new();

    // Get a node
    let _response1 = send_command(&mut server, &mut frame, "DOM.querySelector", json!({
        "nodeId": 1,
        "selector": "div"
    }));

    // Navigate (clears registry)
    send_command(&mut server, &mut frame, "Page.reload", json!({}));

    // Get the same element - should get new ID since registry was cleared
    let response2 = send_command(&mut server, &mut frame, "DOM.querySelector", json!({
        "nodeId": 1,
        "selector": "div"
    }));
    let node_id2 = response2["result"]["nodeId"].as_u64().unwrap();

    // After navigation, the document node ID (1) stays the same, but
    // element nodes get new IDs starting from 2
    assert_eq!(node_id2, 2, "After reload, first queried element should get nodeId 2");
}
