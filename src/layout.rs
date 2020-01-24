use crate::colors::*;
use crate::css::*;
use crate::html::*;
use crate::styles::*;
use crate::utils::*;
use std::collections::HashMap;

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
    }
  }
}

pub fn get_render_array(
  tree: Vec<DomElement>,
  style: Vec<StyleRule>,
  measure_text: &dyn Fn(String, f64, String) -> (f64, f64),
  inherit_declarations: Option<HashMap<String, CssValue>>,
) -> Vec<RenderItem> {
  let mut array: Vec<RenderItem> = vec![];

  let mut elements = tree.clone();
  let elements_len = elements.len();

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
      let mut element = &mut elements[i];
      for style_rule in style.clone() {
        if style_rule.selector.to_uppercase() == element.tag_name || style_rule.selector == "*" {
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

    if display == "none" {
      continue;
    }

    let margin_top = parse_numeric_css_value(
      &get_declaration_value(&declarations, "margin-top", "0"),
      base_font_size,
    );

    let margin_bottom = parse_numeric_css_value(
      &get_declaration_value(&declarations, "margin-bottom", "0"),
      base_font_size,
    );

    let mut previous_margin_bottom =
      get_inherit_value("previous-margin-bottom", CssValue::Number(0.0)).to_number();

    let mut child_previous_margin_bottom =
      get_inherit_value("child-previous-margin-bottom", CssValue::Number(0.0)).to_number();

    new_inherit_declarations.remove("previous-margin-bottom");

    if i > 0 {
      let previous_element = &elements[i - 1];

      // TODO: simplify
      match display.as_str() {
        "inline-block" => {
          y = previous_element.render_item.y;
          if previous_element
            .style
            .get("display")
            .unwrap_or(&"inline-block".to_string())
            .to_string()
            == "inline-block"
          {
            x = previous_element.render_item.x + previous_element.render_item.width;
          } else {
            y = reserved_block_y;
            y += f64::max(previous_element.render_item.margin_bottom, margin_top);

            new_inherit_declarations.insert(
              S("previous-margin-bottom"),
              CssValue::Number(previous_element.render_item.adjacent_margin_bottom),
            );
          }
        }
        _ => {
          y = reserved_block_y;
          y += f64::max(previous_element.render_item.margin_bottom, margin_top);

          new_inherit_declarations.insert(
            S("previous-margin-bottom"),
            CssValue::Number(previous_element.render_item.adjacent_margin_bottom),
          );
        }
      };
    } else {
      new_inherit_declarations.insert(
        S("previous-margin-bottom"),
        CssValue::Number(previous_margin_bottom),
      );

      y += f64::max(0.0, margin_top - previous_margin_bottom);
    }

    let element = &mut elements[i];

    let mut adjacent_margin_bottom = 0.0;

    if element.children.len() > 0 && element.tag_name != "SCRIPT" && element.tag_name != "STYLE" {
      new_inherit_declarations.insert(S("x"), CssValue::Number(x.clone()));
      new_inherit_declarations.insert(S("y"), CssValue::Number(y.clone()));

      let children_render_items = get_render_array(
        element.children.clone(),
        style.clone(),
        measure_text,
        Some(new_inherit_declarations),
      );
      array = [array.clone(), children_render_items.clone()].concat();

      for item in &children_render_items {
        width = f64::max(item.width + (item.x - x), width);
        height = f64::max(item.height + (item.y - y), height);
      }

      if i == elements_len - 1 {
        adjacent_margin_bottom = margin_bottom;
      }

      adjacent_margin_bottom = margin_bottom;

      adjacent_margin_bottom = f64::max(
        adjacent_margin_bottom,
        children_render_items
          .first()
          .unwrap()
          .adjacent_margin_bottom,
      );
    }

    match element.node_type {
      NodeType::Text => {
        element.node_value = element
          .node_value
          .replace("&nbsp;", " ")
          .replace("&gt;", ">")
          .replace("&lt;", "<");
        let size = measure_text(element.node_value.clone(), font_size, font_path.to_string());
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
        };
        element.render_item = item.clone();
        array = [vec![item], array.clone()].concat();
        reserved_block_y = f64::max(height + y, reserved_block_y);
      }
    }
  }

  return array;
}
