use crate::{css_value::CssValue, styles::{PropertyImpl, ScalarEvaluationContext, Style, StyleScalar}};

use super::font_style::FontStyle;

#[derive(Clone, Debug)]
pub struct Font {
  pub family: FontFamily,
  pub weight: FontWeight,
  pub style: FontStyle,
}

impl Font {
  pub fn empty() -> Font {
    Font {
      family: FontFamily::empty(),
      weight: FontWeight::empty(),
      style: FontStyle::empty(),
    }
  }

  pub fn evaluate(&mut self) -> &Self {
    self
  }

  pub fn create_inherited(&self, inherit_style: &Style) -> Font {
    let mut font = self.clone();

    font.family = self.family.create_inherited(inherit_style);
    font.weight = self.weight.create_inherited(inherit_style);

    font
  }

  pub fn get_path(&self) -> String {
    let mut path = "".to_string();
  
    let font_family = "Times New Roman";
    let font_weight = self.weight.get();
    if font_family.to_lowercase() == "times new roman" {
      path = match font_weight {
        100..699 => match self.style.get().as_str() {
          "italic" => "Times New Roman Italique 400.ttf".to_string(),
          _ => "Times New Roman 400.ttf".to_string(),
        },
        700..999 => match self.style.get().as_str() {
          "italic" => "Times New Roman Italique 700.ttf".to_string(),
          _ => "Times New Roman 700.ttf".to_string(),
        },
        _ => "Times New Roman 400.ttf".to_string(),
      }
    }

    path
  }
}

#[derive(Clone, Debug)]
pub struct FontFamily {
  value: Option<CssValue>,
}

impl FontFamily {
  pub fn new(value: CssValue) -> FontFamily {
    FontFamily { value: Some(value) }
  }

  pub fn empty() -> FontFamily {
    FontFamily { value: None }
  }

  pub fn get(&self) -> String {
    let default = "Times New Roman".to_string();
    match &self.value {
      Some(v) => {
        match v {
          CssValue::String(s) => s.clone(),
          _ => default,
        }
      },
      None => default,
    }
  }
}

impl PropertyImpl for FontFamily {
  fn create_inherited(&self, inherit_style: &Style) -> FontFamily {
    let mut family = self.clone();
    if self.value.is_none() {
      family = inherit_style.font.family.clone();
    }
    family
  }

  fn from_value(value: CssValue) -> FontFamily {
    match &value {
      CssValue::Multiple(values) => {
        if values.len() > 0 {
          FontFamily::new(values[0].clone())
        } else {
          FontFamily::empty()
        }
      },
      _ => FontFamily::empty(),
    }
  }
}

#[derive(Clone, Debug)]
pub struct FontWeight {
  value: Option<CssValue>,
  inherited: Option<i32>,
}

impl FontWeight {
  pub fn new(value: CssValue) -> FontWeight {
    FontWeight { value: Some(value), inherited: None }
  }

  pub fn empty() -> FontWeight {
    FontWeight { value: None, inherited: None }
  }

  pub fn get(&self) -> i32 {
    match &self.value {
      Some(v) => {
        match v {
          CssValue::Number(num) => num.floor() as i32,
          CssValue::String(s) => {
            match s.as_str() {
              "normal" => 400,
              "bold" => 700,
              "bolder" => match self.inherited {
                Some(v) => match v {
                  100..=399 => 400,
                  400..=499 => 700,
                  500..=599 => 700,
                  600..=699 => 700,
                  700..=799 => 900,
                  800..=899 => 900,
                  900..=999 => 900,
                  _ => 700,
                },
                _ => 700,
              },
              "lighter" => match self.inherited {
                Some(v) => match v {
                  100..=399 => 100,
                  400..=499 => 100,
                  500..=599 => 100,
                  600..=699 => 400,
                  700..=799 => 400,
                  800..=899 => 400,
                  900..=999 => 400,
                  _ => 100,
                },
                _ => 100,
              },
              _ => 400,
            }
          },
        _ => 400,
        }
      },
      None => 400,
    }
  }
}

impl PropertyImpl for FontWeight {
  fn create_inherited(&self, inherit_style: &Style) -> FontWeight {
    let mut size = self.clone();
    if self.value.is_none() {
      size = inherit_style.font.weight.clone();
      size.inherited = Some(inherit_style.font.weight.get());
    }
    size
  }

  fn from_value(value: CssValue) -> FontWeight {
    match value {
      CssValue::Multiple(values) => {
        if values.len() > 0 {
          FontWeight::new(values[0].clone())
        } else {
          FontWeight::empty()
        }
      },
      
      _ => FontWeight::empty(),
    }
  }
}
