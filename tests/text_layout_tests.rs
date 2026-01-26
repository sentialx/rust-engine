/// Tests for text preprocessing and layout
///
/// These tests verify the unified inline layout system:
/// - Text preprocessing into segments (words)
/// - Text segment positioning during layout
/// - Text wrapping behavior

use std::cell::RefCell;
use std::rc::Rc;

use graviton::html::{parse_html, DomElement};
use graviton::layout::{compute_styles, propagate_styles, reflow, Rect};
use graviton::render_frame::TextMeasurer;
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
// Text Positioning Tests
// =============================================================================

#[test]
fn test_text_segments_positioned_horizontally() {
    let html = r#"<body><div>AA BB CC</div></body>"#;
    let (mut tree, _) = setup_dom(html, "");
    run_reflow(&mut tree, 800.0, 600.0);

    let text_node = find_text_node(&tree).expect("Should find text node");
    let el = text_node.borrow();

    assert_eq!(el.text_segments.len(), 3);

    // First word at x=0
    assert_eq!(el.text_segments[0].x, 0.0, "First word should start at x=0");

    // Second word after first word + space
    // "AA" = 20px, space = 10px, so "BB" starts at 30px
    let expected_x1 = 20.0 + 10.0;
    assert_eq!(el.text_segments[1].x, expected_x1, "Second word should be after first + space");

    // Third word after second word + space
    // 30 + 20 (BB) + 10 (space) = 60
    let expected_x2 = expected_x1 + 20.0 + 10.0;
    assert_eq!(el.text_segments[2].x, expected_x2, "Third word should be after second + space");
}

#[test]
fn test_text_segments_same_y_position() {
    let html = r#"<body><div>One Two Three</div></body>"#;
    let (mut tree, _) = setup_dom(html, "");
    run_reflow(&mut tree, 800.0, 600.0);

    let text_node = find_text_node(&tree).expect("Should find text node");
    let el = text_node.borrow();

    // All words on same line should have same y
    let y0 = el.text_segments[0].y;
    assert_eq!(el.text_segments[1].y, y0, "All words should be on same line");
    assert_eq!(el.text_segments[2].y, y0, "All words should be on same line");
}

// =============================================================================
// Text Wrapping Tests
// =============================================================================

#[test]
fn test_text_wraps_when_exceeds_width() {
    let html = r#"<body><div class="narrow">AAAA BBBB CCCC</div></body>"#;
    let css = r#".narrow { width: 60px; }"#;
    let (mut tree, _) = setup_dom(html, css);
    run_reflow(&mut tree, 800.0, 600.0);

    let text_node = find_text_node(&tree).expect("Should find text node");
    let el = text_node.borrow();

    // Each word is 40px (4 chars * 10px)
    // Container is 60px wide
    // "AAAA" (40px) fits on line 1
    // "BBBB" (40px) doesn't fit (40+10+40=90 > 60), wraps to line 2
    // "CCCC" (40px) doesn't fit on line 2 (40+10+40=90 > 60), wraps to line 3

    assert_eq!(el.text_segments.len(), 3);

    // Each word should be on a different line (different y values)
    let y0 = el.text_segments[0].y;
    let y1 = el.text_segments[1].y;
    let y2 = el.text_segments[2].y;

    assert!(y1 > y0, "Second word should wrap to next line (y1={} > y0={})", y1, y0);
    assert!(y2 > y1, "Third word should wrap to next line (y2={} > y1={})", y2, y1);
}

#[test]
fn test_wrapped_lines_start_at_container_edge() {
    let html = r#"<body><div class="narrow">AAAA BBBB</div></body>"#;
    let css = r#".narrow { width: 60px; }"#;
    let (mut tree, _) = setup_dom(html, css);
    run_reflow(&mut tree, 800.0, 600.0);

    let text_node = find_text_node(&tree).expect("Should find text node");
    let el = text_node.borrow();

    // Second word wraps and should start at x=0 (container left edge)
    let x0 = el.text_segments[0].x;
    let x1 = el.text_segments[1].x;

    assert_eq!(x1, x0, "Wrapped line should start at same x as first line (x0={}, x1={})", x0, x1);
}

#[test]
fn test_multiple_words_fit_on_same_line() {
    let html = r#"<body><div class="wide">AA BB CC DD</div></body>"#;
    let css = r#".wide { width: 200px; }"#;
    let (mut tree, _) = setup_dom(html, css);
    run_reflow(&mut tree, 800.0, 600.0);

    let text_node = find_text_node(&tree).expect("Should find text node");
    let el = text_node.borrow();

    // Each word is 20px, space is 10px
    // Total: 20 + 10 + 20 + 10 + 20 + 10 + 20 = 110px < 200px
    // All words should fit on one line

    assert_eq!(el.text_segments.len(), 4);

    let y0 = el.text_segments[0].y;
    for (i, seg) in el.text_segments.iter().enumerate() {
        assert_eq!(seg.y, y0, "Word {} should be on same line", i);
    }
}

#[test]
fn test_text_wrapping_respects_padding() {
    let html = r#"<body><div class="padded">AAAA BBBB</div></body>"#;
    let css = r#".padded { width: 100px; padding: 10px; }"#;
    let (mut tree, _) = setup_dom(html, css);
    run_reflow(&mut tree, 800.0, 600.0);

    let text_node = find_text_node(&tree).expect("Should find text node");
    let el = text_node.borrow();

    // Container starts at x=0, padding is 10px
    // Text content area starts at x=10
    let first_word_x = el.text_segments[0].x;

    // First word should start exactly at left padding edge
    assert_eq!(first_word_x, 10.0, "First word should start at padding edge (x={})", first_word_x);

    // Content width is 100px, so available text width is 100px
    // Each word is 40px (4 chars * 10px), space is 10px
    // 40 + 10 + 40 = 90px fits in 100px, so both words on same line
    assert_eq!(el.text_segments.len(), 2);
    let y0 = el.text_segments[0].y;
    let y1 = el.text_segments[1].y;
    assert_eq!(y0, y1, "Both words should fit on same line within padding");
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
    assert_eq!(el.text_segments[0].x, 0.0, "Single word should start at x=0");
}

#[test]
fn test_long_word_exceeds_container() {
    let html = r#"<body><div class="tiny">ABCDEFGHIJ</div></body>"#;
    let css = r#".tiny { width: 50px; }"#;
    let (mut tree, _) = setup_dom(html, css);
    run_reflow(&mut tree, 800.0, 600.0);

    let text_node = find_text_node(&tree).expect("Should find text node");
    let el = text_node.borrow();

    // Word is 100px (10 chars * 10px), container is 50px
    // Word cannot be broken, should overflow
    assert_eq!(el.text_segments.len(), 1);
    assert_eq!(el.text_segments[0].width, 100.0, "Long word should keep its width");
    // Word should start at container edge, even if it overflows
    assert_eq!(el.text_segments[0].x, 0.0);
}

// =============================================================================
// Inline-Block Positioning Tests
// =============================================================================

#[test]
fn test_inline_blocks_dont_overlap() {
    let html = r#"<body><div class="container"><span class="item"></span><span class="item"></span><span class="item"></span></div></body>"#;
    let css = r#"
        .container { display: block; }
        .item { display: inline-block; width: 50px; height: 30px; }
    "#;
    let (mut tree, _) = setup_dom(html, css);
    run_reflow(&mut tree, 800.0, 600.0);

    // Find the container
    let container = find_by_class(&tree, "container").expect("Should find container");
    let container_el = container.borrow();

    // Get all items (should be 3)
    let items: Vec<_> = container_el.children.iter()
        .filter(|c| c.borrow().class_list.contains(&"item".to_string()))
        .collect();

    assert_eq!(items.len(), 3, "Should have 3 inline-block items");

    // Check that items don't overlap
    let item1_flow = items[0].borrow().computed_flow.clone().expect("Item should have computed flow");
    let item2_flow = items[1].borrow().computed_flow.clone().expect("Item should have computed flow");
    let item3_flow = items[2].borrow().computed_flow.clone().expect("Item should have computed flow");

    // Each item is 50px wide
    // Item 2 should start after item 1 ends
    assert!(
        item2_flow.x >= item1_flow.x + item1_flow.width,
        "Item 2 (x={}) should not overlap with item 1 (x={}, width={})",
        item2_flow.x, item1_flow.x, item1_flow.width
    );

    // Item 3 should start after item 2 ends
    assert!(
        item3_flow.x >= item2_flow.x + item2_flow.width,
        "Item 3 (x={}) should not overlap with item 2 (x={}, width={})",
        item3_flow.x, item2_flow.x, item2_flow.width
    );
}

#[test]
fn test_inline_blocks_horizontal_spacing() {
    let html = r#"<body><div class="container"><span class="item"></span><span class="item"></span></div></body>"#;
    let css = r#"
        .container { display: block; }
        .item { display: inline-block; width: 100px; height: 50px; }
    "#;
    let (mut tree, _) = setup_dom(html, css);
    run_reflow(&mut tree, 800.0, 600.0);

    let container = find_by_class(&tree, "container").expect("Should find container");
    let container_el = container.borrow();

    let items: Vec<_> = container_el.children.iter()
        .filter(|c| c.borrow().class_list.contains(&"item".to_string()))
        .collect();

    let item1_flow = items[0].borrow().computed_flow.clone().expect("Item should have computed flow");
    let item2_flow = items[1].borrow().computed_flow.clone().expect("Item should have computed flow");

    // Items should be positioned correctly
    assert_eq!(item1_flow.x, 0.0, "First item should start at x=0");
    assert!(item2_flow.x >= 100.0, "Second item should start at x>=100 (after first item), got {}", item2_flow.x);
}

#[test]
fn test_inline_block_wrap_to_next_line() {
    let html = r#"<body><div class="container"><span class="item"></span><span class="item"></span><span class="item"></span></div></body>"#;
    let css = r#"
        .container { display: block; width: 150px; }
        .item { display: inline-block; width: 100px; height: 30px; }
    "#;
    let (mut tree, _) = setup_dom(html, css);
    run_reflow(&mut tree, 800.0, 600.0);

    let container = find_by_class(&tree, "container").expect("Should find container");
    let container_el = container.borrow();

    let items: Vec<_> = container_el.children.iter()
        .filter(|c| c.borrow().class_list.contains(&"item".to_string()))
        .collect();

    assert_eq!(items.len(), 3);

    let item1_flow = items[0].borrow().computed_flow.clone().expect("Item 1 should have computed flow");
    let item2_flow = items[1].borrow().computed_flow.clone().expect("Item 2 should have computed flow");
    let item3_flow = items[2].borrow().computed_flow.clone().expect("Item 3 should have computed flow");

    // Container is 150px, items are 100px each
    // Item 1 fits on line 1
    // Item 2 doesn't fit (100 + 100 > 150), wraps to line 2
    // Item 3 doesn't fit (100 + 100 > 150), wraps to line 3

    // All items should start at x=0 (each on its own line)
    assert_eq!(item1_flow.x, 0.0, "Item 1 should be at x=0");
    assert_eq!(item2_flow.x, 0.0, "Item 2 should wrap to x=0");
    assert_eq!(item3_flow.x, 0.0, "Item 3 should wrap to x=0");

    // Items should be on different y positions (different lines)
    assert!(item2_flow.y > item1_flow.y, "Item 2 should be on next line");
    assert!(item3_flow.y > item2_flow.y, "Item 3 should be on next line");
}

// =============================================================================
// Container Sizing Tests
// =============================================================================

#[test]
fn test_container_height_matches_text_lines() {
    let html = r#"<body><div class="narrow">AAA BBB CCC</div></body>"#;
    let css = r#".narrow { width: 50px; }"#;
    let (mut tree, _) = setup_dom(html, css);
    run_reflow(&mut tree, 800.0, 600.0);

    let text_node = find_text_node(&tree).expect("Should find text node");
    let el = text_node.borrow();

    // Each word is 30px (3 chars * 10px), container is 50px
    // "AAA" (30px) fits on line 1
    // "BBB" (30px) doesn't fit (30 + 10 + 30 = 70 > 50), wraps to line 2
    // "CCC" (30px) doesn't fit (30 + 10 + 30 = 70 > 50), wraps to line 3
    // Total: 3 lines, each 20px high = 60px total height

    assert_eq!(el.text_segments.len(), 3, "Should have 3 words");

    // Verify each word is on a different line
    let y0 = el.text_segments[0].y;
    let y1 = el.text_segments[1].y;
    let y2 = el.text_segments[2].y;

    assert!(y1 > y0, "Word 2 should be on line 2");
    assert!(y2 > y1, "Word 3 should be on line 3");

    // Calculate total height from segments
    let max_bottom = el.text_segments.iter()
        .map(|s| s.y + s.height)
        .fold(0.0_f32, f32::max);
    let min_top = el.text_segments.iter()
        .map(|s| s.y)
        .fold(f32::MAX, f32::min);
    let text_height = max_bottom - min_top;

    // 3 lines * 20px = 60px
    assert_eq!(text_height, 60.0, "Text should span exactly 3 lines (height={})", text_height);
}
