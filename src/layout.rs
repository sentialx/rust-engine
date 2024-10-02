use crate::colors::*;
use crate::css::*;
use crate::css_value::CssValue;
use crate::html::*;
use crate::render_frame::TextMeasurer;
use crate::styles::*;
use crate::utils::*;
use std::cell::Ref;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
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
    pub element: Option<Rc<RefCell<DomElement>>>,
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
            element: None,
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
    tree: &Vec<Rc<RefCell<DomElement>>>,
    style: &Vec<StyleRule>,
) -> bool {
    let mut dirty = false;

    let elements_len = tree.len();

    for i in 0..elements_len {
        let mut hovered = false;
        let element = &tree[i].borrow();

        if element.computed_style.is_none() {
            continue;
        }

        // if element.computed_style.as_ref().unwrap().hoverable {
        //     hovered = hit_test_element(&element, mouse_x, mouse_y);
        // }

        if hovered != element.is_hovered {
            // element.is_hovered = hovered;
            dirty = true;
        }

        if element.children.len() > 0 {
            if should_rerender(mouse_x, mouse_y, &element.children, style) {
                return true;
            }
        }
    }

    return dirty;
}

pub fn element_matches_hover_selector(element: &DomElement, selector: &str) -> bool {
    selector.to_uppercase() == element.tag_name.clone() + ":HOVER"
}

pub fn get_element_at(
    render_items: &Vec<RenderItem>,
    x: f32,
    y: f32,
) -> Option<&Rc<RefCell<DomElement>>> {
    for i in (0..render_items.len()).rev() {
        let item = &render_items[i];

        let element = match &item.element {
            Some(e) => e,
            None => continue,
        };
        let element = element.borrow();

        if element.node_type == NodeType::Text {
            continue;
        }

        let computed_flow = element.computed_flow.as_ref().unwrap();
        let rect = Rect {
            x: computed_flow.x,
            y: computed_flow.y,
            width: computed_flow.width,
            height: computed_flow.height,
        };

        if rect_contains(&rect, x, y) {
            return Some(item.element.as_ref().unwrap());
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
                    "~" => attr_value
                        .split_whitespace()
                        .any(|part| part == value.as_deref().unwrap()),
                    "|" => {
                        attr_value == value.as_deref().unwrap()
                            || attr_value.starts_with(&(value.clone().unwrap() + "-"))
                    }
                    "^" => attr_value.starts_with(value.as_deref().unwrap()),
                    "$" => attr_value.ends_with(value.as_deref().unwrap()),
                    "*" => attr_value.contains(value.as_deref().unwrap()),
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

#[derive(Clone, Debug)]
pub struct CssVariablesContext {
    pub variables: HashMap<String, CssValue>,
}

impl CssVariablesContext {
    pub fn new() -> CssVariablesContext {
        CssVariablesContext {
            variables: HashMap::new(),
        }
    }
}

pub fn compute_styles(
    tree: &mut Vec<Rc<RefCell<DomElement>>>,
    style: &Vec<StyleRule>,
    parents: &mut Vec<*mut DomElement>,
    var_ctx: Option<CssVariablesContext>,
) {
    let is_root = parents.len() == 0;

    let mut var_ctx = var_ctx;

    if var_ctx.is_none() {
        var_ctx = Some(CssVariablesContext::new());

        for rule in style {
            if rule.selector.to_string() == ":root" {
                for decl in &rule.declarations {
                    if decl.key.starts_with("--") {
                        var_ctx.as_mut().unwrap().variables.insert(
                            decl.key.clone(),
                            decl.value.clone(),
                        );
                    }
                }
            }
        }
    }

    for i in 0..tree.len() {
        let mut element = tree[i].borrow_mut();
        let mut hoverable = false;

        for style_rule in style {
            if element_matches_selector(&element, &style_rule.selector, parents) {
                element.style.insert_declarations(&style_rule.declarations, var_ctx.as_ref().unwrap());
                element.matched_styles.push(style_rule.clone());
            }
        }

        if element.children.len() > 0 && element.tag_name != "SCRIPT" && element.tag_name != "STYLE"
        {
            parents.push(tree[i].as_ptr());
            compute_styles(&mut element.children, style, parents, var_ctx.clone());
            parents.pop();
        }
    }
}

pub fn propagate_styles(tree: &mut Vec<Rc<RefCell<DomElement>>>, parent_style: Option<&Style>) {
    let parent_style = match parent_style {
        Some(s) => s,
        None => &Style::new(),
    };

    for i in 0..tree.len() {
        let mut element = tree[i].borrow_mut();
        let inherited_styles = element.style.create_inherited(&parent_style);

        if element.children.len() > 0 && element.tag_name != "SCRIPT" && element.tag_name != "STYLE"
        {
            propagate_styles(&mut element.children, Some(&inherited_styles));
        }

        element.inherited_style = Some(inherited_styles);
    }
}

#[derive(Clone, Debug)]
pub struct ReflowContext {
    pub x: f32,
    pub y: f32,
    pub rel_x: f32,
    pub rel_y: f32,
    pub font_size: f32,
    pub parent_width: f32,
    pub parent_height: f32,
    pub parent_max_width: f32,
    pub layout_x_start: Option<f32>,
    pub adjacent_margin_bottom: f32,
}

fn is_horizontal_layout(computed_style: &Style) -> bool {
    return computed_style.display.get() == "inline-block"
        || computed_style.display.get() == "inline"
        || computed_style.float.get() != "none" || computed_style.display.get() == "inline-flex";
}

fn uses_absolute_positioning(computed_style: &ComputedStyle) -> bool {
    return computed_style.position == "absolute"
        || computed_style.position == "fixed" || computed_style.position == "sticky";
}

pub fn reflow(
    tree: &mut Vec<Rc<RefCell<DomElement>>>,
    text_measurer: &mut dyn TextMeasurer,
    context: Option<&mut ReflowContext>,
    viewport: &Rect,
) {
    let mut default_context = ReflowContext {
        x: 0.0,
        y: 0.0,
        rel_x: 0.0,
        rel_y: 0.0,
        font_size: 16.0,
        parent_width: viewport.width,
        parent_height: viewport.height,
        parent_max_width: viewport.width,
        layout_x_start: None,
        adjacent_margin_bottom: 0.0,
    };

    let sibling_context = context.unwrap_or(&mut default_context);

    let x_base = sibling_context.x;
    let y_base = sibling_context.y;

    let mut reserved_block_y = y_base;

    let mut last_element: Option<usize> = None;

    for i in 0..tree.len() {
        let mut x = x_base;
        let mut y = y_base;

        let mut width = 0.0;
        let mut height = 0.0;

        let mut max_width = sibling_context.parent_max_width;

        // let parent_style = unsafe {
        //     match tree[i].parent_node.as_ref() {
        //         Some(p) => p.inherited_style.as_ref(),
        //         None => None,
        //     }
        // };
        // let tag_name = tree[i].tag_name.clone();
        {
            let mut element = tree[i].borrow_mut();
            let style = element.inherited_style.as_mut().unwrap();

            if style.display.get() == "none" {
                continue;
            }

            let font_scalar_ctx = ScalarEvaluationContext::from_parent(
                sibling_context.font_size,
                sibling_context.font_size,
            );
            let parent_width_scalar_ctx = ScalarEvaluationContext::from_parent(
                sibling_context.font_size,
                sibling_context.parent_width,
            );
            let parent_height_scalar_ctx = ScalarEvaluationContext::from_parent(
                sibling_context.font_size,
                sibling_context.parent_height,
            );

            style.margin.evaluate(&font_scalar_ctx);
            style.padding.evaluate(&font_scalar_ctx);
            style.font_size.evaluate(&font_scalar_ctx);
            style.inset.evaluate(&font_scalar_ctx);
            style.width.evaluate(&parent_width_scalar_ctx);
            style.height.evaluate(&parent_height_scalar_ctx);

            if style.width.has_numeric_value() {
                max_width = style.width.get();
                width = style.width.get();
            }

            if style.height.has_numeric_value() {
                height = style.height.get();
            }
        }

        {
            let mut element = tree[i].borrow_mut();
            let comp_style = element
                .inherited_style
                .as_ref()
                .unwrap()
                .to_computed_style();
            element.computed_style = Some(comp_style);
        }
        {
            let element = tree[i].borrow();
            let comp_style = element.computed_style.as_ref().unwrap();
            let style = element.inherited_style.as_ref().unwrap();

            if uses_absolute_positioning(comp_style) {
                if style.inset.top.has_numeric_value() {
                    y = style.inset.top.get() + sibling_context.rel_y;
                }
                if style.inset.left.has_numeric_value() {
                    x = style.inset.left.get() + sibling_context.rel_x;
                }
            }

            let previous_margin_bottom = sibling_context.adjacent_margin_bottom;

            if last_element.is_some()
                && !uses_absolute_positioning(comp_style)
            {
                let previous_element = &tree[last_element.unwrap()].borrow();
                let prev_style = previous_element.inherited_style.as_ref().unwrap();
                let prev_computed_flow = previous_element.computed_flow.as_ref().unwrap();

                if is_horizontal_layout(prev_style) {
                    if comp_style.display == "inline-block" || comp_style.float != "none" || comp_style.display == "inline-flex" {
                        y = prev_computed_flow.y;
                    } else {
                        y = prev_computed_flow.continue_y;
                    }
                }

                let should_continue_horizontal_layout =
                    is_horizontal_layout(style) && is_horizontal_layout(prev_style);

                if is_horizontal_layout(style) && should_continue_horizontal_layout {
                    // Horizontal layout
                    if comp_style.display == "inline-block" || comp_style.float != "none" || comp_style.display == "inline-flex" {
                        x = prev_computed_flow.x
                            + prev_computed_flow.width
                            + prev_style.margin.right.get();
                    } else {
                        x = prev_computed_flow.continue_x;
                    }
                    // x = prev_computed_flow.x + prev_computed_flow.width;
                    sibling_context.adjacent_margin_bottom = 0.0;
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

            if sibling_context.layout_x_start.is_none()
                && is_horizontal_layout(style)
                && comp_style.display == "inline"
            {
                sibling_context.layout_x_start = Some(x);
            }
        }

        if !uses_absolute_positioning(tree[i].borrow().computed_style.as_ref().unwrap()) {
            last_element = Some(i);
        }

        let mut continue_x = 0.0;
        let mut continue_y = 0.0;

        let mut adjacent_margin_bottom = 0.0;

        let has_children = {
            let element = tree[i].borrow();
            element.children.len() > 0
                && element.tag_name != "SCRIPT"
                && element.tag_name != "STYLE"
        };

        let mut parent_context: Option<ReflowContext> = None;

        if has_children {
            let element = tree[i].borrow();
            let style = element.inherited_style.as_ref().unwrap();

            parent_context = Some(sibling_context.clone());
            let parent_context = parent_context.as_mut().unwrap();

            if style.width.has_numeric_value() {
                parent_context.parent_width = width;
            }
            if style.height.has_numeric_value() {
                parent_context.parent_height = height;
            }
        }

        {
            let element = tree[i].borrow();
            let comp_style = element.computed_style.as_ref().unwrap();

            x += comp_style.margin.left;

            width += comp_style.padding.left + comp_style.padding.right;
            height += comp_style.padding.top + comp_style.padding.bottom;

            continue_x = x + width + comp_style.margin.right;
            continue_y = y + height + comp_style.margin.bottom;
        }

        if has_children {
            let parent_context = parent_context.as_mut().unwrap();

            let element = tree[i].borrow();
            let comp_style = element.computed_style.as_ref().unwrap();

            parent_context.x = x + comp_style.padding.left;
            parent_context.y = y + comp_style.padding.top;
            
            parent_context.parent_max_width = max_width;
            
            if uses_absolute_positioning(comp_style)
            || comp_style.position == "relative"
            {
                parent_context.rel_x = parent_context.x;
                parent_context.rel_y = parent_context.y;
            }

            if comp_style.display != "inline" {
                parent_context.layout_x_start = None;
            }
        }

        if has_children {
            let mut parent_context = parent_context.unwrap();
            let mut element = tree[i].borrow_mut();

            reflow(
                &mut element.children,
                text_measurer,
                Some(&mut parent_context),
                viewport,
            );

            let comp_style = element.computed_style.as_ref().unwrap();
            let style = element.inherited_style.as_ref().unwrap();

            for el in &element.children {
                let el = el.borrow();
                let el_computed_flow = el.computed_flow.as_ref();
                let el_computed_style = el.computed_style.as_ref();
                if el_computed_flow.is_none() || el_computed_style.is_none() {
                    continue;
                }
                let el_computed_flow = el_computed_flow.unwrap();
                let el_computed_style = el_computed_style.unwrap();
                if uses_absolute_positioning(el_computed_style)
                {
                    continue;
                }
                if !style.width.has_numeric_value() {
                    width = f32::max(
                        el_computed_flow.width
                            + (el_computed_flow.x - x)
                            + el_computed_style.margin.right
                            + comp_style.padding.right,
                        width,
                    );
                }

                if !style.height.has_numeric_value() {
                    height = f32::max(
                        el_computed_flow.height
                            + (el_computed_flow.y - y)
                            + el_computed_style.margin.bottom
                            + comp_style.padding.bottom,
                        height,
                    );
                }

                continue_x = el_computed_flow.continue_x;
                continue_y = el_computed_flow.continue_y;
            }

            adjacent_margin_bottom = comp_style.margin.bottom;

            for el in &element.children {
                let el = el.borrow();
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

        let mut element = tree[i].borrow_mut();

        match element.node_type {
            NodeType::Text => {
                element.node_value = element
                    .node_value
                    .replace("&nbsp;", " ")
                    .replace("&gt;", ">")
                    .replace("&lt;", "<");

                let comp_style = element.computed_style.as_ref().unwrap();
                let style = element.inherited_style.as_ref().unwrap();

                let lines = if style.white_space.get() != "nowrap" {
                    wrap_text(
                        element.node_value.clone(),
                        max_width,
                        text_measurer,
                        comp_style.font_size,
                        style.font.get_path(),
                        x,
                        y,
                        sibling_context.layout_x_start.unwrap_or(x),
                    )
                } else {
                    let size = text_measurer.measure(
                        &element.node_value,
                        comp_style.font_size,
                        &style.font.get_path(),
                    );
                    vec![TextLine {
                        text: element.node_value.clone(),
                        x: x,
                        y: y,
                        width: size.0,
                        height: size.1,
                    }]
                };

                if lines.len() > 0 {
                    let last = lines.last().unwrap();
                    continue_x = last.x + last.width;
                    continue_y = last.y;

                    if !style.width.has_numeric_value() {
                        if lines.len() == 1 {
                            width = last.width;
                        } else {
                            let first = lines.first().unwrap();
                            width = first.width;
                        }
                    }
                    if !style.height.has_numeric_value() {
                        for line in &lines {
                            height += line.height;
                        }
                        height += 8.0;
                    }
                }

                element.lines = lines;
            }
            _ => {}
        }

        match element.node_type {
            NodeType::Comment => {}
            _ => {
                let comp_style = element.computed_style.as_ref().unwrap();
                if !uses_absolute_positioning(comp_style) {
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

        if size.0 + (lx - layout_x_start) + word_size.0 > max_width {
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

pub fn get_render_array(
    tree: &mut Vec<Rc<RefCell<DomElement>>>,
    viewport: &Rect,
) -> Vec<RenderItem> {
    let mut array: Vec<RenderItem> = vec![];

    for i in 0..tree.len() {
        let element = &mut tree[i].borrow_mut();
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

        let computed_style = element.computed_style.as_ref();
        if computed_style.is_none() {
            continue;
        }
        let computed_style = computed_style.unwrap();
        if computed_style.display == "none" || computed_style.visibility == "hidden" {
            continue;
        }

        let is_in_viewport = is_in_viewport(viewport, &rect);
        if element.children.len() > 0 && element.tag_name != "SCRIPT" && element.tag_name != "STYLE"
        {
            let children_render_items = get_render_array(&mut element.children, viewport);
            array.extend(children_render_items);
        }

        // let element = &tree[i];
        let computed_flow = element.computed_flow.as_ref().unwrap();
        let computed_style = element.computed_style.as_ref().unwrap();

        let has_something_to_render =
            element.node_value != "" || computed_style.background_color != (0.0, 0.0, 0.0, 0.0);
        // The element has nothing to render
        if !is_in_viewport {
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
                    underline: computed_style.text_decoration == "underline",
                    element: Some(tree[i].clone()),
                };
                array.insert(0, item);
            }
        }
    }

    return array;
}
