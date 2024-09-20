use crate::{colors::{hex_to_rgb, ColorTupleA}, css_value::CssValue, lisia_colors::match_named_color, styles::{PropertyImpl, Style, StyleScalar}};

#[derive(Clone, Debug)]
pub struct Color {
  value: Option<ColorTupleA>,
  inheritable: bool,
  default: ColorTupleA,
}

impl Color {
  pub fn new(value: ColorTupleA, inheritable: bool, default: ColorTupleA) -> Color {
    Color { value: Some(value), inheritable, default }
  }

  pub fn empty(inheritable: bool, default: ColorTupleA) -> Color {
    Color { value: None, inheritable, default }
  }

  pub fn get(&self) -> ColorTupleA {
    match &self.value {
      Some(value) => value.clone(),
      None => self.default,
    }
  }

  pub fn create_inherited(&self, inherit: &Color) -> Color {
    let mut size = self.clone();
    if (self.value.is_none() && self.inheritable) {
      size = inherit.clone();
    }
    size
  }

  pub fn from_value(&mut self, value: CssValue) {
    match value {
      CssValue::Multiple(values) => {
        if values.len() > 0 {
          match &values[0] {
            CssValue::String(value) => {
              match value.as_str() {
                "transparent" => {
                  self.value = Some((0.0, 0.0, 0.0, 0.0));
                },
                _ if value.starts_with("#") => {
                   
                    let color = hex_to_rgb(value);
                    let is_ok = color.is_ok();
                    let c = &color.unwrap_or((0.0, 0.0, 0.0));
                    self.value = Some((c.0, c.1, c.2, if is_ok { 1.0 } else { 0.0 }));
                  
                },
                _ => {
                  let c = match_named_color(value);
                  if c.is_some() {
                    let c = c.unwrap();
                    self.value = Some((c[0], c[1], c[2], 1.0));
                  }
                }
              }
            },
            CssValue::Function(func) => {
              match func.name.as_str() {
                "rgba(" | "rgb(" => {
                  let mut comps = vec![];
                  for arg in &func.args {
                    match arg {
                      CssValue::Number(num) => {
                        comps.push(num.floor() as i32);
                      },
                      _ => {},
                    }
                  }

                  if comps.len() == 3 {
                    self.value = Some((comps[0] as f32, comps[1] as f32, comps[2] as f32, 1.0));
                  } else if comps.len() == 4 {
                    self.value = Some((comps[0] as f32, comps[1] as f32, comps[2] as f32, (comps[3] as f32)));
                  }
                },
                _ => {},
              }
            },
            
            _ => {},
          }
        }
      },
      _ => {},
    }
  }
}
