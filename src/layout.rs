use crate::colors::*;
use crate::css::*;
use crate::html::*;
use crate::styles::*;
use crate::utils::*;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug)]
pub struct RenderItem {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub text: String,
    pub font_size: f64,
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
            text: S(""),
            underline: false,
        }
    }
}

pub fn hit_test_element(element: &DomElement, x: f64, y: f64) -> bool {
    return rect_contains(&element.computed_flow.as_ref().unwrap().hover_rect, x, y);
}

pub fn rect_contains(rect: &Rect, x: f64, y: f64) -> bool {
    return x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height;
}

pub fn is_in_viewport(rect: &Rect, viewport: &Rect) -> bool {
    return rect.x + rect.width >= viewport.x
        && rect.x <= viewport.x + viewport.width
        && rect.y + rect.height >= viewport.y
        && rect.y <= viewport.y + viewport.height;
}

pub fn should_rerender(
    mouse_x: f64,
    mouse_y: f64,
    tree: &mut Vec<DomElement>,
    style: &Vec<StyleRule>,
) -> bool {
    let mut dirty = false;

    let elements_len = tree.len();

    for i in 0..elements_len {
        let mut hovered = false;
        let mut element = &mut tree[i];

        for style_rule in style {
            if element_matches_hover_selector(&element, &style_rule.selector) {
                hovered = hit_test_element(&element, mouse_x, mouse_y);
            }
        }

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

pub fn element_matches_single_selector(element: &DomElement, selector: &str) -> bool {
    if selector == "*" {
        return true;
    }

    if selector.to_uppercase() == element.tag_name {
        return true;
    }

    if selector.starts_with('#') && element.attributes.contains_key("id") {
        return element.attributes.get("id").unwrap() == &selector[1..];
    }

    if selector.starts_with(".") && element.attributes.contains_key("class") {
        let selector_classes = selector.split(".").collect::<Vec<&str>>()[1..].to_vec();
        let classes = element.attributes.get("class").unwrap().split(" ").collect::<Vec<&str>>();
        return selector_classes.iter().all(|c| classes.contains(c));
    }

    return false;
}

pub fn element_matches_selector(element: &DomElement, selector: &str) -> bool {
    let selectors = selector.split(",").collect::<Vec<&str>>();

    for s in selectors {
        if element_matches_single_selector(element, s.trim()) {
            return true;
        }
    }

    return false;
}

pub fn get_element_at(
    tree: &Vec<DomElement>,
    x: f64,
    y: f64,
) -> Option<&DomElement> {
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
        } else if rect_contains(&rect, x, y) {
            return Some(element);
        }
    }

    return None;
}

pub fn compute_styles(
    tree: &mut Vec<DomElement>,
    style: &Vec<StyleRule>,
    inherit_declarations: Option<HashMap<String, CssValue>>,
) {
    let elements_len = tree.len();

    let inherit_declarations = inherit_declarations.unwrap_or(HashMap::new());

    for i in 0..elements_len {
        let mut new_inherit_declarations = inherit_declarations.clone();

        let mut element = &mut tree[i];
        element.style = HashMap::new();
        {
            for style_rule in style {
                if element_matches_selector(&element, &style_rule.selector) {
                    for declaration in &style_rule.declarations {
                        element
                            .style
                            .insert(declaration.0.clone(), declaration.1.clone());
                    }
                }
            }
        }

        let get_inherit_value = |k: &str, d: CssValue| {
            get_inheritable_declaration_value(&element.style, &inherit_declarations, k, d)
        };

        let display = get_declaration_value(&element.style, "display", "inline-block");

        let background_color_css =
            get_declaration_value(&element.style, "background-color", "none");

        let font_weight_css = get_inherit_value("font-weight", css_string("normal"));
        let font_weight = font_weight_css.to_string();

        let font_style_css = get_inherit_value("font-style", css_string("normal"));
        let font_style = font_style_css.to_string();

        let font_family_css = get_inherit_value("font-family", css_string("Times New Roman"));
        let font_family = font_family_css.to_string();

        let text_decoration_css = get_inherit_value("text-decoration", css_string("none"));
        let text_decoration = text_decoration_css.to_string();

        let color_css = get_inherit_value("color", css_string("#000"));

        let color = match color_css {
            CssValue::Color(c) => c,
            CssValue::String(c) => match parse_css_color(&c) {
                Ok(c) => c,
                Err(e) => {
                    println!("{}", e);
                    (0.0, 0.0, 0.0, 1.0)
                }
            },
            CssValue::Number(_) => (0.0, 0.0, 0.0, 1.0),
        };

        let background_color = if background_color_css == "none" {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            match parse_css_color(&background_color_css) {
                Ok(c) => c,
                Err(e) => {
                    println!("{}", e);
                    (0.0, 0.0, 0.0, 0.0)
                }
            }
        };

        let mut base_font_size = 16.0;
        let mut font_size: f64 = base_font_size;
        let mut font_path: String = "Times New Roman 400.ttf".to_string();

        new_inherit_declarations.insert(S("font-family"), font_family_css.clone());
        new_inherit_declarations.insert(S("font-weight"), font_weight_css.clone());
        new_inherit_declarations.insert(S("font-style"), font_style_css.clone());
        new_inherit_declarations.insert(S("text-decoration"), text_decoration_css.clone());
        new_inherit_declarations.insert(S("color"), CssValue::Color(color));

        if font_family.to_lowercase() == "times new roman" {
            if font_weight == "bold" {
                if font_style == "italic" {
                    font_path = "Times New Roman Italique 700.ttf".to_string();
                } else {
                    font_path = "Times New Roman 700.ttf".to_string();
                }
            } else {
                if font_style == "italic" {
                    font_path = "Times New Roman Italique 400.ttf".to_string();
                } else {
                    font_path = "Times New Roman 400.ttf".to_string();
                }
            }
        }

        // TODO: simplify
        {
            let font_size_inherit = inherit_declarations.get("font-size");
            let font_size_str = element.style.get("font-size");

            match font_size_str {
                Some(f) => {
                    match font_size_inherit {
                        Some(f_i) => {
                            base_font_size = (*f_i).to_number();
                        }
                        None => {}
                    }

                    font_size = parse_numeric_css_value(&f, base_font_size);
                }
                None => match font_size_inherit {
                    Some(f_i) => {
                        font_size = (*f_i).to_number();
                    }
                    None => {}
                },
            }

            new_inherit_declarations.insert("font-size".to_string(), CssValue::Number(font_size));
        }

        let get_numeric_declaration_value = |k: &str| -> f64 {
            parse_numeric_css_value(
                &get_declaration_value(&element.style, k, "0"),
                base_font_size,
            )
        };

        let margin_top = get_numeric_declaration_value("margin-top");
        let margin_bottom = get_numeric_declaration_value("margin-bottom");
        let margin_left = get_numeric_declaration_value("margin-left");
        let margin_right = get_numeric_declaration_value("margin-right");

        let padding_top = get_numeric_declaration_value("padding-top");
        let padding_bottom = get_numeric_declaration_value("padding-bottom");
        let padding_left = get_numeric_declaration_value("padding-left");
        let padding_right = get_numeric_declaration_value("padding-right");

        let element = &mut tree[i];

        if element.children.len() > 0 && element.tag_name != "SCRIPT" && element.tag_name != "STYLE" {
            compute_styles(&mut element.children, style, Some(new_inherit_declarations));
        }

        element.computed_style = Some(ComputedStyle {
            margin: Margin {
                top: margin_top,
                right: margin_right,
                bottom: margin_bottom,
                left: margin_left,
            },
            padding: Margin {
                top: padding_top,
                right: padding_right,
                bottom: padding_bottom,
                left: padding_left,
            },
            background_color: background_color,
            color: color,
            font_size: font_size,
            font_path: font_path.to_string(),
            text_decoration: text_decoration.to_string(),
            display: display.to_string(),
            float: get_declaration_value(&element.style, "float", "none"),
        });
    }
}

#[derive(Clone, Debug)]
pub struct ReflowContext {
    pub x: f64,
    pub y: f64,
    pub adjacent_margin_bottom: f64,
}

fn is_horizontal_layout(computed_style: &ComputedStyle) -> bool {
    return computed_style.display == "inline-block" || computed_style.display == "inline" || computed_style.float != "none";
}

pub fn reflow(
    tree: &mut Vec<DomElement>,
    measure_text: &dyn Fn(String, f64, String) -> (f64, f64),
    context: Option<ReflowContext>,
) {
    let elements_len = tree.len();

    let mut context = context.unwrap_or(ReflowContext {
        x: 0.0,
        y: 0.0,
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

        let computed_style = &tree[i].computed_style.as_ref().unwrap();

        if computed_style.display == "none" {
            continue;
        }

        let previous_margin_bottom = context.adjacent_margin_bottom;

        if last_element.is_some() {
            let previous_element = &tree[last_element.unwrap()];
            let prev_computed_style = previous_element.computed_style.as_ref().unwrap();
            let prev_computed_flow = previous_element.computed_flow.as_ref().unwrap();

            if is_horizontal_layout(prev_computed_style) {
                y = prev_computed_flow.y;
            }

            let should_continue_horizontal_layout = is_horizontal_layout(computed_style)
                && is_horizontal_layout(prev_computed_style);

            if is_horizontal_layout(computed_style) && should_continue_horizontal_layout {
                // Horizontal layout
                x = prev_computed_flow.x + prev_computed_flow.width;
                context.adjacent_margin_bottom = 0.0;
            } else {
                // Vertical layout
                y = reserved_block_y;
                y += f64::max(computed_style.margin.bottom, computed_style.margin.top);

                // context.adjacent_margin_bottom = prev_computed_flow.adjacent_margin_bottom;
            }

            x += computed_style.margin.right;
        } else {
            y += f64::max(0.0, computed_style.margin.top - previous_margin_bottom);
        }

        last_element = Some(i);

        let element = &mut tree[i];
        let computed_style = element.computed_style.as_ref().unwrap();

        x += computed_style.margin.left;

        let mut adjacent_margin_bottom = 0.0;

        width = computed_style.padding.left + computed_style.padding.right;
        height = computed_style.padding.top + computed_style.padding.bottom;

        if element.children.len() > 0
            && element.tag_name != "SCRIPT"
            && element.tag_name != "STYLE"
        {
            context.x = x + computed_style.padding.left;
            context.y = y + computed_style.padding.top;

            reflow(
                &mut element.children,
                measure_text,
                Some(context.clone()),
            );

            for el in &element.children {
                let el_computed_flow = el.computed_flow.as_ref();
                let el_computed_style = el.computed_style.as_ref();
                if el_computed_flow.is_none() || el_computed_style.is_none() {
                    continue;
                }
                let el_computed_flow = el_computed_flow.unwrap();
                let el_computed_style = el_computed_style.unwrap();
                width = f64::max(
                    el_computed_flow.width + (el_computed_flow.x - x) + el_computed_style.margin.right + computed_style.padding.right,
                    width,
                );
                
                height = f64::max(
                    el_computed_flow.height + (el_computed_flow.y - y) + el_computed_style.margin.bottom + computed_style.padding.bottom,
                    height,
                );
            }

            adjacent_margin_bottom = computed_style.margin.bottom;

            for el in &element.children {
                let computed_flow = el.computed_flow.as_ref();
                if computed_flow.is_none() {
                    continue;
                }
                let computed_flow = computed_flow.unwrap();
                adjacent_margin_bottom =
                    f64::max(adjacent_margin_bottom, computed_flow.adjacent_margin_bottom);
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
                let size = measure_text(
                    element.node_value.clone(),
                    computed_style.font_size,
                    computed_style.font_path.to_string(),
                );
                width = size.0;
                height = size.1;
            }
            _ => {}
        }

        match element.node_type {
            NodeType::Comment => {}
            _ => {
                reserved_block_y = f64::max(height + y, reserved_block_y);
            }
        }

        element.computed_flow = Some(ComputedFlow {
            x: x,
            y: y,
            width: width,
            height: height,
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

pub fn get_render_array(
    tree: &mut Vec<DomElement>,
    viewport: &Rect,
) -> Vec<RenderItem> {
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
            let children_render_items = get_render_array(
                &mut element.children,
                viewport,
            );
            array.extend(children_render_items);
        }

        let element = &tree[i];
        let computed_flow = element.computed_flow.as_ref().unwrap();
        let computed_style = element.computed_style.as_ref();
        if computed_style.is_none() {
            continue;
        }
        let computed_style = computed_style.unwrap();

        let has_something_to_render = element.node_value != ""
            || computed_style.background_color != (0.0, 0.0, 0.0, 0.0);
        // The element has nothing to render
        if !has_something_to_render
            || !is_in_viewport
            || computed_style.display == "none"
        {
            continue;
        }

        match element.node_type {
            NodeType::Comment => {}
            _ => {
                let item = RenderItem {
                    x: computed_flow.x,
                    y: computed_flow.y,
                    width: computed_flow.width,
                    height: computed_flow.height,
                    background_color: computed_style.background_color,
                    text: element.node_value.clone(),
                    font_size: computed_style.font_size,
                    font_path: computed_style.font_path.clone(),
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
