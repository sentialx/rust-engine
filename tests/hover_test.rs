use graviton::frame::Frame;
use graviton::layout::Rect;
use graviton::css::parse_css;
use graviton::devtools_protocol::DevtoolsServer;
use serde_json::{json, Value};

#[test]
fn test_hover_css_parsing() {
    let css = r#"
      .pill {
        display: inline-block;
        background-color: #111827;
      }

      .pill:hover {
        background-color: #2563eb;
      }
    "#;

    let rules = parse_css(css);
    println!("Parsed {} rules:", rules.len());
    for rule in &rules {
        println!("  Selector: '{}'", rule.selector.to_string());
    }

    assert_eq!(rules.len(), 2, "Should parse 2 rules");

    let hover_rule = rules.iter().find(|r| r.selector.to_string().contains("hover"));
    assert!(hover_rule.is_some(), "Should have a :hover rule");
}

#[test]
fn test_hover_in_html() {
    let html = r#"
<!DOCTYPE html>
<html>
<head>
<style>
.pill {
    display: inline-block;
    background-color: #111827;
}
.pill:hover {
    background-color: #2563eb;
}
</style>
</head>
<body>
    <span class="pill">Test</span>
</body>
</html>
"#;

    let viewport = Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
    let mut frame = Frame::new(viewport);
    frame.load_html(html);

    println!("Parsed CSS rules: {}", frame.parsed_css.len());
    for rule in &frame.parsed_css {
        println!("  Rule: '{}'", rule.selector.to_string());
    }

    let hover_rule = frame.parsed_css.iter().find(|r| {
        let s = r.selector.to_string().to_lowercase();
        s.contains("hover")
    });
    assert!(hover_rule.is_some(), "Should have parsed a :hover rule");
}

#[test]
fn test_hover_style_applied() {
    let html = r#"
<!DOCTYPE html>
<html>
<head>
<style>
.pill {
    display: inline-block;
    background-color: #111827;
}
.pill:hover {
    background-color: #ff0000;
}
</style>
</head>
<body>
    <span class="pill" id="target">Test</span>
</body>
</html>
"#;

    let viewport = Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
    let mut frame = Frame::new(viewport);
    frame.load_html(html);

    // Find the pill element
    let pill = frame.get_element_by_id("target").expect("should find pill element");

    // Check initial state - not hovered
    {
        let el = pill.borrow();
        println!("Initial hover state: {}", el.pseudo_classes.hover);
        assert!(!el.pseudo_classes.hover, "Should not be hovered initially");
    }

    // Set hover
    frame.set_hover(Some(pill.clone()));

    // Check hover state after set_hover
    {
        let el = pill.borrow();
        println!("After set_hover: {}", el.pseudo_classes.hover);
        assert!(el.pseudo_classes.hover, "Should be hovered after set_hover");
    }

    // Check if :hover style was applied - look for matched styles with hover
    {
        let el = pill.borrow();
        println!("Matched styles: {}", el.matched_styles.len());
        for style in &el.matched_styles {
            println!("  Matched: {}", style.selector.to_string());
        }

        let has_hover_style = el.matched_styles.iter().any(|s| {
            s.selector.to_string().to_lowercase().contains("hover")
        });
        assert!(has_hover_style, "Should have :hover style matched after hovering");
    }
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

#[test]
fn test_hover_ui_demo() {
    let viewport = Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
    let mut frame = Frame::new(viewport);
    frame.load_url("ui_demo.html");
    let mut server = DevtoolsServer::new();

    // Print all hover rules
    println!("All hover rules in ui_demo.html:");
    for rule in &frame.parsed_css {
        let s = rule.selector.to_string();
        if s.to_lowercase().contains("hover") {
            println!("  {}", s);
        }
    }

    // Query for a pill element using DOM.querySelector
    let response = send_command(&mut server, &mut frame, "DOM.getDocument", json!({"depth": 0}));
    println!("DOM.getDocument response: {:?}", response);

    let response = send_command(&mut server, &mut frame, "DOM.querySelector", json!({
        "nodeId": 1,
        "selector": ".pill"
    }));
    println!("DOM.querySelector .pill response: {:?}", response);

    let node_id = response["result"]["nodeId"].as_u64().expect("should get nodeId");

    // Get matched styles before hover
    let response = send_command(&mut server, &mut frame, "CSS.getMatchedStylesForNode", json!({
        "nodeId": node_id
    }));
    println!("Matched styles before hover:");
    if let Some(rules) = response["result"]["matchedCSSRules"].as_array() {
        for rule in rules {
            println!("  {}", rule["rule"]["selectorList"]["text"]);
        }
    }

    // Set hover on the pill element
    let pill = server.get_element_by_id(node_id).expect("should find element");
    frame.set_hover(Some(pill.clone()));

    // Get matched styles after hover
    let response = send_command(&mut server, &mut frame, "CSS.getMatchedStylesForNode", json!({
        "nodeId": node_id
    }));
    println!("Matched styles after hover:");
    if let Some(rules) = response["result"]["matchedCSSRules"].as_array() {
        for rule in rules {
            println!("  {}", rule["rule"]["selectorList"]["text"]);
        }
    }

    // Verify hover style is matched
    let has_hover = response["result"]["matchedCSSRules"]
        .as_array()
        .map(|rules| rules.iter().any(|r| {
            r["rule"]["selectorList"]["text"]
                .as_str()
                .map(|s| s.to_lowercase().contains("hover"))
                .unwrap_or(false)
        }))
        .unwrap_or(false);

    assert!(has_hover, "Should have :hover style matched after hovering");
}

#[test]
fn test_hover_via_mouse_move() {
    use base64::Engine;
    use std::fs;

    let viewport = Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
    let mut frame = Frame::new(viewport);
    frame.load_url("ui_demo.html");
    let mut server = DevtoolsServer::new();

    // Find a .pill element and get its box model
    let response = send_command(&mut server, &mut frame, "DOM.getDocument", json!({"depth": 0}));
    let response = send_command(&mut server, &mut frame, "DOM.querySelector", json!({
        "nodeId": 1,
        "selector": ".pill"
    }));
    let node_id = response["result"]["nodeId"].as_u64().expect("should get nodeId");

    // Get the box model to find coordinates
    let response = send_command(&mut server, &mut frame, "DOM.getBoxModel", json!({
        "nodeId": node_id
    }));
    println!("Box model: {:?}", response);

    let content = response["result"]["model"]["content"].as_array().expect("should have content");
    // content is [x1, y1, x2, y2, x3, y3, x4, y4] - get center
    let x = (content[0].as_f64().unwrap() + content[2].as_f64().unwrap()) / 2.0;
    let y = (content[1].as_f64().unwrap() + content[5].as_f64().unwrap()) / 2.0;
    println!("Pill center: ({}, {})", x, y);

    // Take screenshot before hover
    let response = send_command(&mut server, &mut frame, "Page.captureScreenshot", json!({
        "format": "png"
    }));
    let data_before = response["result"]["data"].as_str().expect("should have screenshot data");
    let bytes_before = base64::engine::general_purpose::STANDARD.decode(data_before).expect("decode base64");
    fs::write("/tmp/hover_before.png", &bytes_before).expect("write screenshot");
    println!("Saved /tmp/hover_before.png");

    // Get matched styles before hover
    let response = send_command(&mut server, &mut frame, "CSS.getMatchedStylesForNode", json!({
        "nodeId": node_id
    }));
    println!("Matched styles BEFORE mouseMoved:");
    if let Some(rules) = response["result"]["matchedCSSRules"].as_array() {
        for rule in rules {
            println!("  {}", rule["rule"]["selectorList"]["text"]);
        }
    }

    // Dispatch mouse move to hover over the pill
    let response = send_command(&mut server, &mut frame, "Input.dispatchMouseEvent", json!({
        "type": "mouseMoved",
        "x": x,
        "y": y
    }));
    println!("Mouse move response: {:?}", response);

    // Get matched styles after hover
    let response = send_command(&mut server, &mut frame, "CSS.getMatchedStylesForNode", json!({
        "nodeId": node_id
    }));
    println!("Matched styles AFTER mouseMoved:");
    let mut has_hover = false;
    if let Some(rules) = response["result"]["matchedCSSRules"].as_array() {
        for rule in rules {
            let selector = rule["rule"]["selectorList"]["text"].as_str().unwrap_or("");
            println!("  {}", selector);
            if selector.to_lowercase().contains("hover") {
                has_hover = true;
            }
        }
    }

    // Take screenshot after hover
    let response = send_command(&mut server, &mut frame, "Page.captureScreenshot", json!({
        "format": "png"
    }));
    let data_after = response["result"]["data"].as_str().expect("should have screenshot data");
    let bytes_after = base64::engine::general_purpose::STANDARD.decode(data_after).expect("decode base64");
    fs::write("/tmp/hover_after.png", &bytes_after).expect("write screenshot");
    println!("Saved /tmp/hover_after.png");

    // Verify hover style was applied
    assert!(has_hover, "Should have :hover style matched after mouseMoved");

    // Verify screenshots are different (hover changed the appearance)
    assert_ne!(bytes_before, bytes_after, "Screenshots should differ after hover");
}
