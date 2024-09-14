use crate::colors::*;
use crate::styles::*;
use crate::utils::*;

#[derive(Clone)]
pub enum CssValue {
    String(String),
    Number(f64),
    Color(ColorTupleA),
}

impl CssValue {
    pub fn to_number(&self) -> f64 {
        match &self {
            CssValue::Number(obj) => *obj,
            _ => 0.0,
        }
    }

    pub fn to_string(&self) -> &String {
        match &self {
            CssValue::String(obj) => obj,
            _ => panic!("Cannot convert to string"),
        }
    }
}

pub fn css_string(st: &str) -> CssValue {
    CssValue::String(st.to_string())
}

pub fn parse_numeric_css_value(value: &str, base_font_size: f64) -> f64 {
    let chars = value.chars().enumerate();

    let mut unit: String = "".to_string();
    let mut val_str: String = "".to_string();

    let mut capturing_unit = false;

    for (i, c) in chars {
        if c == ' ' {
            continue;
        }

        if c == 'p' || c == 'e' {
            capturing_unit = true;
        }

        if capturing_unit {
            unit.push(c);
        } else {
            val_str.push(c);
        }
    }

    match val_str.parse() {
        Ok(val) => {
            if unit == "em" {
                val * base_font_size
            } else {
                val
            }
        }
        Err(e) => {
            println!("Error while parsing css value {}: {}", value, e);
            0.0
        }
    }
}

pub fn parse_rect_like_value(value: &str) -> (&str, &str, &str, &str) {
    let values: Vec<&str> = value.split(" ").collect();
    let mut top = "0";
    let mut right = "0";
    let mut bottom = "0";
    let mut left = "0";

    if values.len() == 1 {
        top = values[0];
        right = values[0];
        bottom = values[0];
        left = values[0];
    } else if values.len() == 2 {
        top = values[0];
        right = values[1];
        bottom = values[0];
        left = values[1];
    } else if values.len() == 3 {
        top = values[0];
        right = values[1];
        bottom = values[2];
        left = values[1];
    } else if values.len() == 4 {
        top = values[0];
        right = values[1];
        bottom = values[2];
        left = values[3];
    }

    return (top, right, bottom, left);
}

pub fn expand_shorthand_values(style_rule: &mut StyleRule) {
  let mut expanded_declarations = Vec::new();

  for declaration in &style_rule.declarations {
      let key = &declaration.0;
      let value = &declaration.1;

      if key == "margin" {
          let (top, right, bottom, left) = parse_rect_like_value(value);

          expanded_declarations.push(KeyValue::new_values("margin-top", top));
          expanded_declarations.push(KeyValue::new_values("margin-right", right));
          expanded_declarations.push(KeyValue::new_values("margin-bottom", bottom));
          expanded_declarations.push(KeyValue::new_values("margin-left", left));
      } else if key == "padding" {
          let (top, right, bottom, left) = parse_rect_like_value(value);

          expanded_declarations.push(KeyValue::new_values("padding-top", top));
          expanded_declarations.push(KeyValue::new_values("padding-right", right));
          expanded_declarations.push(KeyValue::new_values("padding-bottom", bottom));
          expanded_declarations.push(KeyValue::new_values("padding-left", left));
      }
  }

  style_rule.declarations.append(&mut expanded_declarations);
}

pub fn parse_css(css: &str) -> Vec<StyleRule> {
    let mut list: Vec<StyleRule> = vec![];

    let mut captured_text = "".to_string();
    let mut captured_code = "".to_string();

    let mut style_rule = StyleRule::new();
    let mut declaration = KeyValue::new();

    let mut is_capturing_selector = true;
    let mut inside_comment = false;

    let chars = css.chars().enumerate();

    for (i, c) in chars {
        if (c == '/' || c == '*') && captured_code.ends_with("/") && !inside_comment {
            inside_comment = true;
        } else if inside_comment {
            if (c == '/' && captured_code.ends_with("*")) {
                inside_comment = false;
                println!("Comment: {}", captured_code);
                captured_code = "".to_string();
                continue;
            }
        }

        captured_code.push(c);

        if inside_comment {
            continue;
        }

        if c == '/' {
            continue;
        }

        if c == '{' {
            style_rule.selector = captured_text.trim().to_string();
            captured_text = "".to_string();
            is_capturing_selector = false;
        } else if c == ':' && !is_capturing_selector {
            declaration.0 = captured_text.trim().to_string();
            captured_text = "".to_string();
        } else if c == ';' || c == '}' {
            if (declaration.0 != "") {
                declaration.1 = captured_text.trim().to_string();
                style_rule.declarations.push(declaration);
                declaration = KeyValue::new();
                captured_text = "".to_string();
            }

            if c == '}' {
                style_rule.css = captured_code.trim().to_string();
                expand_shorthand_values(&mut style_rule);
                list.push(style_rule);
                style_rule = StyleRule::new();
                captured_code = "".to_string();
                captured_text = "".to_string();
                is_capturing_selector = true;
            }
        } else {
            captured_text.push(c);
        }
    }

    return list;
}
