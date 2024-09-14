use crate::css::*;
use crate::html::*;
use crate::utils::*;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct StyleRule {
  pub css: String,
  pub selector: String,
  pub declarations: Vec<KeyValue>,
}

impl StyleRule {
  pub fn new() -> StyleRule {
    StyleRule {
      css: "".to_string(),
      selector: "".to_string(),
      declarations: vec![],
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
