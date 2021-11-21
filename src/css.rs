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

  pub fn to_string(&self) -> String {
    match &self {
      CssValue::String(obj) => obj.to_string(),
      _ => "".to_string(),
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

pub fn parse_css(css: &str) -> Vec<StyleRule> {
  let mut list: Vec<StyleRule> = vec![];

  let mut captured_text = "".to_string();
  let mut captured_code = "".to_string();

  let mut style_rule = StyleRule::new();
  let mut declaration = KeyValue::new();

  let mut is_capturing_selector = true;

  let chars = css.chars().enumerate();

  for (i, c) in chars {
    captured_code.push(c);

    if c == '{' {
      style_rule.selector = captured_text.trim().to_string();
      captured_text = "".to_string();
      is_capturing_selector = false;
    } else if c == ':' && !is_capturing_selector {
      declaration.0 = captured_text.trim().to_string();
      captured_text = "".to_string();
    } else if c == ';' {
      declaration.1 = captured_text.trim().to_string();
      style_rule.declarations.push(declaration);
      declaration = KeyValue::new();
      captured_text = "".to_string();
    } else if c == '}' {
      style_rule.css = captured_code.trim().to_string();
      list.push(style_rule);
      style_rule = StyleRule::new();
      captured_code = "".to_string();
      is_capturing_selector = true;
    } else {
      captured_text.push(c);
    }
  }

  return list;
}
