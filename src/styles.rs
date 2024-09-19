use crate::colors::ColorTupleA;
use crate::css::*;
use crate::css_value::{CssSizeUnit, CssValue};
use crate::html::{DomElement, NodeType};
use crate::properties::color::Color;
use crate::properties::font::{Font, FontFamily, FontWeight};
use crate::properties::font_size::FontSize;
use crate::properties::font_style::FontStyle;
use crate::properties::margin::{Margin, MarginComponent};
use crate::properties::string_property::StringProperty;
use crate::utils::*;
use std::collections::{HashMap, HashSet};

pub trait PropertyImpl {
  fn create_inherited(&self, inherit_style: &Style) -> Self;
  fn from_value(value: CssValue) -> Self;
}

#[derive(Clone, Debug)]
pub struct StyleScalar {
  pub value: CssValue,
  calculated: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct ScalarEvaluationContext {
  pub percent_base: f32,
  pub em_base: f32,
  pub rem_base: f32,
}

impl ScalarEvaluationContext {
  pub fn from_parent(font_size: f32, percent_base: f32) -> ScalarEvaluationContext {
    ScalarEvaluationContext {
      percent_base: percent_base,
      em_base: font_size,
      rem_base: font_size,
    }
  }
}

impl StyleScalar {
  pub fn new(value: CssValue) -> StyleScalar {
    StyleScalar { value: value, calculated: None }
  }

  pub fn zero() -> StyleScalar {
    StyleScalar { value: CssValue::Invalid, calculated: Some(0.0) }
  }

  pub fn evaluate(&mut self, ctx: &ScalarEvaluationContext) -> &Self {
    match &self.value {
      CssValue::Number(v) => {
        self.calculated = Some(*v);
      }
      CssValue::Size(size) => {
        match size.unit {
          CssSizeUnit::Percent => {
            self.calculated = Some(ctx.percent_base * size.value / 100.0);
          }
          CssSizeUnit::Em => {
            self.calculated = Some(ctx.em_base * size.value);
          }
          // CssSizeUnit::Rem => {
          //   self.calculated = Some(ctx.rem_base * size.value);
          // }
          CssSizeUnit::Px => {
            self.calculated = Some(size.value);
          }
          _ => {}
        }
      }
      _ => {}
    }

    self
  }

  pub fn get(&self) -> Option<f32> {
    self.calculated
  }
}


#[derive(Clone, Debug)]
pub enum Property {
  Margin(Margin),
  Padding(Margin),
}

#[derive(Clone, Debug)]
pub struct Declaration {
  pub key: String,
  pub value: CssValue,
  pub important: bool,
}

#[derive(Clone, Debug)]
pub struct StyleRule {
  pub css: String,
  pub selector: CssSelector,
  pub declarations: Vec<Declaration>,
}

impl StyleRule {
  pub fn new() -> StyleRule {
    StyleRule {
      css: "".to_string(),
      selector: CssSelector::AndGroup { selectors: vec![] },
      declarations: vec![],
    }
  }
}

#[derive(Clone, Debug)]
pub struct ComputedMargin {
  pub top: f32,
  pub right: f32,
  pub bottom: f32,
  pub left: f32,
}

#[derive(Clone, Debug)]
pub struct ComputedStyle {
  pub margin: ComputedMargin,
  pub padding: ComputedMargin,
  pub font_family: String,
  pub font_weight: i32,
  pub font_size: f32,
  pub font_style: String,
  pub display: String,
  pub float: String,
  pub text_decoration: String,
  pub color: ColorTupleA,
  pub background_color: ColorTupleA,
  pub position: String,
  pub inset: ComputedMargin,
}

#[derive(Clone, Debug)]
pub struct Style {
  pub margin: Margin,
  pub padding: Margin,
  pub font: Font,
  pub font_size: FontSize,
  pub display: StringProperty,
  pub float: StringProperty,
  pub text_decoration: StringProperty,
  pub color: Color,
  pub background_color: Color,
  pub position: StringProperty,
  pub inset: Margin,
  inserted: HashSet<String>,
}

impl Style {
  pub fn new() -> Style {
    Style {
      margin: Margin::empty(),
      padding: Margin::empty(),
      font: Font::empty(),
      font_size: FontSize::empty(),
      display: StringProperty::empty(false, "inline"),
      float: StringProperty::empty(false, "none"),
      text_decoration: StringProperty::empty(false, "none"),
      color: Color::empty(true, (0.0, 0.0, 0.0, 1.0)),
      background_color: Color::empty(false, (0.0, 0.0, 0.0, 0.0)),
      position: StringProperty::empty(false, "static"),
      inset: Margin::empty(),
      inserted: HashSet::new(),
    }
  }

  // TODO: handle !important
  pub fn insert_declarations(&mut self, declarations: &Vec<Declaration>) {
    // remove consequtive non-important declarations

    for declaration in declarations {

      if self.inserted.contains(&declaration.key) && !declaration.important {
      
        continue;
      }

      if declaration.important {
        if declaration.key == "margin-left" {
          println!("inserting margin-left: {:?}", declaration);
        }
        self.inserted.insert(declaration.key.clone());
        
      }
      
      match declaration.key.as_str() {
        "margin-top" => self.margin.top = MarginComponent::from_value(declaration.value.clone()),
        "margin-right" => self.margin.right = MarginComponent::from_value(declaration.value.clone()),
        "margin-bottom" => self.margin.bottom = MarginComponent::from_value(declaration.value.clone()),
        "margin-left" => self.margin.left = MarginComponent::from_value(declaration.value.clone()),
        "padding-top" => self.padding.top = MarginComponent::from_value(declaration.value.clone()),
        "padding-right" => self.padding.right = MarginComponent::from_value(declaration.value.clone()),
        "padding-bottom" => self.padding.bottom = MarginComponent::from_value(declaration.value.clone()),
        "padding-left" => self.padding.left = MarginComponent::from_value(declaration.value.clone()),
        "margin" => self.margin = Margin::from_value(declaration.value.clone()),
        "padding" => self.padding = Margin::from_value(declaration.value.clone()),
        "font-family" => self.font.family = FontFamily::from_value(declaration.value.clone()),
        "font-weight" => self.font.weight = FontWeight::from_value(declaration.value.clone()),
        "font-size" => self.font_size = FontSize::from_value(declaration.value.clone()),
        "font-style" => self.font.style = FontStyle::from_value(declaration.value.clone()),
        "display" => self.display.from_value(declaration.value.clone()),
        "float" => self.float.from_value(declaration.value.clone()),
        "text-decoration" => self.text_decoration.from_value(declaration.value.clone()),
        "color" => self.color.from_value(declaration.value.clone()),
        "background-color" => self.background_color.from_value(declaration.value.clone()),
        "position" => self.position.from_value(declaration.value.clone()),
        "top" => self.inset.top = MarginComponent::from_value(declaration.value.clone()),
        "right" => self.inset.right = MarginComponent::from_value(declaration.value.clone()),
        "bottom" => self.inset.bottom = MarginComponent::from_value(declaration.value.clone()),
        "left" => self.inset.left = MarginComponent::from_value(declaration.value.clone()),
        "inset" => self.inset = Margin::from_value(declaration.value.clone()),
        _ => {}
      }
    }
  }

  pub fn create_inherited(&self, inherit_style: &Style) -> Style {
    Style {
      margin: self.margin.create_inherited(inherit_style),
      padding: self.padding.create_inherited(inherit_style),
      font: self.font.create_inherited(inherit_style),
      font_size: self.font_size.create_inherited(inherit_style),
      display: self.display.create_inherited(&inherit_style.display),
      float: self.float.create_inherited(&inherit_style.float),
      text_decoration: self.text_decoration.create_inherited(&inherit_style.text_decoration),
      color: self.color.create_inherited(&inherit_style.color),
      background_color: self.background_color.create_inherited(&inherit_style.background_color),
      position: self.position.create_inherited(&inherit_style.position),
      inset: self.inset.create_inherited(inherit_style),
      inserted: self.inserted.clone(),
    }
  }

  pub fn to_computed_style(&self) -> ComputedStyle {
    ComputedStyle {
      margin: self.margin.to_computed(),
      padding: self.padding.to_computed(),
      font_family: self.font.family.get(),
      font_weight: self.font.weight.get(),
      font_size: self.font_size.get(),
      font_style: self.font.style.get(),
      display: self.display.get(),
      float: self.float.get(),
      text_decoration: self.text_decoration.get(),
      color: self.color.get(),
      background_color: self.background_color.get(),
      position: self.position.get(),
      inset: self.inset.to_computed(),
    }
  }
}

pub fn get_styles(tree: Vec<DomElement>, parent: Option<DomElement>) -> String {
  let mut style: String = "".to_string();

  for element in tree {
    if element.children.len() > 0 && element.tag_name != "SCRIPT" {
      style += &get_styles(element.children.clone(), Some(element.clone()));
    }
    match element.node_type {
      NodeType::Text => match &parent {
        Some(p) => {
          if p.tag_name == "STYLE" {
            style += &element.node_value;
          }
        }
        None => {}
      },
      _ => {}
    }
  }

  return style;
}

pub fn get_declaration_value(
  declarations: &HashMap<String, String>,
  key: &str,
  default: &str,
) -> String {
  (*declarations.get(key).unwrap_or(&default.to_string())).to_string()
}

pub fn get_inheritable_declaration_option(
  declarations: &HashMap<String, String>,
  inherit_declarations: &HashMap<String, CssValue>,
  key: &str,
) -> Option<CssValue> {
  let inherited = match inherit_declarations.get(key) {
    Some(v) => Some(v.clone()),
    None => None,
  };

  match declarations.get(key) {
    Some(v) => if v == "inherit" { inherited } else { Some(CssValue::String(v.clone())) },
    None => inherited,
  }
}

pub fn get_inheritable_declaration_value(
  declarations: &HashMap<String, String>,
  inherit_declarations: &HashMap<String, CssValue>,
  key: &str,
  default: CssValue,
) -> CssValue {
  match get_inheritable_declaration_option(declarations, inherit_declarations, key) {
    Some(v) => v.clone(),
    None => default,
  }
}
