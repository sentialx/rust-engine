use crate::{css_value::CssValue, styles::{PropertyImpl, Style, StyleScalar}};

#[derive(Clone, Debug)]
pub struct FontStyle {
  value: Option<CssValue>,
}

impl FontStyle {
  pub fn new(value: CssValue) -> FontStyle {
    FontStyle { value: Some(value) }
  }

  pub fn empty() -> FontStyle {
    FontStyle { value: None }
  }

  pub fn get(&self) -> String {
    match &self.value {
      Some(value) => match value {
        CssValue::String(value) => value.to_string(),
        _ => "normal".to_string(),
      },
      None => "normal".to_string(),
    }
  }
}

impl PropertyImpl for FontStyle {
  fn create_inherited(&self, inherit_style: &Style) -> FontStyle {
    let mut size = self.clone();
    if self.value.is_none() {
      size = inherit_style.font.style.clone();
    }
    size
  }

  fn from_value(value: CssValue) -> FontStyle {
    match value {
      CssValue::Multiple(values) => {
        if values.len() > 0 {
          FontStyle::new(values[0].clone())
        } else {
          FontStyle::empty()
        }
      },
      _ => FontStyle::empty(),
    }
  }
}