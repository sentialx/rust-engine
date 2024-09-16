use crate::{css_value::CssValue, styles::{PropertyImpl, Style, StyleScalar}};

#[derive(Clone, Debug)]
pub struct StringProperty {
  value: Option<CssValue>,
  inheritable: bool,
  default_value: String,
}

impl StringProperty {
  pub fn new(value: CssValue, inheritable: bool, default_value: &str) -> StringProperty {
    StringProperty { value: Some(value), inheritable, default_value: default_value.to_string() }
  }

  pub fn empty(inheritable: bool, default_value: &str) -> StringProperty {
    StringProperty { value: None, inheritable, default_value: default_value.to_string() }
  }

  pub fn get(&self) -> String {
    match &self.value {
      Some(value) => match value {
        CssValue::String(value) => value.to_string(),
        _ => self.default_value.to_string(),
      },
      None => self.default_value.to_string(),
    }
  }

  pub fn create_inherited(&self, inherit: &StringProperty) -> StringProperty {
    let mut size = self.clone();
    if self.value.is_none() && self.inheritable {
      size = inherit.clone();
    }
    size
  }

  pub fn from_value(&mut self, value: CssValue) {
    match value {
      CssValue::Multiple(values) => {
        if values.len() > 0 {
          self.value = Some(values[0].clone())
        }
      },
      _ => {},
    }
  }
}
