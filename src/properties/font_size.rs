use crate::{css_value::{CssSize, CssSizeUnit, CssValue}, styles::{PropertyImpl, ScalarEvaluationContext, Style, StyleScalar}};

#[derive(Clone, Debug)]
pub struct FontSize {
  value: Option<StyleScalar>,
}

impl FontSize {
  pub fn new(value: CssValue) -> FontSize {
    FontSize { value: Some(StyleScalar::new(value)) }
  }

  pub fn empty() -> FontSize {
    FontSize { value: None }
  }

  pub fn get(&self) -> f32 {
    match &self.value {
      Some(value) => match value.get() {
        Some(value) => value,
        None => 16.0,
      },
      None => 16.0,
    }
  }

  pub fn evaluate(&mut self) -> &Self {
    let ctx = ScalarEvaluationContext { percent_base: 16.0 };
    match &mut self.value {
      Some(value) => {
        value.evaluate(&ctx);
      },
      None => {},
    }
    self
  }
}

impl PropertyImpl for FontSize {
  fn create_inherited(&self, inherit_style: &Style) -> FontSize {
    let mut size = self.clone();
    if self.value.is_none() {
      size = inherit_style.font_size.clone();
    }
    size
  }

  fn from_value(value: CssValue) -> FontSize {
    match value {
      CssValue::Multiple(values) => {
        if values.len() > 0 {
          FontSize::new(values[0].clone())
        } else {
          FontSize::empty()
        }
      },
      _ => FontSize::empty(),
    }
  }
}