use crate::colors::*;
use crate::css::*;
use crate::html::*;
use crate::styles::*;
use crate::utils::*;
use std::collections::HashMap;

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
    pub render: bool,
    pub background_color: ColorTupleA,
    pub margin_bottom: f64,
    pub adjacent_margin_bottom: f64,
    pub color: ColorTupleA,
    pub underline: bool,
    pub margin_right: f64,
    pub hover_rect: Rect,
}

impl RenderItem {
    pub fn new() -> RenderItem {
        RenderItem {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            font_size: 16.0,
            margin_bottom: 0.0,
            render: false,
            color: (0.0, 0.0, 0.0, 1.0),
            background_color: (0.0, 0.0, 0.0, 0.0),
            font_path: S(""),
            text: S(""),
            underline: false,
            adjacent_margin_bottom: 0.0,
            margin_right: 0.0,
            hover_rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
        }
    }
}

pub fn hit_test_element(element: &DomElement, x: f64, y: f64) -> bool {
    return x >= element.render_item.hover_rect.x
        && x <= element.render_item.hover_rect.x + element.render_item.hover_rect.width
        && y >= element.render_item.hover_rect.y
        && y <= element.render_item.hover_rect.y + element.render_item.hover_rect.height;
}

pub fn should_rerender(
    mouse_x: f64,
    mouse_y: f64,
    tree: &mut Vec<DomElement>,
    style: Vec<StyleRule>,
) -> bool {
    let mut dirty = false;

    let elements_len = tree.len();

    for i in 0..elements_len {
        let mut hovered = false;
        let mut element = &mut tree[i];

        for style_rule in style.clone() {
            if element_matches_hover_selector(&element, &style_rule.selector) {
                hovered = hit_test_element(&element, mouse_x, mouse_y);
            }
        }

        if hovered != element.is_hovered {
            element.is_hovered = hovered;
            dirty = true;
        }

        if element.children.len() > 0 {
            if should_rerender(mouse_x, mouse_y, &mut element.children, style.clone()) {
                return true;
            }
        }
    }

    return dirty;
}

pub fn element_matches_hover_selector(element: &DomElement, selector: &str) -> bool {
    selector.to_uppercase() == element.tag_name.clone() + ":HOVER"
}

pub fn element_matches_selector(element: &DomElement, selector: &str) -> bool {
    selector.to_uppercase() == element.tag_name
        || selector == "*"
        || (element_matches_hover_selector(&element, selector) && element.is_hovered)
}

pub fn get_render_array(
    tree: &mut Vec<DomElement>,
    style: Vec<StyleRule>,
    measure_text: &dyn Fn(String, f64, String) -> (f64, f64),
    inherit_declarations: Option<HashMap<String, CssValue>>,
) -> Vec<RenderItem> {
    let mut array: Vec<RenderItem> = vec![];

    let elements_len = tree.len();

    let inherit_declarations = inherit_declarations.unwrap_or(HashMap::new());

    let x_base = (*inherit_declarations
        .get("x")
        .unwrap_or(&CssValue::Number(0.0)))
    .to_number();

    let y_base = (*inherit_declarations
        .get("y")
        .unwrap_or(&CssValue::Number(0.0)))
    .to_number();

    let mut reserved_block_y = y_base.clone();

    for i in 0..elements_len {
        let mut x = x_base.clone();
        let mut y = y_base.clone();

        let mut declarations: HashMap<String, String> = HashMap::new();
        let mut new_inherit_declarations = inherit_declarations.clone();

        {
            let mut element = &mut tree[i];
            for style_rule in style.clone() {
                if element_matches_selector(&element, &style_rule.selector) {
                    for declaration in style_rule.declarations.clone() {
                        declarations.insert(declaration.0, declaration.1);
                    }
                }
            }

            element.style = declarations.clone();
        }

        let mut width = 0.0;
        let mut height = 0.0;

        let get_inherit_value = |k: &str, d: CssValue| {
            get_inheritable_declaration_value(&declarations, &inherit_declarations, k, d)
        };

        let display = get_declaration_value(&declarations, "display", "inline-block");

        if display == "none" {
            continue;
        }

        let background_color_css = get_declaration_value(&declarations, "background-color", "none");

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
        let mut font_size: f64 = base_font_size.clone();
        let mut font_path: String = "Times New Roman 400.ttf".to_string();

        new_inherit_declarations.insert(S("font-family"), font_family_css);
        new_inherit_declarations.insert(S("font-weight"), font_weight_css);
        new_inherit_declarations.insert(S("font-style"), font_style_css);
        new_inherit_declarations.insert(S("text-decoration"), text_decoration_css);
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
            let font_size_str = declarations.get("font-size");

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
                &get_declaration_value(&declarations, k, "0"),
                base_font_size,
            )
        };

        let margin_top = get_numeric_declaration_value("margin-top");
        let margin_bottom = get_numeric_declaration_value("margin-bottom");
        let margin_left = get_numeric_declaration_value("margin-left");
        let margin_right = get_numeric_declaration_value("margin-right");

        let previous_margin_bottom =
            get_inherit_value("previous-margin-bottom", CssValue::Number(0.0)).to_number();

        if i > 0 {
            let previous_element = &tree[i - 1];

            if display == "inline-block" {
                y = previous_element.render_item.y;
            }

            let should_continue_horizontal_layout = display == "inline-block"
                && previous_element
                    .style
                    .get("display")
                    .unwrap_or(&"inline-block".to_string())
                    .to_string()
                    == "inline-block";

            if display == "inline-block" && should_continue_horizontal_layout {
                // Horizontal layout
                x = previous_element.render_item.x + previous_element.render_item.width;
                new_inherit_declarations.remove("previous-margin-bottom");
            } else {
                // Vertical layout
                y = reserved_block_y;
                y += f64::max(previous_element.render_item.margin_bottom, margin_top);

                new_inherit_declarations.insert(
                    S("previous-margin-bottom"),
                    CssValue::Number(previous_element.render_item.adjacent_margin_bottom),
                );
            }

            x += previous_element.render_item.margin_right;
        } else {
            y += f64::max(0.0, margin_top - previous_margin_bottom);
        }

        x += margin_left;

        let element = &mut tree[i];

        let mut adjacent_margin_bottom = 0.0;

        if element.children.len() > 0 && element.tag_name != "SCRIPT" && element.tag_name != "STYLE"
        {
            new_inherit_declarations.insert(S("x"), CssValue::Number(x.clone()));
            new_inherit_declarations.insert(S("y"), CssValue::Number(y.clone()));

            let children_render_items = get_render_array(
                &mut element.children,
                style.clone(),
                measure_text,
                Some(new_inherit_declarations),
            );
            array = [array.clone(), children_render_items.clone()].concat();

            for item in &children_render_items {
                width = f64::max(item.width + (item.x - x) + item.margin_right, width);
                height = f64::max(item.height + (item.y - y) + item.margin_bottom, height);
            }

            adjacent_margin_bottom = margin_bottom;

            if children_render_items.first().is_some() {
                adjacent_margin_bottom = f64::max(
                    adjacent_margin_bottom,
                    children_render_items
                        .first()
                        .unwrap()
                        .adjacent_margin_bottom,
                );
            }
        }

        match element.node_type {
            NodeType::Text => {
                element.node_value = element
                    .node_value
                    .replace("&nbsp;", " ")
                    .replace("&gt;", ">")
                    .replace("&lt;", "<");
                let size =
                    measure_text(element.node_value.clone(), font_size, font_path.to_string());
                width = size.0;
                height = size.1;
            }
            _ => {}
        }

        match element.node_type {
            NodeType::Comment => {}
            _ => {
                let item = RenderItem {
                    x: x,
                    y: y,
                    width: width,
                    height: height,
                    background_color: background_color,
                    text: element.node_value.clone(),
                    font_size: font_size,
                    font_path: font_path.to_string(),
                    margin_bottom: margin_bottom,
                    render: (element.node_type == NodeType::Text && element.node_value != "")
                        || background_color_css != "none",
                    color: color,
                    underline: element.node_value != "" && text_decoration == "underline",
                    adjacent_margin_bottom: adjacent_margin_bottom,
                    margin_right: margin_right,
                    hover_rect: if element.is_hovered {
                        element.render_item.hover_rect.clone()
                    } else {
                        Rect {
                            x: x,
                            y: y,
                            width: width,
                            height: height,
                        }
                    },
                };
                element.render_item = item.clone();
                array = [vec![item], array.clone()].concat();
                reserved_block_y = f64::max(height + y, reserved_block_y);
            }
        }
    }

    return array;
}
