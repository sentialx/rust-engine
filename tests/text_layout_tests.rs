/// Tests for text preprocessing
///
/// These tests verify text preprocessing functionality:
/// - Text splitting into word segments
/// - Word measurement (width, height, ascent)
/// - Space width calculation
/// - HTML entity handling
///
/// NOTE: Layout positioning tests (wrapping, inline-block positioning, etc.)
/// are done via Chromium baseline comparison in layout_comparison_tests.rs.
/// See AGENTS.md for guidelines on layout testing.

use std::cell::RefCell;
use std::rc::Rc;

use graviton::html::{parse_html, DomElement, NodeType};
use graviton::layout::{compute_styles, propagate_styles, reflow, Rect};
use graviton::text::{BoxTextMeasurer, TextMeasurer};
use graviton::css::parse_css;
use graviton::styles::StyleRule;

/// Mock text measurer with predictable measurements
/// Each character is 10px wide, height is always 20px, ascent is 16px (80% of height)
struct MockTextMeasurer;

impl TextMeasurer for MockTextMeasurer {
    fn measure(&mut self, text: &str, _font_size: f32, _font_family: &str) -> (f32, f32) {
        let width = text.len() as f32 * 10.0;
        let height = 20.0;
        (width, height)
    }

    fn ascent(&mut self, _font_size: f32, _font_family: &str) -> f32 {
        16.0  // 80% of height (20px)
    }
}

/// Helper to set up a DOM tree with styles
fn setup_dom(html: &str, css: &str) -> (Vec<Rc<RefCell<DomElement>>>, Vec<StyleRule>) {
    let mut tree = parse_html(html);
    let default_css = r#"
        body { display: block; margin: 0; }
        div { display: block; }
        span { display: inline; }
    "#;
    let default_styles = parse_css(default_css);
    let custom_styles = parse_css(css);
    let styles: Vec<StyleRule> = [default_styles, custom_styles].concat();

    compute_styles(&mut tree, &styles, &mut vec![], None);
    propagate_styles(&mut tree, None);

    (tree, styles)
}

/// Helper to run reflow on a DOM tree
fn run_reflow(tree: &mut Vec<Rc<RefCell<DomElement>>>, width: f32, height: f32) {
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width,
        height,
    };
    let mut measurer = MockTextMeasurer;
    reflow(tree, &mut measurer, None, &viewport);
}

/// Helper to find an element by traversing the tree
fn find_text_node(tree: &Vec<Rc<RefCell<DomElement>>>) -> Option<Rc<RefCell<DomElement>>> {
    for el in tree {
        let borrowed = el.borrow();
        if borrowed.node_type == graviton::html::NodeType::Text {
            return Some(el.clone());
        }
        if !borrowed.children.is_empty() {
            if let Some(found) = find_text_node(&borrowed.children) {
                return Some(found);
            }
        }
    }
    None
}

/// Helper to find element by class name
#[allow(dead_code)]
fn find_by_class(tree: &Vec<Rc<RefCell<DomElement>>>, class: &str) -> Option<Rc<RefCell<DomElement>>> {
    for el in tree {
        let borrowed = el.borrow();
        if borrowed.class_list.contains(&class.to_string()) {
            return Some(el.clone());
        }
        if !borrowed.children.is_empty() {
            if let Some(found) = find_by_class(&borrowed.children, class) {
                return Some(found);
            }
        }
    }
    None
}

fn find_by_tag_name(tree: &Vec<Rc<RefCell<DomElement>>>, tag_name: &str) -> Option<Rc<RefCell<DomElement>>> {
    for el in tree {
        let borrowed = el.borrow();
        if borrowed.tag_name == tag_name {
            return Some(el.clone());
        }
        if !borrowed.children.is_empty() {
            if let Some(found) = find_by_tag_name(&borrowed.children, tag_name) {
                return Some(found);
            }
        }
    }
    None
}

fn find_by_node_type(tree: &Vec<Rc<RefCell<DomElement>>>, node_type: NodeType) -> Option<Rc<RefCell<DomElement>>> {
    for el in tree {
        let borrowed = el.borrow();
        if borrowed.node_type == node_type {
            return Some(el.clone());
        }
        if !borrowed.children.is_empty() {
            if let Some(found) = find_by_node_type(&borrowed.children, node_type.clone()) {
                return Some(found);
            }
        }
    }
    None
}

// =============================================================================
// Text Preprocessing Tests
// =============================================================================

#[test]
fn test_text_preprocessing_splits_into_words() {
    let html = r#"<body><div>Hello world test</div></body>"#;
    let (mut tree, _) = setup_dom(html, "");
    run_reflow(&mut tree, 800.0, 600.0);

    let text_node = find_text_node(&tree).expect("Should find text node");
    let el = text_node.borrow();

    assert_eq!(el.text_segments.len(), 3, "Should have 3 words");
    assert_eq!(el.text_segments[0].text, "Hello");
    assert_eq!(el.text_segments[1].text, "world");
    assert_eq!(el.text_segments[2].text, "test");
}

#[test]
fn test_text_preprocessing_measures_words() {
    let html = r#"<body><div>Hi there</div></body>"#;
    let (mut tree, _) = setup_dom(html, "");
    run_reflow(&mut tree, 800.0, 600.0);

    let text_node = find_text_node(&tree).expect("Should find text node");
    let el = text_node.borrow();

    // MockTextMeasurer: 10px per character, 20px height
    assert_eq!(el.text_segments[0].text, "Hi");
    assert_eq!(el.text_segments[0].width, 20.0, "Hi should be 20px wide (2 chars)");
    assert_eq!(el.text_segments[0].height, 20.0);

    assert_eq!(el.text_segments[1].text, "there");
    assert_eq!(el.text_segments[1].width, 50.0, "there should be 50px wide (5 chars)");
    assert_eq!(el.text_segments[1].height, 20.0);
}

#[test]
fn test_text_preprocessing_calculates_space_width() {
    let html = r#"<body><div>A B</div></body>"#;
    let (mut tree, _) = setup_dom(html, "");
    run_reflow(&mut tree, 800.0, 600.0);

    let text_node = find_text_node(&tree).expect("Should find text node");
    let el = text_node.borrow();

    // MockTextMeasurer: space is 1 character = 10px
    assert_eq!(el.space_width, 10.0, "Space should be 10px wide");
}

#[test]
fn test_text_preprocessing_handles_multiple_spaces() {
    let html = r#"<body><div>Hello    world</div></body>"#;
    let (mut tree, _) = setup_dom(html, "");
    run_reflow(&mut tree, 800.0, 600.0);

    let text_node = find_text_node(&tree).expect("Should find text node");
    let el = text_node.borrow();

    // split_whitespace collapses multiple spaces
    assert_eq!(el.text_segments.len(), 2, "Should collapse multiple spaces");
    assert_eq!(el.text_segments[0].text, "Hello");
    assert_eq!(el.text_segments[1].text, "world");
}

#[test]
fn test_text_preprocessing_handles_html_entities() {
    let html = r#"<body><div>A &gt; B &lt; C</div></body>"#;
    let (mut tree, _) = setup_dom(html, "");
    run_reflow(&mut tree, 800.0, 600.0);

    let text_node = find_text_node(&tree).expect("Should find text node");
    let el = text_node.borrow();

    // Check that entities were converted
    let all_text: String = el.text_segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" ");
    assert!(all_text.contains(">"), "Should convert &gt; to >");
    assert!(all_text.contains("<"), "Should convert &lt; to <");
}


// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_empty_text_node() {
    let html = r#"<body><div></div></body>"#;
    let (mut tree, _) = setup_dom(html, "");
    run_reflow(&mut tree, 800.0, 600.0);

    // Empty div should not have text segments
    // This shouldn't crash
    let text_node = find_text_node(&tree);
    assert!(text_node.is_none(), "Empty div should not have text node");
}

#[test]
fn test_whitespace_only_text() {
    let html = r#"<body><div>   </div></body>"#;
    let (mut tree, _) = setup_dom(html, "");
    run_reflow(&mut tree, 800.0, 600.0);

    let text_node = find_text_node(&tree);
    if let Some(node) = text_node {
        let el = node.borrow();
        // Whitespace-only text should have no segments (split_whitespace returns empty)
        assert_eq!(el.text_segments.len(), 0, "Whitespace-only should have no segments");
    }
}

#[test]
fn test_single_word() {
    let html = r#"<body><div>Hello</div></body>"#;
    let (mut tree, _) = setup_dom(html, "");
    run_reflow(&mut tree, 800.0, 600.0);

    let text_node = find_text_node(&tree).expect("Should find text node");
    let el = text_node.borrow();

    assert_eq!(el.text_segments.len(), 1);
    assert_eq!(el.text_segments[0].text, "Hello");
    assert_eq!(el.text_segments[0].width, 50.0, "Hello should be 50px wide (5 chars)");
}

#[test]
fn test_long_word_measurement() {
    let html = r#"<body><div>ABCDEFGHIJ</div></body>"#;
    let (mut tree, _) = setup_dom(html, "");
    run_reflow(&mut tree, 800.0, 600.0);

    let text_node = find_text_node(&tree).expect("Should find text node");
    let el = text_node.borrow();

    // Word is 100px (10 chars * 10px)
    assert_eq!(el.text_segments.len(), 1);
    assert_eq!(el.text_segments[0].width, 100.0, "Long word should be measured correctly");
    assert_eq!(el.text_segments[0].height, 20.0);
}



// =============================================================================
// Text Ascent Tests
// =============================================================================

#[test]
fn test_text_segments_have_ascent() {
    let html = r#"<body><div>Hello World</div></body>"#;
    let (mut tree, _) = setup_dom(html, "");
    run_reflow(&mut tree, 800.0, 600.0);

    let text_node = find_text_node(&tree).expect("Should find text node");
    let el = text_node.borrow();

    // MockTextMeasurer returns ascent of 16.0 (80% of 20px height)
    for seg in &el.text_segments {
        assert_eq!(seg.ascent, 16.0, "Segment '{}' should have ascent of 16.0", seg.text);
    }
}

#[test]
fn test_ascent_used_for_baseline() {
    // Ascent determines where the baseline is within the text box
    // baseline_y = seg.y + seg.ascent
    let html = r#"<body><div>Test</div></body>"#;
    let (mut tree, _) = setup_dom(html, "");
    run_reflow(&mut tree, 800.0, 600.0);

    let text_node = find_text_node(&tree).expect("Should find text node");
    let el = text_node.borrow();

    let seg = &el.text_segments[0];
    let baseline_y = seg.y + seg.ascent;

    // Baseline should be within the text box
    assert!(baseline_y > seg.y, "Baseline should be below top of text box");
    assert!(baseline_y < seg.y + seg.height, "Baseline should be above bottom of text box");

    // With height=20 and ascent=16, baseline is at y+16, which leaves 4px for descenders
    assert_eq!(seg.height - seg.ascent, 4.0, "Should have 4px below baseline for descenders");
}

#[test]
fn test_box_text_measurer_dimensions() {
    let mut measurer = BoxTextMeasurer::default();
    let (width, height) = measurer.measure("abcd", 10.0, "Times New Roman");
    let ascent = measurer.ascent(10.0, "Times New Roman");

    assert_eq!(height, 10.0, "Box measurer uses font size as height");
    assert_eq!(width, 24.0, "Box measurer uses 0.6x font size per character");
    assert_eq!(ascent, 8.0, "Box measurer uses 0.8x font size for ascent");
}

#[test]
fn test_html5ever_parse_builds_dom_nodes() {
    let html = "<!doctype html><!-- comment --><div class=\"a b\" style=\"margin: 5px;\">Hi</div>";
    let tree = parse_html(html);

    let doctype = find_by_node_type(&tree, NodeType::DocumentType).expect("Should find doctype node");
    assert_eq!(doctype.borrow().node_value.to_lowercase(), "html");

    let comment = find_by_node_type(&tree, NodeType::Comment).expect("Should find comment node");
    assert_eq!(comment.borrow().node_value.trim(), "comment");

    let div = find_by_tag_name(&tree, "DIV").expect("Should find DIV element");
    let div_borrowed = div.borrow();
    assert!(div_borrowed.class_list.contains(&"a".to_string()));
    assert!(div_borrowed.class_list.contains(&"b".to_string()));
    assert_eq!(
        div_borrowed.attributes.get("style").map(|v| v.as_str()),
        Some("margin: 5px;")
    );
}


