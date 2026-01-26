use super::*;
use crate::styles::{ComputedStyle, ComputedMargin};
use crate::render_frame::TextMeasurer;
use crate::css_value::{CssValue, CssSize, CssSizeUnit};
use crate::properties::margin::{Margin, MarginComponent};
use crate::properties::string_property::StringProperty;
use std::rc::Rc;
use std::cell::RefCell;

// Test-only helper functions (previously in layout.rs)

fn compute_element_position(
    element: &DomElement,
    prev_element: Option<&DomElement>,
    formatting_context: FormattingContext,
    prev_formatting_context: Option<FormattingContext>,
    inline_ctx: &mut InlineContext,
    reserved_block_y: &mut f32,
    x_base: f32,
    y_base: f32,
    previous_margin_bottom: f32,
) -> (f32, f32) {
    let computed_style = element.computed_style.as_ref().unwrap();
    let is_inline = matches!(formatting_context, FormattingContext::InlineContainer | FormattingContext::TextNode);
    let is_block = matches!(formatting_context, FormattingContext::BlockContainer);

    if let Some(prev) = prev_element {
        let prev_flow = prev.computed_flow.as_ref().unwrap();
        let prev_is_inline = matches!(prev_formatting_context, Some(FormattingContext::InlineContainer | FormattingContext::TextNode));

        // Continue inline context if both current and previous are inline
        if is_inline && prev_is_inline {
            if inline_ctx.active {
                // Continue existing inline line - inline_ctx.x was updated by update_layout_state
                // from previous element: prev_x + prev_width + prev_margin.right
                inline_ctx.continue_line(prev_flow.y, prev_flow.height);
                // Return base position (margin.left will be added in reflow)
                return (inline_ctx.x, inline_ctx.y);
            } else {
                // Starting new inline context - calculate from previous element's continue_x
                // continue_x = prev_x + prev_width + prev_margin.right (calculated after prev was laid out)
                let prev_style = prev.computed_style.as_ref().unwrap();
                // For inline-block/float, calculate from x + width + margin.right
                // For regular inline, use continue_x which already accounts for everything
                let next_x = if prev.computed_style.as_ref().unwrap().display == "inline-block" 
                    || prev.computed_style.as_ref().unwrap().float != "none"
                    || prev.computed_style.as_ref().unwrap().display == "inline-flex" {
                    prev_flow.x + prev_flow.width + prev_style.margin.right
                } else {
                    prev_flow.continue_x
                };
                inline_ctx.start_new_line(next_x, prev_flow.y);
                // Return base position (margin.left will be added in reflow)
                return (inline_ctx.x, inline_ctx.y);
            }
        }

        // Block element or new inline context after block
        inline_ctx.break_context();
        *reserved_block_y = f32::max(*reserved_block_y, prev_flow.y + prev_flow.height);

        if is_block {
            let y = *reserved_block_y + f32::max(computed_style.margin.bottom, computed_style.margin.top);
            return (x_base, y);
        } else {
            // Starting new inline context after block
            inline_ctx.start_new_line(x_base, *reserved_block_y);
            return (x_base, *reserved_block_y);
        }
    } else {
        // First element in context
        let y = y_base + f32::max(0.0, computed_style.margin.top - previous_margin_bottom);
        if is_inline {
            inline_ctx.start_new_line(x_base, y);
            return (x_base, y);
        } else {
            return (x_base, y);
        }
    }
}

/// Helper to update layout state after positioning an element
fn update_layout_state(
    formatting_context: FormattingContext,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    margin_bottom: f32,
    margin_right: f32,
    adjacent_margin_bottom: f32,
    inline_ctx: &mut InlineContext,
    reserved_block_y: &mut f32,
    context: &mut ReflowContext,
) {
    match formatting_context {
        FormattingContext::BlockContainer => {
            *reserved_block_y = f32::max(height + y + margin_bottom, *reserved_block_y);
            context.adjacent_margin_bottom = adjacent_margin_bottom;
            inline_ctx.break_context();
        }
        FormattingContext::InlineContainer | FormattingContext::TextNode => {
            // Update inline_x for next element: current x (which includes margin.left) + width + right margin
            // x already includes margin.left (added in reflow), so we use it directly
            inline_ctx.x = x + width + margin_right;
            inline_ctx.line_height = inline_ctx.line_height.max(height);
            *reserved_block_y = f32::max(height + y, *reserved_block_y);
            context.adjacent_margin_bottom = 0.0;
        }
        FormattingContext::FlexContainer | FormattingContext::GridContainer => {
            // TODO: Implement flex/grid layout
            // For now, treat as block container
            *reserved_block_y = f32::max(height + y + margin_bottom, *reserved_block_y);
            context.adjacent_margin_bottom = adjacent_margin_bottom;
            inline_ctx.break_context();
        }
    }
}

// Helper to create a test element with computed style
fn create_test_element(
    tag_name: &str,
    display: &str,
    margin: ComputedMargin,
    padding: ComputedMargin,
) -> DomElement {
    let mut element = DomElement::new(NodeType::Element);
    element.tag_name = tag_name.to_uppercase();
    // Set display on element.style so it's preserved through propagate_styles
    set_display(&mut element.style, display);
    // Set padding on element.style so it's used by the layout pipeline
    set_padding(&mut element.style, padding.top, padding.right, padding.bottom, padding.left);
    // Set margin on element.style
    set_margin(&mut element.style, margin.top, margin.right, margin.bottom, margin.left);
    element.computed_style = Some(ComputedStyle {
        margin: margin.clone(),
        padding: padding.clone(),
        background_color: (0.0, 0.0, 0.0, 0.0),
        color: (0.0, 0.0, 0.0, 1.0),
        font_size: 16.0,
        font_family: "Times New Roman".to_string(),
        font_weight: 400,
        font_style: "normal".to_string(),
        text_decoration: "none".to_string(),
        display: display.to_string(),
        float: "none".to_string(),
        position: "static".to_string(),
        inset: ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        width: 0.0,
        height: 0.0,
        white_space: "normal".to_string(),
        visibility: "visible".to_string(),
    });
    element
}

// Helper to create a text node
fn create_text_node(text: &str) -> DomElement {
    let mut element = DomElement::new(NodeType::Text);
    element.node_value = text.to_string();
    element.computed_style = Some(ComputedStyle {
        margin: ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        padding: ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        background_color: (0.0, 0.0, 0.0, 0.0),
        color: (0.0, 0.0, 0.0, 1.0),
        font_size: 16.0,
        font_family: "Times New Roman".to_string(),
        font_weight: 400,
        font_style: "normal".to_string(),
        text_decoration: "none".to_string(),
        display: "inline".to_string(),
        float: "none".to_string(),
        position: "static".to_string(),
        inset: ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        width: 0.0,
        height: 0.0,
        white_space: "normal".to_string(),
        visibility: "visible".to_string(),
    });
    element
}

#[test]
fn test_formatting_context_block() {
    let element = create_test_element("div", "block", 
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    assert_eq!(get_formatting_context(&element), FormattingContext::BlockContainer);
}

#[test]
fn test_formatting_context_inline() {
    let element = create_test_element("span", "inline",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    assert_eq!(get_formatting_context(&element), FormattingContext::InlineContainer);
}

#[test]
fn test_formatting_context_inline_block() {
    let element = create_test_element("span", "inline-block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    assert_eq!(get_formatting_context(&element), FormattingContext::InlineContainer);
}

#[test]
fn test_formatting_context_text_node() {
    let element = create_text_node("Hello");
    assert_eq!(get_formatting_context(&element), FormattingContext::TextNode);
}

#[test]
fn test_compute_element_position_first_block() {
    let element = create_test_element("div", "block",
        ComputedMargin { top: 10.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    let mut inline_ctx = InlineContext::new(0.0, 0.0);
    let mut reserved_block_y = 0.0;
    
    let (x, y) = compute_element_position(
        &element,
        None,
        FormattingContext::BlockContainer,
        None,
        &mut inline_ctx,
        &mut reserved_block_y,
        0.0,
        0.0,
        0.0,
    );
    
    assert_eq!(x, 0.0);
    assert_eq!(y, 10.0); // y_base + margin.top
}

#[test]
fn test_compute_element_position_block_after_block() {
    let mut prev = create_test_element("div", "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 10.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    prev.computed_flow = Some(ComputedFlow {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 50.0,
        adjacent_margin_bottom: 0.0,
        hover_rect: Rect { x: 0.0, y: 0.0, width: 100.0, height: 50.0 },
        continue_x: 0.0,
        continue_y: 0.0,
    });

    let element = create_test_element("div", "block",
        ComputedMargin { top: 20.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    
    let mut inline_ctx = InlineContext::new(0.0, 0.0);
    let mut reserved_block_y = 0.0;
    
    let (x, y) = compute_element_position(
        &element,
        Some(&prev),
        FormattingContext::BlockContainer,
        Some(FormattingContext::BlockContainer),
        &mut inline_ctx,
        &mut reserved_block_y,
        0.0,
        0.0,
        0.0,
    );
    
    assert_eq!(x, 0.0);
    // Should be at prev.y + prev.height + max(prev.margin.bottom, element.margin.top)
    assert_eq!(y, 50.0 + 20.0); // 50 (prev height) + 20 (max of margins)
    assert_eq!(reserved_block_y, 50.0); // Updated to prev.y + prev.height
}

#[test]
fn test_compute_element_position_inline_after_inline() {
    let mut prev = create_test_element("span", "inline",
        ComputedMargin { top: 0.0, right: 5.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    prev.computed_flow = Some(ComputedFlow {
        x: 0.0,
        y: 0.0,
        width: 50.0,
        height: 16.0,
        adjacent_margin_bottom: 0.0,
        hover_rect: Rect { x: 0.0, y: 0.0, width: 50.0, height: 16.0 },
        continue_x: 55.0, // x + width + margin.right = 0 + 50 + 5
        continue_y: 0.0,
    });

    let element = create_test_element("span", "inline",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 10.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    
    let mut inline_ctx = InlineContext::new(0.0, 0.0);
    inline_ctx.active = true;
    inline_ctx.x = 55.0; // prev.x + prev.width + prev.margin.right
    
    let mut reserved_block_y = 0.0;
    
    let (x, y) = compute_element_position(
        &element,
        Some(&prev),
        FormattingContext::InlineContainer,
        Some(FormattingContext::InlineContainer),
        &mut inline_ctx,
        &mut reserved_block_y,
        0.0,
        0.0,
        0.0,
    );
    
    // compute_element_position returns base position (without margin.left)
    // margin.left (10.0) will be added in reflow, so we expect just inline_ctx.x
    assert_eq!(x, 55.0); // inline_ctx.x (margin.left added later in reflow)
    assert_eq!(y, 0.0); // Same y as previous
}

#[test]
fn test_update_layout_state_block() {
    let mut inline_ctx = InlineContext::new(0.0, 0.0);
    let mut reserved_block_y = 0.0;
    let mut context = ReflowContext {
        x: 0.0,
        y: 0.0,
        rel_x: 0.0,
        rel_y: 0.0,
        font_size: 16.0,
        parent_width: 100.0,
        parent_height: 100.0,
        parent_max_width: 100.0,
        layout_x_start: None,
        adjacent_margin_bottom: 0.0,
        shrink_to_fit: false,
    };

    update_layout_state(
        FormattingContext::BlockContainer,
        0.0,
        10.0,
        100.0,
        50.0,
        5.0, // margin_bottom
        0.0, // margin_right
        5.0, // adjacent_margin_bottom
        &mut inline_ctx,
        &mut reserved_block_y,
        &mut context,
    );

    assert_eq!(reserved_block_y, 65.0); // y + height + margin_bottom = 10 + 50 + 5
    assert_eq!(context.adjacent_margin_bottom, 5.0);
    assert!(!inline_ctx.active); // Should break context
}

#[test]
fn test_update_layout_state_inline() {
    let mut inline_ctx = InlineContext::new(0.0, 0.0);
    let mut reserved_block_y = 0.0;
    let mut context = ReflowContext {
        x: 0.0,
        y: 0.0,
        rel_x: 0.0,
        rel_y: 0.0,
        font_size: 16.0,
        parent_width: 100.0,
        parent_height: 100.0,
        parent_max_width: 100.0,
        layout_x_start: None,
        adjacent_margin_bottom: 0.0,
        shrink_to_fit: false,
    };

    update_layout_state(
        FormattingContext::InlineContainer,
        0.0,
        10.0,
        50.0,
        16.0,
        0.0, // margin_bottom
        5.0, // margin_right
        0.0, // adjacent_margin_bottom
        &mut inline_ctx,
        &mut reserved_block_y,
        &mut context,
    );

    assert_eq!(inline_ctx.x, 55.0); // x + width + margin_right = 0 + 50 + 5
    assert_eq!(inline_ctx.line_height, 16.0);
    assert_eq!(reserved_block_y, 26.0); // y + height = 10 + 16
    assert_eq!(context.adjacent_margin_bottom, 0.0);
}

// Mock TextMeasurer for integration tests
struct MockTextMeasurer;
 
impl TextMeasurer for MockTextMeasurer {
    fn measure(&mut self, text: &str, font_size: f32, _font_family: &str) -> (f32, f32) {
        // Simple approximation: each character is roughly 0.6 * font_size wide
        let width = text.len() as f32 * font_size * 0.6;
        let height = font_size + 8.0;
        (width, height)
    }
}

fn css_px(value: f32) -> CssValue {
    CssValue::Size(CssSize {
        value,
        unit: CssSizeUnit::Px,
    })
}

fn set_display(style: &mut crate::styles::Style, value: &str) {
    style.display = StringProperty::new(
        CssValue::String(value.to_string()),
        false,
        "inline",
    );
}

fn set_padding(style: &mut crate::styles::Style, top: f32, right: f32, bottom: f32, left: f32) {
    style.padding = Margin::new(css_px(top), css_px(right), css_px(bottom), css_px(left));
}

fn set_margin(style: &mut crate::styles::Style, top: f32, right: f32, bottom: f32, left: f32) {
    style.margin = Margin::new(css_px(top), css_px(right), css_px(bottom), css_px(left));
}

#[test]
fn test_inline_items_wrap_when_line_overflows() {
    let mut topbar = DomElement::new(NodeType::Element);
    topbar.tag_name = "DIV".to_string();
    set_display(&mut topbar.style, "block");
    set_padding(&mut topbar.style, 10.0, 10.0, 10.0, 10.0);

    let make_nav_item = |label: &str| {
        let mut item = DomElement::new(NodeType::Element);
        item.tag_name = "SPAN".to_string();
        set_display(&mut item.style, "inline-block");
        set_padding(&mut item.style, 6.0, 10.0, 6.0, 10.0);
        let text = create_text_node(label);
        item.children.push(Rc::new(RefCell::new(text)));
        Rc::new(RefCell::new(item))
    };

    let nav1 = make_nav_item("Dashboard");
    let nav2 = make_nav_item("Projects");
    let nav3 = make_nav_item("Settings");
    let nav4 = make_nav_item("Help");

    topbar.children.push(nav1.clone());
    topbar.children.push(nav2.clone());
    topbar.children.push(nav3.clone());
    topbar.children.push(nav4.clone());

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(topbar))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: 200.0,
        height: 200.0,
    };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let first_y = nav1.borrow().computed_flow.as_ref().unwrap().y;
    let last_y = nav4.borrow().computed_flow.as_ref().unwrap().y;
    assert!(
        last_y > first_y,
        "expected inline items to wrap to a new line when overflowing"
    );
}

#[test]
fn test_inline_block_wraps_to_next_line_when_item_too_wide() {
    let mut topbar = DomElement::new(NodeType::Element);
    topbar.tag_name = "DIV".to_string();
    set_display(&mut topbar.style, "block");

    let make_nav_item = |label: &str| {
        let mut item = DomElement::new(NodeType::Element);
        item.tag_name = "SPAN".to_string();
        set_display(&mut item.style, "inline-block");
        set_padding(&mut item.style, 6.0, 10.0, 6.0, 10.0);
        let text = create_text_node(label);
        item.children.push(Rc::new(RefCell::new(text)));
        Rc::new(RefCell::new(item))
    };

    let nav1 = make_nav_item("Dashboard");
    let nav2 = make_nav_item("Settings");

    topbar.children.push(nav1.clone());
    topbar.children.push(nav2.clone());

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(topbar))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: 180.0,
        height: 200.0,
    };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let nav1_y = nav1.borrow().computed_flow.as_ref().unwrap().y;
    let nav2_y = nav2.borrow().computed_flow.as_ref().unwrap().y;
    assert!(
        nav2_y > nav1_y,
        "expected second inline-block to wrap to a new line when it does not fit"
    );
}

#[test]
fn test_inline_block_text_wraps_within_remaining_width() {
    let mut topbar = DomElement::new(NodeType::Element);
    topbar.tag_name = "DIV".to_string();
    set_display(&mut topbar.style, "block");

    let make_nav_item = |label: &str| {
        let mut item = DomElement::new(NodeType::Element);
        item.tag_name = "SPAN".to_string();
        set_display(&mut item.style, "inline-block");
        set_padding(&mut item.style, 6.0, 10.0, 6.0, 10.0);
        let text = create_text_node(label);
        item.children.push(Rc::new(RefCell::new(text)));
        Rc::new(RefCell::new(item))
    };

    let nav1 = make_nav_item("Sidebar");
    let nav2 = make_nav_item("Learn more");

    topbar.children.push(nav1.clone());
    topbar.children.push(nav2.clone());

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(topbar))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: 90.0,
        height: 200.0,
    };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let nav2_flow = nav2.borrow().computed_flow.as_ref().unwrap().clone();
    let lines = nav2.borrow().children[0].borrow().lines.clone();
    assert!(
        lines.len() > 1,
        "expected inline-block text to wrap within remaining width"
    );
    for (i, line) in lines.iter().enumerate() {
        assert!(
            line.width <= nav2_flow.width + 1.0,
            "line {} width ({}) should not exceed inline-block width ({})",
            i,
            line.width,
            nav2_flow.width
        );
    }
}

#[test]
fn test_inline_block_text_does_not_wrap_when_width_available() {
    let mut topbar = DomElement::new(NodeType::Element);
    topbar.tag_name = "DIV".to_string();
    set_display(&mut topbar.style, "block");

    let make_nav_item = |label: &str| {
        let mut item = DomElement::new(NodeType::Element);
        item.tag_name = "SPAN".to_string();
        set_display(&mut item.style, "inline-block");
        set_padding(&mut item.style, 6.0, 10.0, 6.0, 10.0);
        let text = create_text_node(label);
        item.children.push(Rc::new(RefCell::new(text)));
        Rc::new(RefCell::new(item))
    };

    let nav1 = make_nav_item("Sidebar");
    let nav2 = make_nav_item("Learn more");

    topbar.children.push(nav1.clone());
    topbar.children.push(nav2.clone());

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(topbar))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: 260.0,
        height: 200.0,
    };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let lines = nav2.borrow().children[0].borrow().lines.clone();
    assert_eq!(
        lines.len(),
        1,
        "expected inline-block text to stay on one line when it fits"
    );
}

#[test]
fn test_inline_block_does_not_wrap_for_small_overflow() {
    let mut topbar = DomElement::new(NodeType::Element);
    topbar.tag_name = "DIV".to_string();
    set_display(&mut topbar.style, "block");

    let make_nav_item = |label: &str| {
        let mut item = DomElement::new(NodeType::Element);
        item.tag_name = "SPAN".to_string();
        set_display(&mut item.style, "inline-block");
        set_padding(&mut item.style, 6.0, 10.0, 6.0, 10.0);
        let text = create_text_node(label);
        item.children.push(Rc::new(RefCell::new(text)));
        Rc::new(RefCell::new(item))
    };

    let nav1 = make_nav_item("Dashboard");
    let nav2 = make_nav_item("Settings");

    topbar.children.push(nav1.clone());
    topbar.children.push(nav2.clone());

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(topbar))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let dashboard_width = "Dashboard".len() as f32 * 16.0 * 0.6 + 20.0;
    let settings_width = "Settings".len() as f32 * 16.0 * 0.6 + 20.0;
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: dashboard_width + settings_width - 4.0,
        height: 200.0,
    };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let nav1_y = nav1.borrow().computed_flow.as_ref().unwrap().y;
    let nav2_y = nav2.borrow().computed_flow.as_ref().unwrap().y;
    assert!(
        (nav2_y - nav1_y).abs() < 0.1,
        "expected inline-blocks to stay on the same line with small overflow slack"
    );
}

#[test]
fn test_inline_block_does_not_exceed_viewport_width() {
    let mut body = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    body.inherited_style = Some(crate::styles::Style::new());

    let mut content = create_test_element(
        "div",
        "inline-block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 12.0, right: 12.0, bottom: 12.0, left: 12.0 },
    );
    content.inherited_style = Some(crate::styles::Style::new());

    let mut card = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 10.0, right: 10.0, bottom: 10.0, left: 10.0 },
    );
    card.inherited_style = Some(crate::styles::Style::new());
    let card_text = create_text_node(
        "This is a long block of text that should wrap within the content area.",
    );
    card.children.push(Rc::new(RefCell::new(card_text)));

    content.children.push(Rc::new(RefCell::new(card)));
    body.children.push(Rc::new(RefCell::new(content)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(body))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: 200.0,
        height: 400.0,
    };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let content_flow = tree[0].borrow().children[0]
        .borrow()
        .computed_flow
        .as_ref()
        .unwrap()
        .clone();
    assert!(
        content_flow.x + content_flow.width <= viewport.width + 1.0,
        "Inline-block content should not exceed viewport width"
    );
}

#[test]
fn test_inline_block_wraps_when_min_content_exceeds_remaining_width() {
    let mut container = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    container.inherited_style = Some(crate::styles::Style::new());

    let mut sidebar = create_test_element(
        "span",
        "inline-block",
        ComputedMargin { top: 0.0, right: 10.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 4.0, right: 8.0, bottom: 4.0, left: 8.0 },
    );
    sidebar.inherited_style = Some(crate::styles::Style::new());
    let sidebar_text = create_text_node("Nav");
    sidebar.children.push(Rc::new(RefCell::new(sidebar_text)));

    let mut content = create_test_element(
        "span",
        "inline-block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 4.0, right: 8.0, bottom: 4.0, left: 8.0 },
    );
    content.inherited_style = Some(crate::styles::Style::new());
    let content_text = create_text_node("Supercalifragilisticexpialidocious");
    content.children.push(Rc::new(RefCell::new(content_text)));

    container.children.push(Rc::new(RefCell::new(sidebar)));
    container.children.push(Rc::new(RefCell::new(content)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(container))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: 200.0,
        height: 200.0,
    };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let sidebar_flow = tree[0].borrow().children[0]
        .borrow()
        .computed_flow
        .as_ref()
        .unwrap()
        .clone();
    let content_flow = tree[0].borrow().children[1]
        .borrow()
        .computed_flow
        .as_ref()
        .unwrap()
        .clone();
    let word_width = "Supercalifragilisticexpialidocious".len() as f32 * 16.0 * 0.6;

    assert!(
        content_flow.width >= word_width - 1.0,
        "Inline-block width ({}) should not shrink below min-content ({})",
        content_flow.width,
        word_width
    );
    // The content inline-block should wrap to a new line since its min-content width
    // (the long word) exceeds the remaining width after the sidebar
    assert!(
        content_flow.y > sidebar_flow.y + 0.1,
        "Content inline-block should wrap to a new line. sidebar_y={}, content_y={}",
        sidebar_flow.y,
        content_flow.y
    );

    // After wrapping, content should start at x=0 (container's left edge)
    assert!(
        content_flow.x < sidebar_flow.x + 1.0,
        "After wrapping, content should start at container's left edge. content_x={}",
        content_flow.x
    );
}

/// Test that inline-blocks stay side by side when they fit within the viewport
#[test]
fn test_inline_blocks_side_by_side_when_they_fit() {
    let mut container = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    container.inherited_style = Some(crate::styles::Style::new());

    let mut sidebar = create_test_element(
        "span",
        "inline-block",
        ComputedMargin { top: 0.0, right: 10.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 4.0, right: 8.0, bottom: 4.0, left: 8.0 },
    );
    sidebar.inherited_style = Some(crate::styles::Style::new());
    let sidebar_text = create_text_node("Nav");
    sidebar.children.push(Rc::new(RefCell::new(sidebar_text)));

    let mut content = create_test_element(
        "span",
        "inline-block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 4.0, right: 8.0, bottom: 4.0, left: 8.0 },
    );
    content.inherited_style = Some(crate::styles::Style::new());
    // Use a shorter word that will fit
    let content_text = create_text_node("Dashboard");
    content.children.push(Rc::new(RefCell::new(content_text)));

    container.children.push(Rc::new(RefCell::new(sidebar)));
    container.children.push(Rc::new(RefCell::new(content)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(container))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    // Use a wide viewport so both inline-blocks fit
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: 400.0,
        height: 200.0,
    };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let sidebar_flow = tree[0].borrow().children[0]
        .borrow()
        .computed_flow
        .as_ref()
        .unwrap()
        .clone();
    let content_flow = tree[0].borrow().children[1]
        .borrow()
        .computed_flow
        .as_ref()
        .unwrap()
        .clone();

    // Both inline-blocks should be on the same line (same y position)
    assert!(
        (content_flow.y - sidebar_flow.y).abs() < 1.0,
        "Inline-blocks should be on the same line when they fit. sidebar_y={}, content_y={}",
        sidebar_flow.y,
        content_flow.y
    );

    // Content should be positioned after the sidebar
    assert!(
        content_flow.x > sidebar_flow.x + sidebar_flow.width,
        "Content should be positioned after sidebar. sidebar_x={}, sidebar_width={}, content_x={}",
        sidebar_flow.x,
        sidebar_flow.width,
        content_flow.x
    );
}

/// Test that inline-block wraps based on nested content's min-content width including padding
#[test]
fn test_inline_block_wraps_with_nested_padded_content() {
    let mut container = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    container.inherited_style = Some(crate::styles::Style::new());

    let mut sidebar = create_test_element(
        "span",
        "inline-block",
        ComputedMargin { top: 0.0, right: 10.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 4.0, right: 8.0, bottom: 4.0, left: 8.0 },
    );
    sidebar.inherited_style = Some(crate::styles::Style::new());
    let sidebar_text = create_text_node("Nav");
    sidebar.children.push(Rc::new(RefCell::new(sidebar_text)));

    // Content has nested blocks with padding
    let mut content = create_test_element(
        "div",
        "inline-block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 4.0, right: 12.0, bottom: 4.0, left: 12.0 }, // 24px horizontal padding
    );
    content.inherited_style = Some(crate::styles::Style::new());

    // Nested card with padding
    let mut card = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 10.0, right: 10.0, bottom: 10.0, left: 10.0 }, // 20px horizontal padding
    );
    card.inherited_style = Some(crate::styles::Style::new());
    // Use a long word so min-content is based on it
    let card_text = create_text_node("Notifications"); // 13 chars = 124.8px at 16px * 0.6
    card.children.push(Rc::new(RefCell::new(card_text)));
    content.children.push(Rc::new(RefCell::new(card)));

    container.children.push(Rc::new(RefCell::new(sidebar)));
    container.children.push(Rc::new(RefCell::new(content)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(container))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    // Word width: 13 * 16 * 0.6 = 124.8
    // Card padding: 20
    // Content padding: 24
    // Total min-content: 124.8 + 20 + 24 = 168.8
    // Sidebar text: 3 * 16 * 0.6 = 28.8
    // Sidebar padding: 16
    // Sidebar margin_right: 10
    // Sidebar total: 28.8 + 16 + 10 = 54.8
    // Needed: 54.8 + 168.8 + 8 (tolerance) = 231.6
    // Use viewport narrower than this to force wrapping
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: 200.0,
        height: 200.0,
    };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let sidebar_flow = tree[0].borrow().children[0]
        .borrow()
        .computed_flow
        .as_ref()
        .unwrap()
        .clone();
    let content_flow = tree[0].borrow().children[1]
        .borrow()
        .computed_flow
        .as_ref()
        .unwrap()
        .clone();

    // Content should wrap to a new line because its min-content (including nested padding)
    // exceeds the remaining space after sidebar
    assert!(
        content_flow.y > sidebar_flow.y + 0.1,
        "Content should wrap when nested min-content with padding exceeds remaining space. sidebar_y={}, content_y={}",
        sidebar_flow.y,
        content_flow.y
    );
}

#[test]
fn test_inline_block_shrink_to_fit_uses_child_width() {
    let mut sidebar = DomElement::new(NodeType::Element);
    sidebar.tag_name = "DIV".to_string();
    set_display(&mut sidebar.style, "inline-block");
    set_padding(&mut sidebar.style, 6.0, 8.0, 6.0, 8.0);

    let mut label = DomElement::new(NodeType::Element);
    label.tag_name = "DIV".to_string();
    set_display(&mut label.style, "block");
    let text = create_text_node("Integrations");
    label.children.push(Rc::new(RefCell::new(text)));
    sidebar.children.push(Rc::new(RefCell::new(label)));

    let mut spacer = DomElement::new(NodeType::Element);
    spacer.tag_name = "DIV".to_string();
    set_display(&mut spacer.style, "inline-block");
    set_padding(&mut spacer.style, 6.0, 6.0, 6.0, 6.0);
    let spacer_text = create_text_node("X");
    spacer.children.push(Rc::new(RefCell::new(spacer_text)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![
        Rc::new(RefCell::new(sidebar)),
        Rc::new(RefCell::new(spacer)),
    ];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: 260.0,
        height: 200.0,
    };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let sidebar_flow = tree[0].borrow().computed_flow.as_ref().unwrap().clone();
    let expected_text_width = "Integrations".len() as f32 * 16.0 * 0.6;
    let min_width = expected_text_width + 16.0;
    assert!(
        sidebar_flow.width >= min_width - 0.5,
        "inline-block should not collapse below child text width"
    );
}

#[test]
fn test_wrap_text_respects_layout_start_offset() {
    let mut text_measurer = MockTextMeasurer;
    let lines = wrap_text(
        "Hello world".to_string(),
        70.0,
        &mut text_measurer,
        10.0,
        "Times New Roman 400.ttf".to_string(),
        10.0,
        0.0,
        0.0,
    );

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].text, "Hello");
    assert_eq!(lines[1].text, "world");
}

// Integration tests for actual reflow behavior that previously failed
// These test the scenarios that caused the layout to break

#[test]
fn test_reflow_inline_elements_horizontal_layout() {
    // Test that inline elements are positioned horizontally (the main failure case)
    let mut tree: Vec<Rc<RefCell<DomElement>>> = Vec::new();
    
    // Create two inline elements
    let mut el1 = create_test_element("span", "inline",
        ComputedMargin { top: 0.0, right: 5.0, bottom: 0.0, left: 10.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    el1.inherited_style = Some(crate::styles::Style::new());
    
    let mut el2 = create_test_element("span", "inline",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 10.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    el2.inherited_style = Some(crate::styles::Style::new());
    
    tree.push(Rc::new(RefCell::new(el1)));
    tree.push(Rc::new(RefCell::new(el2)));
    
    // Set up styles
    for i in 0..tree.len() {
        let mut element = tree[i].borrow_mut();
        let comp_style = element.inherited_style.as_ref().unwrap().to_computed_style();
        element.computed_style = Some(comp_style);
    }
    
    // Simulate what reflow does for inline elements
    // First element
    {
        let element = tree[0].borrow();
        let comp_style = element.computed_style.as_ref().unwrap();
        let formatting_ctx = get_formatting_context(&element);
        assert_eq!(formatting_ctx, FormattingContext::InlineContainer);
    }
    
    // Second element should be positioned after first
    {
        let prev = tree[0].borrow();
        let element = tree[1].borrow();
        let prev_formatting_ctx = get_formatting_context(&prev);
        let formatting_ctx = get_formatting_context(&element);
        
        assert_eq!(prev_formatting_ctx, FormattingContext::InlineContainer);
        assert_eq!(formatting_ctx, FormattingContext::InlineContainer);
        
        // Both should be inline, so they should continue horizontal layout
        let prev_is_inline = matches!(prev_formatting_ctx, FormattingContext::InlineContainer | FormattingContext::TextNode);
        let is_inline = matches!(formatting_ctx, FormattingContext::InlineContainer | FormattingContext::TextNode);
        assert!(prev_is_inline && is_inline, "Both elements should be inline for horizontal layout");
    }
}

#[test]
fn test_reflow_block_after_inline_breaks_line() {
    // Test that block element after inline breaks the line (vertical layout)
    // We use the computed_style directly since it's already set correctly by create_test_element
    let el1 = create_test_element("span", "inline",
        ComputedMargin { top: 0.0, right: 5.0, bottom: 0.0, left: 10.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    
    let el2 = create_test_element("div", "block",
        ComputedMargin { top: 10.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    
    // Check formatting contexts (get_formatting_context reads from computed_style)
    let ctx1 = get_formatting_context(&el1);
    let ctx2 = get_formatting_context(&el2);
    
    assert_eq!(ctx1, FormattingContext::InlineContainer);
    assert_eq!(ctx2, FormattingContext::BlockContainer);
    
    // Block after inline should break horizontal layout
    let prev_is_inline = matches!(ctx1, FormattingContext::InlineContainer | FormattingContext::TextNode);
    let is_block = matches!(ctx2, FormattingContext::BlockContainer);
    assert!(prev_is_inline && is_block, "Block after inline should break horizontal layout");
}

#[test]
fn test_reflow_inline_block_positioning() {
    // Test inline-block elements use x + width + margin.right positioning
    let mut el = create_test_element("span", "inline-block",
        ComputedMargin { top: 0.0, right: 5.0, bottom: 0.0, left: 10.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    el.style.margin = Margin::new(
        CssValue::Size(CssSize { value: 8.0, unit: CssSizeUnit::Px }),
        CssValue::Size(CssSize { value: 0.0, unit: CssSizeUnit::Px }),
        CssValue::Size(CssSize { value: 0.0, unit: CssSizeUnit::Px }),
        CssValue::Size(CssSize { value: 8.0, unit: CssSizeUnit::Px }),
    );
    
    let formatting_ctx = get_formatting_context(&el);
    assert_eq!(formatting_ctx, FormattingContext::InlineContainer);
    
    // inline-block should be classified as InlineContainer
    // but use special positioning logic (x + width + margin.right)
}

// Comprehensive integration tests for full reflow function

#[test]
fn test_reflow_simple_block_layout() {
    // Test simple vertical stacking of block elements
    let mut tree: Vec<Rc<RefCell<DomElement>>> = Vec::new();
    
    let mut el1 = create_test_element("div", "block",
        ComputedMargin { top: 10.0, right: 0.0, bottom: 20.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    el1.inherited_style = Some(crate::styles::Style::new());
    el1.node_value = "Block 1".to_string();
    
    let mut el2 = create_test_element("div", "block",
        ComputedMargin { top: 15.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    el2.inherited_style = Some(crate::styles::Style::new());
    el2.node_value = "Block 2".to_string();
    
    tree.push(Rc::new(RefCell::new(el1)));
    tree.push(Rc::new(RefCell::new(el2)));
    
    // Set up styles and compute flows
    propagate_styles(&mut tree, None);
    for i in 0..tree.len() {
        let mut element = tree[i].borrow_mut();
        let comp_style = element.inherited_style.as_ref().unwrap().to_computed_style();
        element.computed_style = Some(comp_style);
    }
    
    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
    
    reflow(&mut tree, &mut text_measurer, None, &viewport);
    
    // Check that elements have computed flows
    assert!(tree[0].borrow().computed_flow.is_some(), "First element should have computed flow");
    assert!(tree[1].borrow().computed_flow.is_some(), "Second element should have computed flow");
    
    // Check that elements are positioned vertically
    let el1_flow = tree[0].borrow().computed_flow.as_ref().unwrap().clone();
    let el2_flow = tree[1].borrow().computed_flow.as_ref().unwrap().clone();
    
    // First element should be at top
    assert!(el1_flow.y >= 0.0, "First block should be positioned");
    
    // Second element should be below first (or at least positioned)
    assert!(el2_flow.y >= el1_flow.y, "Second block should be at or below first");
    
    // Both should start at x=0 (block elements)
    assert_eq!(el1_flow.x, 0.0);
    assert_eq!(el2_flow.x, 0.0);
}

#[test]
fn test_reflow_block_margin_offsets_position() {
    // Block margins should offset the element's position
    let mut el = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 8.0, right: 0.0, bottom: 0.0, left: 8.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    el.style.margin = Margin::new(
        CssValue::Size(CssSize { value: 8.0, unit: CssSizeUnit::Px }),
        CssValue::Size(CssSize { value: 0.0, unit: CssSizeUnit::Px }),
        CssValue::Size(CssSize { value: 0.0, unit: CssSizeUnit::Px }),
        CssValue::Size(CssSize { value: 8.0, unit: CssSizeUnit::Px }),
    );
    el.style.display = StringProperty::new(
        CssValue::String("block".to_string()),
        false,
        "inline",
    );
    el.node_value = "Block".to_string();

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(el))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 200.0, height: 200.0 };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let flow = tree[0].borrow().computed_flow.as_ref().unwrap().clone();
    assert!(
        (flow.x - 8.0).abs() < 0.1,
        "Block x ({}) should include margin-left (8.0)",
        flow.x
    );
    assert!(
        (flow.y - 8.0).abs() < 0.1,
        "Block y ({}) should include margin-top (8.0)",
        flow.y
    );
}

#[test]
fn test_reflow_block_auto_width_fills_parent() {
    // Block elements with auto width should fill the parent's content width.
    let mut parent = create_test_element(
        "div",
        "inline-block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    parent.style.display = StringProperty::new(
        CssValue::String("block".to_string()),
        false,
        "inline",
    );

    let mut child = create_test_element(
        "span",
        "inline-block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    child.style.display = StringProperty::new(
        CssValue::String("inline-block".to_string()),
        false,
        "inline",
    );
    let text = create_text_node("Small");
    child.children.push(Rc::new(RefCell::new(text)));

    parent.children.push(Rc::new(RefCell::new(child)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(parent))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 300.0, height: 200.0 };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let flow = tree[0].borrow().computed_flow.as_ref().unwrap().clone();
    assert!(
        (flow.width - 300.0).abs() < 1.0,
        "Block width ({}) should fill parent content width (300.0)",
        flow.width
    );
}

#[test]
fn test_reflow_inline_block_shrink_to_fit_block_child() {
    // Inline-block parent should shrink to fit block child in shrink-to-fit context.
    let mut parent = create_test_element(
        "div",
        "inline-block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    parent.style.display = StringProperty::new(
        CssValue::String("inline-block".to_string()),
        false,
        "inline",
    );

    let mut child = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    child.style.display = StringProperty::new(
        CssValue::String("block".to_string()),
        false,
        "inline",
    );
    let text = create_text_node("Hello");
    child.children.push(Rc::new(RefCell::new(text)));

    parent.children.push(Rc::new(RefCell::new(child)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(parent))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 300.0, height: 200.0 };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let flow = tree[0].borrow().computed_flow.as_ref().unwrap().clone();
    assert!(
        flow.width > 30.0 && flow.width < 80.0,
        "Inline-block width ({}) should shrink to child text size",
        flow.width
    );
}

#[test]
fn test_reflow_inline_elements_full() {
    // Test full reflow with multiple inline elements (the main failure scenario)
    let mut tree: Vec<Rc<RefCell<DomElement>>> = Vec::new();
    
    for i in 0..3 {
        let mut el = create_test_element("span", "inline",
            ComputedMargin { top: 0.0, right: 5.0, bottom: 0.0, left: 5.0 },
            ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
        el.inherited_style = Some(crate::styles::Style::new());
        el.node_value = format!("Item {}", i + 1);
        tree.push(Rc::new(RefCell::new(el)));
    }
    
    propagate_styles(&mut tree, None);
    for i in 0..tree.len() {
        let mut element = tree[i].borrow_mut();
        let comp_style = element.inherited_style.as_ref().unwrap().to_computed_style();
        element.computed_style = Some(comp_style);
    }
    
    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
    
    reflow(&mut tree, &mut text_measurer, None, &viewport);
    
    // Check that elements have computed flows
    assert!(tree[0].borrow().computed_flow.is_some(), "First element should have computed flow");
    assert!(tree[1].borrow().computed_flow.is_some(), "Second element should have computed flow");
    assert!(tree[2].borrow().computed_flow.is_some(), "Third element should have computed flow");
    
    // Check that inline elements are positioned horizontally
    let el1_flow = tree[0].borrow().computed_flow.as_ref().unwrap().clone();
    let el2_flow = tree[1].borrow().computed_flow.as_ref().unwrap().clone();
    let el3_flow = tree[2].borrow().computed_flow.as_ref().unwrap().clone();
    
    // All should be on the same line (same y) - allow small differences for rounding
    assert!((el1_flow.y - el2_flow.y).abs() < 1.0, "Inline elements should be on same line");
    assert!((el2_flow.y - el3_flow.y).abs() < 1.0, "Inline elements should be on same line");
    
    // Second should be to the right of first (or at least positioned)
    assert!(el2_flow.x >= el1_flow.x, "Second inline should be at or after first");
    
    // Third should be to the right of second (or at least positioned)
    assert!(el3_flow.x >= el2_flow.x, "Third inline should be at or after second");
}

#[test]
fn test_reflow_mixed_inline_block() {
    // Test mixed inline and block elements
    let mut tree: Vec<Rc<RefCell<DomElement>>> = Vec::new();
    
    // Inline element
    let mut el1 = create_test_element("span", "inline",
        ComputedMargin { top: 0.0, right: 5.0, bottom: 0.0, left: 5.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    el1.inherited_style = Some(crate::styles::Style::new());
    el1.node_value = "Inline".to_string();
    
    // Block element (should break line)
    let mut el2 = create_test_element("div", "block",
        ComputedMargin { top: 10.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    el2.inherited_style = Some(crate::styles::Style::new());
    el2.node_value = "Block".to_string();
    
    // Another inline element (should start new line after block)
    let mut el3 = create_test_element("span", "inline",
        ComputedMargin { top: 0.0, right: 5.0, bottom: 0.0, left: 5.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    el3.inherited_style = Some(crate::styles::Style::new());
    el3.node_value = "Inline2".to_string();
    
    tree.push(Rc::new(RefCell::new(el1)));
    tree.push(Rc::new(RefCell::new(el2)));
    tree.push(Rc::new(RefCell::new(el3)));
    
    propagate_styles(&mut tree, None);
    for i in 0..tree.len() {
        let mut element = tree[i].borrow_mut();
        let comp_style = element.inherited_style.as_ref().unwrap().to_computed_style();
        element.computed_style = Some(comp_style);
    }
    
    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
    
    reflow(&mut tree, &mut text_measurer, None, &viewport);
    
    // Check that elements have computed flows
    assert!(tree[0].borrow().computed_flow.is_some(), "First element should have computed flow");
    assert!(tree[1].borrow().computed_flow.is_some(), "Second element should have computed flow");
    assert!(tree[2].borrow().computed_flow.is_some(), "Third element should have computed flow");
    
    let el1_flow = tree[0].borrow().computed_flow.as_ref().unwrap().clone();
    let el2_flow = tree[1].borrow().computed_flow.as_ref().unwrap().clone();
    let el3_flow = tree[2].borrow().computed_flow.as_ref().unwrap().clone();
    
    // Block should be below first inline (or at least positioned)
    assert!(el2_flow.y >= el1_flow.y, "Block should be at or below first inline");
    
    // Third inline should be below block (or at least positioned)
    assert!(el3_flow.y >= el2_flow.y, "Third inline should be at or below block");
    
    // Block should start at x=0
    assert_eq!(el2_flow.x, 0.0, "Block should start at x=0");
}

#[test]
fn test_reflow_nested_elements() {
    // Test nested block elements
    let mut parent = create_test_element("div", "block",
        ComputedMargin { top: 10.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 10.0, right: 10.0, bottom: 10.0, left: 10.0 });
    parent.inherited_style = Some(crate::styles::Style::new());
    
    let mut child1 = create_test_element("div", "block",
        ComputedMargin { top: 5.0, right: 0.0, bottom: 5.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    child1.inherited_style = Some(crate::styles::Style::new());
    child1.node_value = "Child 1".to_string();
    
    let mut child2 = create_test_element("div", "block",
        ComputedMargin { top: 5.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    child2.inherited_style = Some(crate::styles::Style::new());
    child2.node_value = "Child 2".to_string();
    
    parent.children.push(Rc::new(RefCell::new(child1)));
    parent.children.push(Rc::new(RefCell::new(child2)));
    
    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(parent))];
    
    propagate_styles(&mut tree, None);
    for i in 0..tree.len() {
        let mut element = tree[i].borrow_mut();
        let comp_style = element.inherited_style.as_ref().unwrap().to_computed_style();
        element.computed_style = Some(comp_style);
    }
    
    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
    
    reflow(&mut tree, &mut text_measurer, None, &viewport);
    
    // Check that elements have computed flows
    assert!(tree[0].borrow().computed_flow.is_some(), "Parent should have computed flow");
    assert!(tree[0].borrow().children[0].borrow().computed_flow.is_some(), "First child should have computed flow");
    assert!(tree[0].borrow().children[1].borrow().computed_flow.is_some(), "Second child should have computed flow");
    
    let parent_flow = tree[0].borrow().computed_flow.as_ref().unwrap().clone();
    let child1_flow = tree[0].borrow().children[0].borrow().computed_flow.as_ref().unwrap().clone();
    let child2_flow = tree[0].borrow().children[1].borrow().computed_flow.as_ref().unwrap().clone();
    
    // Children should be positioned relative to parent (padding is added to parent's x/y in context)
    // The actual positioning depends on how reflow handles padding, but children should be inside parent
    assert!(child1_flow.x >= parent_flow.x, "Child should be inside parent");
    assert!(child1_flow.y >= parent_flow.y, "Child should be inside parent");
    
    // Second child should be below first (or at least positioned)
    assert!(child2_flow.y >= child1_flow.y, "Second child should be at or below first");
}

#[test]
fn test_reflow_text_nodes() {
    // Test text nodes in inline context
    let mut tree: Vec<Rc<RefCell<DomElement>>> = Vec::new();
    
    let text1 = create_text_node("Hello ");
    let text2 = create_text_node("World");
    
    tree.push(Rc::new(RefCell::new(text1)));
    tree.push(Rc::new(RefCell::new(text2)));
    
    propagate_styles(&mut tree, None);
    for i in 0..tree.len() {
        let mut element = tree[i].borrow_mut();
        let comp_style = element.inherited_style.as_ref().unwrap().to_computed_style();
        element.computed_style = Some(comp_style);
    }
    
    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
    
    reflow(&mut tree, &mut text_measurer, None, &viewport);
    
    // Check that elements have computed flows
    assert!(tree[0].borrow().computed_flow.is_some(), "First text should have computed flow");
    assert!(tree[1].borrow().computed_flow.is_some(), "Second text should have computed flow");
    
    let text1_flow = tree[0].borrow().computed_flow.as_ref().unwrap().clone();
    let text2_flow = tree[1].borrow().computed_flow.as_ref().unwrap().clone();
    
    // Text nodes should be on same line (allow small differences)
    assert!((text1_flow.y - text2_flow.y).abs() < 1.0, "Text nodes should be on same line");
    
    // Second text should be after first (or at least positioned)
    assert!(text2_flow.x >= text1_flow.x, "Second text should be at or after first");
}

#[test]
fn test_reflow_wrapped_text_continues_inline_flow() {
    // Text wraps across lines and inline siblings should continue from the last line.
    let mut tree: Vec<Rc<RefCell<DomElement>>> = Vec::new();

    let text = create_text_node("This is a long line that should wrap");
    let mut inline = create_test_element(
        "span",
        "inline",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    inline.inherited_style = Some(crate::styles::Style::new());
    inline.node_value = "after".to_string();

    tree.push(Rc::new(RefCell::new(text)));
    tree.push(Rc::new(RefCell::new(inline)));

    propagate_styles(&mut tree, None);
    for i in 0..tree.len() {
        let mut element = tree[i].borrow_mut();
        let comp_style = element.inherited_style.as_ref().unwrap().to_computed_style();
        element.computed_style = Some(comp_style);
    }

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 100.0, height: 600.0 };

    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let text_el = tree[0].borrow();
    let inline_flow = tree[1].borrow().computed_flow.as_ref().unwrap().clone();

    assert!(text_el.lines.len() > 1, "Text should wrap into multiple lines");

    let last_line = text_el.lines.last().unwrap();
    assert!(
        inline_flow.y >= last_line.y - 1.0,
        "Inline sibling should not be placed above the last wrapped line"
    );

    let same_line = (inline_flow.y - last_line.y).abs() < 1.0;
    if same_line {
        assert!(
            inline_flow.x >= last_line.x + last_line.width - 1.0,
            "Inline sibling should be positioned after the last line of text"
        );
    } else {
        assert!(
            inline_flow.y > last_line.y,
            "Inline sibling should move to a new line after wrapped text"
        );
    }
}

#[test]
fn test_reflow_inline_elements_with_text_children_width() {
    // Inline elements with text children should advance the inline cursor by child width.
    let mut tree: Vec<Rc<RefCell<DomElement>>> = Vec::new();

    let mut el1 = create_test_element(
        "span",
        "inline",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    el1.inherited_style = Some(crate::styles::Style::new());
    let text1 = create_text_node("Create");
    el1.children.push(Rc::new(RefCell::new(text1)));

    let mut el2 = create_test_element(
        "span",
        "inline",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    el2.inherited_style = Some(crate::styles::Style::new());
    let text2 = create_text_node("Export");
    el2.children.push(Rc::new(RefCell::new(text2)));

    tree.push(Rc::new(RefCell::new(el1)));
    tree.push(Rc::new(RefCell::new(el2)));

    propagate_styles(&mut tree, None);
    for i in 0..tree.len() {
        let mut element = tree[i].borrow_mut();
        let comp_style = element.inherited_style.as_ref().unwrap().to_computed_style();
        element.computed_style = Some(comp_style);
    }

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };

    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let el1_flow = tree[0].borrow().computed_flow.as_ref().unwrap().clone();
    let el2_flow = tree[1].borrow().computed_flow.as_ref().unwrap().clone();

    assert!(
        el2_flow.x >= el1_flow.x + el1_flow.width - 1.0,
        "Second inline should be positioned after first inline's text width"
    );
}

#[test]
fn test_reflow_parent_padding_not_double_counted() {
    // Parent padding should not be added twice to computed size.
    let mut parent = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 10.0, right: 10.0, bottom: 10.0, left: 10.0 },
    );
    {
        let mut style = crate::styles::Style::new();
        style.display = StringProperty::new(
            CssValue::String("inline-block".to_string()),
            false,
            "inline",
        );
        style.padding = Margin::new(
            CssValue::Size(CssSize { value: 10.0, unit: CssSizeUnit::Px }),
            CssValue::Size(CssSize { value: 10.0, unit: CssSizeUnit::Px }),
            CssValue::Size(CssSize { value: 10.0, unit: CssSizeUnit::Px }),
            CssValue::Size(CssSize { value: 10.0, unit: CssSizeUnit::Px }),
        );
        parent.style = style;
    }

    let mut child = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    {
        let mut style = crate::styles::Style::new();
        style.display = StringProperty::new(CssValue::String("block".to_string()), false, "inline");
        style.width = MarginComponent::new(CssValue::Size(CssSize { value: 100.0, unit: CssSizeUnit::Px }));
        style.height = MarginComponent::new(CssValue::Size(CssSize { value: 20.0, unit: CssSizeUnit::Px }));
        child.style = style;
    }

    parent.children.push(Rc::new(RefCell::new(child)));
    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(parent))];

    propagate_styles(&mut tree, None);
    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let parent_flow = tree[0].borrow().computed_flow.as_ref().unwrap().clone();
    assert!(
        (parent_flow.width - 120.0).abs() < 1.0,
        "Parent width should be child width + padding (expected ~120, got {})",
        parent_flow.width
    );
    assert!(
        (parent_flow.height - 40.0).abs() < 1.0,
        "Parent height should be child height + padding (expected ~40, got {})",
        parent_flow.height
    );
}

#[test]
fn test_reflow_inline_element_has_right_padding() {
    // Inline elements with padding should include right padding in computed width.
    let mut parent = create_test_element(
        "span",
        "inline-block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 10.0, bottom: 0.0, left: 10.0 },
    );
    {
        let mut style = crate::styles::Style::new();
        style.display = StringProperty::new(CssValue::String("inline-block".to_string()), false, "inline");
        style.padding = Margin::new(
            CssValue::Size(CssSize { value: 10.0, unit: CssSizeUnit::Px }),
            CssValue::Size(CssSize { value: 10.0, unit: CssSizeUnit::Px }),
            CssValue::Size(CssSize { value: 0.0, unit: CssSizeUnit::Px }),
            CssValue::Size(CssSize { value: 10.0, unit: CssSizeUnit::Px }),
        );
        parent.style = style;
    }

    let text = create_text_node("Saved");
    parent.children.push(Rc::new(RefCell::new(text)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(parent))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let parent_flow = tree[0].borrow().computed_flow.as_ref().unwrap().clone();
    let child_flow = tree[0]
        .borrow()
        .children[0]
        .borrow()
        .computed_flow
        .as_ref()
        .unwrap()
        .clone();

    let expected_min_width = child_flow.width + 20.0;
    assert!(
        parent_flow.width >= expected_min_width - 1.0,
        "Inline element width should include right padding (expected >= {}, got {})",
        expected_min_width,
        parent_flow.width
    );
}

#[test]
fn test_reflow_margin_collapsing() {
    // Test margin collapsing between adjacent block elements
    let mut tree: Vec<Rc<RefCell<DomElement>>> = Vec::new();
    
    let mut el1 = create_test_element("div", "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 20.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    el1.inherited_style = Some(crate::styles::Style::new());
    el1.node_value = "Block 1".to_string();
    
    let mut el2 = create_test_element("div", "block",
        ComputedMargin { top: 15.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 });
    el2.inherited_style = Some(crate::styles::Style::new());
    el2.node_value = "Block 2".to_string();
    
    tree.push(Rc::new(RefCell::new(el1)));
    tree.push(Rc::new(RefCell::new(el2)));
    
    propagate_styles(&mut tree, None);
    for i in 0..tree.len() {
        let mut element = tree[i].borrow_mut();
        let comp_style = element.inherited_style.as_ref().unwrap().to_computed_style();
        element.computed_style = Some(comp_style);
    }
    
    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
    
    reflow(&mut tree, &mut text_measurer, None, &viewport);
    
    // Check that elements have computed flows
    assert!(tree[0].borrow().computed_flow.is_some(), "First element should have computed flow");
    assert!(tree[1].borrow().computed_flow.is_some(), "Second element should have computed flow");
    
    let el1_flow = tree[0].borrow().computed_flow.as_ref().unwrap().clone();
    let el2_flow = tree[1].borrow().computed_flow.as_ref().unwrap().clone();
    
    // Check that elements are positioned (second should be at or below first)
    // Note: If elements have no content, they might have zero height, so we just check positioning
    assert!(el2_flow.y >= el1_flow.y, 
        "Second element should be at or below first: el1.y={}, el2.y={}", el1_flow.y, el2_flow.y);
    
    // If both have height, check spacing
    if el1_flow.height > 0.0 {
        let actual_gap = el2_flow.y - (el1_flow.y + el1_flow.height);
        // Gap should account for margins (could be collapsed or added)
        assert!(actual_gap >= 0.0, 
            "Gap between elements should be >= 0, got {}", actual_gap);
    }
}

#[test]
fn test_text_multi_line_width_uses_max_line() {
    // Multi-line text should use the widest line for content width.
    let mut parent = create_test_element(
        "div",
        "inline-block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    parent.style.display = StringProperty::new(
        CssValue::String("inline-block".to_string()),
        false,
        "inline",
    );

    let text = create_text_node("Dashboard Hover items to see `:HOVER` styles.");
    parent.children.push(Rc::new(RefCell::new(text)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(parent))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 200.0, height: 200.0 };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let parent_flow = tree[0].borrow().computed_flow.as_ref().unwrap().clone();
    let lines = tree[0].borrow().children[0].borrow().lines.clone();

    let max_line_width = lines
        .iter()
        .map(|l| l.width)
        .fold(0.0_f32, |acc, w| acc.max(w));

    assert!(
        parent_flow.width >= max_line_width - 1.0,
        "Parent width ({}) should be at least max line width ({})",
        parent_flow.width,
        max_line_width
    );
}

#[test]
fn test_text_wrapping_stays_within_parent_block() {
    // Test that wrapped text stays within parent block boundaries
    // This was a bug where wrapped lines would jump to x=0 instead of the parent's content area
    let mut parent = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 10.0, right: 10.0, bottom: 10.0, left: 10.0 },
    );
    {
        let mut style = crate::styles::Style::new();
        style.display = StringProperty::new(CssValue::String("block".to_string()), false, "inline");
        style.width = MarginComponent::new(CssValue::Size(CssSize { value: 150.0, unit: CssSizeUnit::Px }));
        style.padding = Margin::new(
            CssValue::Size(CssSize { value: 10.0, unit: CssSizeUnit::Px }),
            CssValue::Size(CssSize { value: 10.0, unit: CssSizeUnit::Px }),
            CssValue::Size(CssSize { value: 10.0, unit: CssSizeUnit::Px }),
            CssValue::Size(CssSize { value: 10.0, unit: CssSizeUnit::Px }),
        );
        parent.style = style;
    }

    // Long text that should wrap within the narrow parent
    let text = create_text_node("This is some long text that should definitely wrap within the narrow container");
    parent.children.push(Rc::new(RefCell::new(text)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(parent))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let parent_flow = tree[0].borrow().computed_flow.as_ref().unwrap().clone();
    let text_lines = {
        let parent = tree[0].borrow();
        let text_el = parent.children[0].borrow();
        text_el.lines.clone()
    };
    
    // Text should have wrapped into multiple lines
    assert!(text_lines.len() > 1, "Text should wrap into multiple lines, got {} lines", text_lines.len());
    
    // All lines should start at or after the parent's content area left edge
    let parent_content_left = parent_flow.x + 10.0; // parent x + padding.left
    for (i, line) in text_lines.iter().enumerate() {
        assert!(
            line.x >= parent_content_left - 1.0,
            "Line {} should start within parent content area (x={}, expected >= {})",
            i, line.x, parent_content_left
        );
    }
    
    // Lines should not extend beyond viewport when they should be wrapping
    for (i, line) in text_lines.iter().enumerate() {
        assert!(
            line.x < viewport.width,
            "Line {} should not be positioned outside viewport (x={})",
            i, line.x
        );
    }
}

#[test]
fn test_element_width_respects_viewport_at_offset() {
    // Elements positioned at an x offset should not exceed viewport width
    // This was a bug where elements would overflow because parent_max_width
    // was passed through without accounting for x position offset
    let mut container = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    container.inherited_style = Some(crate::styles::Style::new());

    // Sidebar takes up some space on the left
    let mut sidebar = create_test_element(
        "span",
        "inline-block",
        ComputedMargin { top: 0.0, right: 10.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    sidebar.inherited_style = Some(crate::styles::Style::new());
    let sidebar_text = create_text_node("Sidebar");
    sidebar.children.push(Rc::new(RefCell::new(sidebar_text)));

    // Content positioned after sidebar
    let mut content = create_test_element(
        "span",
        "inline-block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    content.inherited_style = Some(crate::styles::Style::new());
    // Use shorter text that fits: "Short text" = 10 chars * 8 = 80px
    let content_text = create_text_node("Short text");
    content.children.push(Rc::new(RefCell::new(content_text)));

    container.children.push(Rc::new(RefCell::new(sidebar)));
    container.children.push(Rc::new(RefCell::new(content)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(container))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    // Content text "Short text" = 10 chars * 8 = 80px (+ ~12px height)
    // Sidebar "Sidebar" = 7 chars * 8 = 56px + 10px margin = 66px
    // Total needed = ~146px, use 400px viewport
    let viewport = Rect { x: 0.0, y: 0.0, width: 400.0, height: 600.0 };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let container_flow = tree[0].borrow().computed_flow.as_ref().unwrap().clone();
    let sidebar_flow = tree[0].borrow().children[0].borrow().computed_flow.as_ref().unwrap().clone();
    let content_flow = tree[0].borrow().children[1].borrow().computed_flow.as_ref().unwrap().clone();

    // Content should be positioned after sidebar
    assert!(
        content_flow.x >= sidebar_flow.x + sidebar_flow.width,
        "Content should be positioned after sidebar. sidebar: x={}, w={}, content: x={}, y={}",
        sidebar_flow.x, sidebar_flow.width, content_flow.x, content_flow.y
    );

    // Content's right edge should not exceed viewport
    let content_right = content_flow.x + content_flow.width;
    assert!(
        content_right <= viewport.width + 1.0, // Allow 1px tolerance
        "Content right edge ({}) should not exceed viewport width ({})",
        content_right, viewport.width
    );
}

#[test]
fn test_inline_block_wrap_uses_remaining_width() {
    // Inline-block content after a sidebar should wrap using remaining width,
    // not double-subtract the x offset.
    let mut container = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    container.inherited_style = Some(crate::styles::Style::new());

    let mut sidebar = create_test_element(
        "span",
        "inline-block",
        ComputedMargin { top: 0.0, right: 10.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    sidebar.inherited_style = Some(crate::styles::Style::new());
    let sidebar_text = create_text_node("Sidebar");
    sidebar.children.push(Rc::new(RefCell::new(sidebar_text)));

    let mut content = create_test_element(
        "span",
        "inline-block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    content.inherited_style = Some(crate::styles::Style::new());
    // Use shorter text: "Content here" = 12 chars * 8 = 96px
    let content_text = create_text_node("Content here");
    content.children.push(Rc::new(RefCell::new(content_text)));

    container.children.push(Rc::new(RefCell::new(sidebar)));
    container.children.push(Rc::new(RefCell::new(content)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(container))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    // Content text "Content here" = 12 chars * 8 = 96px
    // Sidebar "Sidebar" = 7 chars * 8 = 56px + 10px margin = 66px
    // Total needed = ~162px, use 360px viewport
    let viewport = Rect { x: 0.0, y: 0.0, width: 360.0, height: 600.0 };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let sidebar_flow = tree[0].borrow().children[0].borrow().computed_flow.as_ref().unwrap().clone();
    let content_flow = tree[0].borrow().children[1].borrow().computed_flow.as_ref().unwrap().clone();

    // Content should be laid out after sidebar and still have a reasonable width.
    assert!(
        content_flow.x >= sidebar_flow.x + sidebar_flow.width,
        "Content should be after sidebar. sidebar: x={}, w={}, content: x={}, y={}",
        sidebar_flow.x, sidebar_flow.width, content_flow.x, content_flow.y
    );
    // Content should have reasonable width (at least the text width ~96px)
    assert!(
        content_flow.width >= 90.0,
        "Content width ({}) should use remaining width, not collapse",
        content_flow.width
    );
}

#[test]
fn test_inline_block_text_wrap_respects_parent_width() {
    let mut container = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    container.inherited_style = Some(crate::styles::Style::new());

    let mut sidebar = create_test_element(
        "span",
        "inline-block",
        ComputedMargin { top: 0.0, right: 10.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    sidebar.inherited_style = Some(crate::styles::Style::new());
    let sidebar_text = create_text_node("Nav");
    sidebar.children.push(Rc::new(RefCell::new(sidebar_text)));

    let mut content = create_test_element(
        "span",
        "inline-block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    content.inherited_style = Some(crate::styles::Style::new());

    let mut card = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    card.inherited_style = Some(crate::styles::Style::new());
    let card_text = create_text_node(
        "This is a long block of text that should wrap within the inline-block content area.",
    );
    card.children.push(Rc::new(RefCell::new(card_text)));

    content.children.push(Rc::new(RefCell::new(card)));

    container.children.push(Rc::new(RefCell::new(sidebar)));
    container.children.push(Rc::new(RefCell::new(content)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(container))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 320.0, height: 400.0 };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let content_flow = tree[0].borrow().children[1]
        .borrow()
        .computed_flow
        .as_ref()
        .unwrap()
        .clone();
    let text_lines = tree[0].borrow().children[1]
        .borrow()
        .children[0]
        .borrow()
        .children[0]
        .borrow()
        .lines
        .clone();

    assert!(
        text_lines.len() > 1,
        "Text should wrap into multiple lines"
    );

    for (i, line) in text_lines.iter().enumerate() {
        assert!(
            line.width <= content_flow.width + 1.0,
            "Line {} width ({}) should not exceed parent width ({})",
            i,
            line.width,
            content_flow.width
        );
    }
}

#[test]
fn test_inline_block_block_child_fills_parent_width() {
    let mut container = DomElement::new(NodeType::Element);
    container.tag_name = "DIV".to_string();
    set_display(&mut container.style, "block");

    let mut sidebar = DomElement::new(NodeType::Element);
    sidebar.tag_name = "DIV".to_string();
    set_display(&mut sidebar.style, "inline-block");
    let sidebar_text = create_text_node("Nav");
    sidebar.children.push(Rc::new(RefCell::new(sidebar_text)));

    let mut content = DomElement::new(NodeType::Element);
    content.tag_name = "DIV".to_string();
    set_display(&mut content.style, "inline-block");

    let mut header = DomElement::new(NodeType::Element);
    header.tag_name = "SPAN".to_string();
    set_display(&mut header.style, "inline");
    let header_text = create_text_node("This is a fairly wide title");
    header.children.push(Rc::new(RefCell::new(header_text)));

    let mut card = DomElement::new(NodeType::Element);
    card.tag_name = "DIV".to_string();
    set_display(&mut card.style, "block");
    let card_text = create_text_node("Card");
    card.children.push(Rc::new(RefCell::new(card_text)));

    content.children.push(Rc::new(RefCell::new(header)));
    content.children.push(Rc::new(RefCell::new(card)));

    container.children.push(Rc::new(RefCell::new(sidebar)));
    container.children.push(Rc::new(RefCell::new(content)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(container))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 600.0, height: 400.0 };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let content_flow = tree[0].borrow().children[1]
        .borrow()
        .computed_flow
        .as_ref()
        .unwrap()
        .clone();
    let card_flow = tree[0].borrow().children[1]
        .borrow()
        .children[1]
        .borrow()
        .computed_flow
        .as_ref()
        .unwrap()
        .clone();

    assert!(
        card_flow.width >= content_flow.width - 1.0,
        "Block child width ({}) should fill inline-block width ({})",
        card_flow.width,
        content_flow.width
    );
}

#[test]
fn test_block_child_does_not_overflow_word() {
    // Block child inside shrink-to-fit container should be at least word width.
    let mut container = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    container.inherited_style = Some(crate::styles::Style::new());

    let mut sidebar = create_test_element(
        "div",
        "inline-block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    sidebar.inherited_style = Some(crate::styles::Style::new());

    let mut item = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    item.inherited_style = Some(crate::styles::Style::new());
    let text = create_text_node("Overview");
    item.children.push(Rc::new(RefCell::new(text)));

    sidebar.children.push(Rc::new(RefCell::new(item)));
    container.children.push(Rc::new(RefCell::new(sidebar)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(container))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 300.0, height: 200.0 };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let item_flow = tree[0].borrow().children[0].borrow().children[0]
        .borrow()
        .computed_flow
        .as_ref()
        .unwrap()
        .clone();
    let lines = tree[0].borrow().children[0].borrow().children[0]
        .borrow()
        .children[0]
        .borrow()
        .lines
        .clone();
    let word_width = lines.first().map(|l| l.width).unwrap_or(0.0);

    assert!(
        item_flow.width >= word_width - 1.0,
        "Item width ({}) should fit word width ({})",
        item_flow.width,
        word_width
    );
}

#[test]
fn test_inline_block_text_measure_uses_available_width() {
    // Inline-block text should be measured against available width, not a stale content width.
    let mut container = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    container.inherited_style = Some(crate::styles::Style::new());

    let mut sidebar = create_test_element(
        "div",
        "inline-block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    sidebar.inherited_style = Some(crate::styles::Style::new());

    let mut item = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    item.inherited_style = Some(crate::styles::Style::new());
    let text = create_text_node("Overview");
    item.children.push(Rc::new(RefCell::new(text)));

    sidebar.children.push(Rc::new(RefCell::new(item)));
    container.children.push(Rc::new(RefCell::new(sidebar)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(container))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 300.0, height: 200.0 };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let sidebar_flow = tree[0].borrow().children[0].borrow().computed_flow.as_ref().unwrap().clone();
    let item_flow = tree[0].borrow().children[0].borrow().children[0]
        .borrow()
        .computed_flow
        .as_ref()
        .unwrap()
        .clone();
    let lines = tree[0].borrow().children[0].borrow().children[0]
        .borrow()
        .children[0]
        .borrow()
        .lines
        .clone();
    let word_width = lines.first().map(|l| l.width).unwrap_or(0.0);

    assert!(
        item_flow.width >= word_width - 1.0,
        "Item width ({}) should fit word width ({})",
        item_flow.width,
        word_width
    );
    assert!(
        sidebar_flow.width >= item_flow.width - 1.0,
        "Sidebar width ({}) should fit item width ({})",
        sidebar_flow.width,
        item_flow.width
    );
}

#[test]
fn test_parent_height_includes_wrapped_text_lines() {
    // Parent height should include all wrapped text lines, not just the first
    let mut parent = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    parent.inherited_style = Some(crate::styles::Style::new());

    // Text that will wrap into multiple lines in a narrow container
    let text = create_text_node("This is a long text that should wrap into multiple lines");
    parent.children.push(Rc::new(RefCell::new(text)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(parent))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    // Narrow viewport to force wrapping
    let viewport = Rect { x: 0.0, y: 0.0, width: 150.0, height: 600.0 };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let parent_el = tree[0].borrow();
    let text_el = parent_el.children[0].borrow();
    let parent_flow = parent_el.computed_flow.as_ref().unwrap();

    // Text should wrap into multiple lines
    assert!(
        text_el.lines.len() > 1,
        "Text should wrap into multiple lines, got {} lines",
        text_el.lines.len()
    );

    // Calculate expected height: sum of all line heights + extra space
    let expected_min_height: f32 = text_el.lines.iter().map(|l| l.height).sum();
    
    // Parent height should accommodate all text lines
    assert!(
        parent_flow.height >= expected_min_height - 1.0,
        "Parent height ({}) should accommodate all text lines (expected >= {})",
        parent_flow.height, expected_min_height
    );
}

#[test]
fn test_text_lines_stay_within_parent_height() {
    // Text line positions should remain within the parent box after final layout
    let mut parent = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    parent.inherited_style = Some(crate::styles::Style::new());

    let text = create_text_node("Hover items to see `:HOVER` styles. This page is built using only the CSS your engine supports.");
    parent.children.push(Rc::new(RefCell::new(text)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(parent))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 260.0, height: 600.0 };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let parent_flow = tree[0].borrow().computed_flow.as_ref().unwrap().clone();
    let lines = tree[0].borrow().children[0].borrow().lines.clone();

    assert!(
        lines.len() > 1,
        "Text should wrap into multiple lines, got {} lines",
        lines.len()
    );

    for line in lines {
        let line_bottom = line.y + line.height;
        assert!(
            line_bottom <= parent_flow.y + parent_flow.height + 1.0,
            "Line bottom ({}) should stay within parent height ({} .. {})",
            line_bottom,
            parent_flow.y,
            parent_flow.y + parent_flow.height
        );
    }
}

#[test]
fn test_text_no_leading_space_after_wrap() {
    // Test that text lines don't have leading spaces after wrapping
    let mut parent = create_test_element(
        "div",
        "block",
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
        ComputedMargin { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
    );
    {
        let mut style = crate::styles::Style::new();
        style.display = StringProperty::new(CssValue::String("block".to_string()), false, "inline");
        style.width = MarginComponent::new(CssValue::Size(CssSize { value: 80.0, unit: CssSizeUnit::Px }));
        parent.style = style;
    }

    let text = create_text_node("Hello World Test");
    parent.children.push(Rc::new(RefCell::new(text)));

    let mut tree: Vec<Rc<RefCell<DomElement>>> = vec![Rc::new(RefCell::new(parent))];
    propagate_styles(&mut tree, None);

    let mut text_measurer = MockTextMeasurer;
    let viewport = Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
    reflow(&mut tree, &mut text_measurer, None, &viewport);

    let text_lines = {
        let parent = tree[0].borrow();
        let text_el = parent.children[0].borrow();
        text_el.lines.clone()
    };
    
    // Check that no line starts with a space
    for (i, line) in text_lines.iter().enumerate() {
        assert!(
            !line.text.starts_with(' '),
            "Line {} should not start with a space: '{}'",
            i, line.text
        );
    }
}
