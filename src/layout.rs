use crate::colors::*;
use crate::css::*;
use crate::html::*;
use crate::render_frame::TextMeasurer;
use crate::styles::*;
use crate::utils::*;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug)]
pub struct RenderItem {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub text_lines: Vec<TextLine>,
    pub font_size: f32,
    pub font_path: String,
    pub background_color: ColorTupleA,
    pub color: ColorTupleA,
    pub underline: bool,
}

impl RenderItem {
    pub fn new() -> RenderItem {
        RenderItem {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            font_size: 16.0,
            color: (0.0, 0.0, 0.0, 1.0),
            background_color: (0.0, 0.0, 0.0, 0.0),
            font_path: S(""),
            text_lines: vec![],
            underline: false,
        }
    }
}

pub fn hit_test_element(element: &DomElement, x: f32, y: f32) -> bool {
    if element.computed_flow.is_none() {
        return false;
    }
    return rect_contains(&element.computed_flow.as_ref().unwrap().hover_rect, x, y);
}

pub fn rect_contains(rect: &Rect, x: f32, y: f32) -> bool {
    return x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height;
}

pub fn is_in_viewport(rect: &Rect, viewport: &Rect) -> bool {
    return rect.x + rect.width >= viewport.x
        && rect.x <= viewport.x + viewport.width
        && rect.y + rect.height >= viewport.y
        && rect.y <= viewport.y + viewport.height;
}

pub fn should_rerender(
    mouse_x: f32,
    mouse_y: f32,
    tree: &mut Vec<DomElement>,
    style: &Vec<StyleRule>,
) -> bool {
    let mut dirty = false;

    let elements_len = tree.len();

    for i in 0..elements_len {
        let mut hovered = false;
        let mut element = &mut tree[i];

        if element.computed_style.is_none() {
            continue;
        }

        // if element.computed_style.as_ref().unwrap().hoverable {
        //     hovered = hit_test_element(&element, mouse_x, mouse_y);
        // }

        if hovered != element.is_hovered {
            element.is_hovered = hovered;
            dirty = true;
        }

        if element.children.len() > 0 {
            if should_rerender(mouse_x, mouse_y, &mut element.children, style) {
                return true;
            }
        }
    }

    return dirty;
}

pub fn element_matches_hover_selector(element: &DomElement, selector: &str) -> bool {
    selector.to_uppercase() == element.tag_name.clone() + ":HOVER"
}

pub fn get_element_at(tree: &Vec<DomElement>, x: f32, y: f32) -> Option<&DomElement> {
    let elements_len = tree.len();

    for i in 0..elements_len {
        let element = &tree[i];
        let computed_flow = element.computed_flow.as_ref();
        if computed_flow.is_none() {
            continue;
        }
        let computed_flow = computed_flow.unwrap();
        let rect = Rect {
            x: computed_flow.x,
            y: computed_flow.y,
            width: computed_flow.width,
            height: computed_flow.height,
        };

        let element = &tree[i];

        if element.children.len() > 0 {
            let child = get_element_at(&element.children, x, y);
            if child.is_some() {
                return child;
            } else if rect_contains(&rect, x, y) {
                return Some(element);
            }
        } else if rect_contains(&rect, x, y) && element.node_type != NodeType::Text {
            return Some(element);
        }
    }

    return None;
}

pub fn element_matches_selector(
    element: &DomElement,
    selector: &CssSelector,
    parents: &[*mut DomElement],
) -> bool {
    match selector {
        CssSelector::Tag(tag) => element.tag_name.eq_ignore_ascii_case(tag) || tag == "*",
        CssSelector::Id(id) => element.attributes.get("id").map_or(false, |v| v == id),
        CssSelector::Class(class) => {
            element.class_list.iter().any(|c| c == class)
            // element.attributes.get("class").map_or(false, |v| v.split_whitespace().any(|c| c == class))
        }
        CssSelector::Attribute {
            name,
            operator,
            value,
        } => match operator {
            Some(op) => match element.attributes.get(name) {
                Some(attr_value) => match op.as_str() {
                    "=" => value.is_some() && attr_value == value.as_deref().unwrap(),
                    "~=" => attr_value
                        .split_whitespace()
                        .any(|part| part == value.as_deref().unwrap()),
                    "|=" => {
                        attr_value == value.as_deref().unwrap()
                            || attr_value.starts_with(&(value.clone().unwrap() + "-"))
                    }
                    "^=" => attr_value.starts_with(value.as_deref().unwrap()),
                    "$=" => attr_value.ends_with(value.as_deref().unwrap()),
                    "*=" => attr_value.contains(value.as_deref().unwrap()),
                    _ => false,
                },
                None => false,
            },
            None => element.attributes.contains_key(name),
        },
        CssSelector::PseudoClass(pseudo) => {
            // Handle pseudo-classes
            false // For now, we just return true
        }
        CssSelector::PseudoElement(pseudo) => {
            // Handle pseudo-elements
            false // For now, we just return true
        }
        CssSelector::Combinator {
            combinator,
            selectors,
        } => match combinator.as_str() {
            ">" => parents.last().map_or(false, |parent| {
                selectors.iter().all(|selector| {
                    element_matches_selector(
                        unsafe { &**parent },
                        selector,
                        &parents[..parents.len() - 1],
                    )
                })
            }),
            " " => parents.iter().rev().any(|parent| {
                selectors.iter().all(|selector| {
                    element_matches_selector(
                        unsafe { &**parent },
                        selector,
                        &parents[..parents.len() - 1],
                    )
                })
            }),
            // "+" => parents.last().map_or(false, |parent| element_matches_selector(parent, selector, &parents[..parents.len()-1])),
            // "~" => parents.iter().rev().any(|parent| element_matches_selector(parent, selector, &parents[..parents.len()-1])),
            _ => false,
        },
        CssSelector::OrGroup { selectors } => {
            selectors.len() > 0
                && selectors
                    .iter()
                    .any(|s| element_matches_selector(element, s, parents))
        }
        CssSelector::AndGroup { selectors } => {
            selectors.len() > 0
                && selectors
                    .iter()
                    .all(|s| element_matches_selector(element, s, parents))
        }
        _ => false,
    }
}

pub fn compute_styles(
    tree: &mut Vec<DomElement>,
    style: &Vec<StyleRule>,
    parent_style: Option<&Style>,
    parents: &mut Vec<*mut DomElement>,
) {
    let elements_len = tree.len();

    let parent_style = match parent_style {
        Some(s) => s,
        None => &Style::new(),
    };

    for i in 0..elements_len {
        let element = &mut tree[i];
        let mut hoverable = false;
        for style_rule in style {
            if element_matches_selector(&element, &style_rule.selector, parents) {
                element.style.insert_declarations(&style_rule.declarations);
                element.matched_selectors.push(style_rule.selector.to_string());
            }
        }

        let inherited_styles = element.style.create_inherited(&parent_style);
        element.inherited_style = Some(inherited_styles);

        let element = &mut tree[i];

        if element.children.len() > 0 && element.tag_name != "SCRIPT" && element.tag_name != "STYLE"
        {
            let el: *mut DomElement = element;
            parents.push(el);
            compute_styles(
                &mut element.children,
                style,
                Some(element.inherited_style.as_ref().unwrap()),
                parents,
            );
            parents.pop();
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReflowContext {
    pub x: f32,
    pub y: f32,
    pub rel_x: f32,
    pub rel_y: f32,
    pub font_size: f32,
    pub layout_x_start: Option<f32>,
    pub adjacent_margin_bottom: f32,
}

fn is_horizontal_layout(computed_style: &Style) -> bool {
    return computed_style.display.get() == "inline-block"
        || computed_style.display.get() == "inline"
        || computed_style.float.get() != "none";
}

pub fn reflow(
    tree: &mut Vec<DomElement>,
    text_measurer: &mut dyn TextMeasurer,
    context: Option<ReflowContext>,
    viewport: &Rect,
) {
    let elements_len = tree.len();

    let mut context = context.unwrap_or(ReflowContext {
        x: 0.0,
        y: 0.0,
        rel_x: 0.0,
        rel_y: 0.0,
        font_size: 16.0,
        layout_x_start: None,
        adjacent_margin_bottom: 0.0,
    });

    let x_base = context.x;
    let y_base = context.y;

    let mut reserved_block_y = y_base;

    let mut last_element: Option<usize> = None;

    for i in 0..elements_len {
        let mut x = x_base;
        let mut y = y_base;

        let mut width = 0.0;
        let mut height = 0.0;

        let mut max_width = viewport.width;

        // let parent_style = unsafe {
        //     match tree[i].parent_node.as_ref() {
        //         Some(p) => p.inherited_style.as_ref(),
        //         None => None,
        //     }
        // };
        let style = tree[i].inherited_style.as_mut().unwrap();

        if style.display.get() == "none" {
            continue;
        }

        let scalar_context =
            ScalarEvaluationContext::from_parent(context.font_size, context.font_size);

        style.margin.evaluate(&scalar_context);
        style.padding.evaluate(&scalar_context);
        style.font_size.evaluate(&scalar_context);
        style.inset.evaluate(&scalar_context);

        let style = tree[i].inherited_style.as_ref().unwrap();

        let comp_style = style.to_computed_style();

        if comp_style.position == "absolute" || comp_style.position == "fixed" {
            if style.inset.top.has_numeric_value() {
                y = style.inset.top.get() + context.rel_y;
            }
            if style.inset.left.has_numeric_value() {
                x = style.inset.left.get() + context.rel_x;
            }
        }

        let previous_margin_bottom = context.adjacent_margin_bottom;

        if last_element.is_some()
            && (comp_style.position != "absolute" && comp_style.position != "fixed")
        {
            let previous_element = &tree[last_element.unwrap()];
            let prev_style = previous_element.inherited_style.as_ref().unwrap();
            let prev_computed_flow = previous_element.computed_flow.as_ref().unwrap();

            if is_horizontal_layout(prev_style) {
                if comp_style.display == "inline-block" || comp_style.float != "none" {
                    y = prev_computed_flow.y;
                } else {
                    y = prev_computed_flow.continue_y;
                }
            }

            let should_continue_horizontal_layout =
                is_horizontal_layout(style) && is_horizontal_layout(prev_style);

            if is_horizontal_layout(style) && should_continue_horizontal_layout {
                // Horizontal layout
                if comp_style.display == "inline-block" || comp_style.float != "none" {
                    x = prev_computed_flow.x + prev_computed_flow.width;
                } else {
                    x = prev_computed_flow.continue_x;
                }
                // x = prev_computed_flow.x + prev_computed_flow.width;
                context.adjacent_margin_bottom = 0.0;
            } else {
                // Vertical layout
                y = reserved_block_y;
                // Collapse margins
                y += f32::max(prev_style.margin.bottom.get(), comp_style.margin.top);

                // context.adjacent_margin_bottom = prev_computed_flow.adjacent_margin_bottom;
            }

            x += comp_style.margin.right;
        } else {
            y += f32::max(0.0, comp_style.margin.top - previous_margin_bottom);
        }

        if context.layout_x_start.is_none()
            && is_horizontal_layout(style)
            && comp_style.display == "inline"
        {
            context.layout_x_start = Some(x);
        }

        last_element = Some(i);

        let element = &mut tree[i];
        element.computed_style = Some(comp_style);

        let comp_style = element.computed_style.as_ref().unwrap();
        let style = element.inherited_style.as_ref().unwrap();

        x += comp_style.margin.left;

        let mut adjacent_margin_bottom = 0.0;

        width = comp_style.padding.left + comp_style.padding.right;
        height = comp_style.padding.top + comp_style.padding.bottom;

        let mut continue_x = x + width + comp_style.margin.right;
        let mut continue_y = y + height + comp_style.margin.bottom;

        if element.children.len() > 0 && element.tag_name != "SCRIPT" && element.tag_name != "STYLE"
        {
            context.x = x + comp_style.padding.left;
            context.y = y + comp_style.padding.top;

            if comp_style.position == "absolute"
                || comp_style.position == "fixed"
                || comp_style.position == "relative"
            {
                context.rel_x = context.x;
                context.rel_y = context.y;
            }

            if comp_style.display != "inline" {
                context.layout_x_start = None;
            }

            reflow(
                &mut element.children,
                text_measurer,
                Some(context.clone()),
                viewport,
            );

            for el in &element.children {
                let el_computed_flow = el.computed_flow.as_ref();
                let el_computed_style = el.computed_style.as_ref();
                if el_computed_flow.is_none() || el_computed_style.is_none() {
                    continue;
                }
                let el_computed_flow = el_computed_flow.unwrap();
                let el_computed_style = el_computed_style.unwrap();
                if el_computed_style.position == "absolute" || el_computed_style.position == "fixed"
                {
                    continue;
                }
                width = f32::max(
                    el_computed_flow.width
                        + (el_computed_flow.x - x)
                        + el_computed_style.margin.right
                        + comp_style.padding.right,
                    width,
                );

                height = f32::max(
                    el_computed_flow.height
                        + (el_computed_flow.y - y)
                        + el_computed_style.margin.bottom
                        + comp_style.padding.bottom,
                    height,
                );

                continue_x = el_computed_flow.continue_x;
                continue_y = el_computed_flow.continue_y;
            }

            adjacent_margin_bottom = comp_style.margin.bottom;

            for el in &element.children {
                let computed_flow = el.computed_flow.as_ref();
                if computed_flow.is_none() {
                    continue;
                }
                let computed_flow = computed_flow.unwrap();
                adjacent_margin_bottom =
                    f32::max(adjacent_margin_bottom, computed_flow.adjacent_margin_bottom);
                break;
            }
        }

        match element.node_type {
            NodeType::Text => {
                element.node_value = element
                    .node_value
                    .replace("&nbsp;", " ")
                    .replace("&gt;", ">")
                    .replace("&lt;", "<");

                let lines = wrap_text(
                    element.node_value.clone(),
                    max_width,
                    text_measurer,
                    comp_style.font_size,
                    style.font.get_path(),
                    x,
                    y,
                    context.layout_x_start.unwrap_or(x),
                );

                if lines.len() > 0 {
                    let last = lines.last().unwrap();
                    continue_x = last.x + last.width;
                    continue_y = last.y;

                    if lines.len() == 1 {
                        width = last.width;
                    } else {
                        let first = lines.first().unwrap();
                        width = first.width;
                    }
                    for line in &lines {
                        height += line.height;
                    }
                }

                element.lines = lines;
            }
            _ => {}
        }

        match element.node_type {
            NodeType::Comment => {}
            _ => {
                if comp_style.position != "absolute" && comp_style.position != "fixed" {
                    reserved_block_y = f32::max(height + y, reserved_block_y);
                }
            }
        }

        element.computed_flow = Some(ComputedFlow {
            x: x,
            y: y,
            width: width,
            height: height,
            continue_x: continue_x,
            continue_y: continue_y,
            adjacent_margin_bottom: adjacent_margin_bottom,
            hover_rect: if element.is_hovered && element.computed_style.is_some() {
                element.computed_flow.as_ref().unwrap().hover_rect.clone()
            } else {
                Rect {
                    x: x,
                    y: y,
                    width: width,
                    height: height,
                }
            },
        });
    }
}

pub fn wrap_text(
    text: String,
    max_width: f32,
    text_measurer: &mut dyn TextMeasurer,
    font_size: f32,
    font_path: String,
    x: f32,
    y: f32,
    layout_x_start: f32,
) -> Vec<TextLine> {
    let words = text.split(" ").collect::<Vec<&str>>();
    let mut line = "".to_string();
    let mut lines: Vec<TextLine> = vec![];

    let space_size = text_measurer.measure(" ", font_size, &font_path);

    let mut lx = x;
    let mut ly = y;
    let mut lw = 0.0;

    for word in words {
        let size = text_measurer.measure(&line, font_size, &font_path);

        let word_space = format!(" {}", word);
        let word_size = text_measurer.measure(&word_space, font_size, &font_path);

        lw = size.0;

        if size.0 + lx + word_size.0 > max_width {
            lines.push(TextLine {
                text: line.clone(),
                x: lx,
                y: ly,
                width: lw,
                height: size.1,
            });
            ly += space_size.1;
            lx = layout_x_start;
            line = "".to_string();
            lw = 0.0;
        }

        line += word_space.as_str();
        lw += word_size.0;
    }

    lines.push(TextLine {
        text: line.clone(),
        x: lx,
        y: ly,
        width: lw,
        height: space_size.1,
    });

    return lines;
}

pub fn get_render_array(tree: &mut Vec<DomElement>, viewport: &Rect) -> Vec<RenderItem> {
    let mut array: Vec<RenderItem> = vec![];

    for i in 0..tree.len() {
        let element = &mut tree[i];
        let computed_flow = element.computed_flow.as_ref();
        if computed_flow.is_none() {
            continue;
        }
        let computed_flow = computed_flow.unwrap();
        let rect = Rect {
            x: computed_flow.x,
            y: computed_flow.y,
            width: computed_flow.width,
            height: computed_flow.height,
        };

        let is_in_viewport = is_in_viewport(viewport, &rect);
        if element.children.len() > 0
            && element.tag_name != "SCRIPT"
            && element.tag_name != "STYLE"
            && is_in_viewport
        {
            let children_render_items = get_render_array(&mut element.children, viewport);
            array.extend(children_render_items);
        }

        let element = &tree[i];
        let computed_flow = element.computed_flow.as_ref().unwrap();
        let computed_style = element.computed_style.as_ref();
        if computed_style.is_none() {
            continue;
        }
        let computed_style = computed_style.unwrap();

        let has_something_to_render =
            element.node_value != "" || computed_style.background_color != (0.0, 0.0, 0.0, 0.0);
        // The element has nothing to render
        if !has_something_to_render || !is_in_viewport || computed_style.display == "none" {
            continue;
        }

        let style = element.inherited_style.as_ref().unwrap();

        match element.node_type {
            NodeType::Comment => {}
            _ => {
                let item = RenderItem {
                    x: computed_flow.x,
                    y: computed_flow.y,
                    width: computed_flow.width,
                    height: computed_flow.height,
                    background_color: computed_style.background_color,
                    text_lines: element.lines.clone(),
                    font_size: computed_style.font_size,
                    font_path: style.font.get_path(),
                    color: computed_style.color,
                    underline: element.node_value != ""
                        && computed_style.text_decoration == "underline",
                };
                array.insert(0, item);
            }
        }
    }

    return array;
}
