use crate::colors::ColorTupleA;
use crate::layout::*;
use crate::utils::*;
use std::collections::HashMap;
use std::hash::Hash;

#[derive(Clone, Debug, PartialEq)]
pub enum NodeType {
  Element,
  Text,
  DocumentType,
  Comment,
}

#[derive(Clone, Debug)]
pub enum TagType {
  None,
  Opening,
  Closing,
  SelfClosing,
}

#[derive(Clone, Debug)]
pub struct Margin {
  pub top: f64,
  pub right: f64,
  pub bottom: f64,
  pub left: f64,
}

#[derive(Clone, Debug)]
pub struct ComputedStyle {
  pub margin: Margin,
  pub padding: Margin,
  pub background_color: ColorTupleA,
  pub color: ColorTupleA,
  pub font_size: f64,
  pub font_path: String,
  pub text_decoration: String,
  pub display: String,
  pub float: String,
  pub hoverable: bool,
}

#[derive(Clone, Debug)]
pub struct TextLine {
  pub text: String,
  pub x: f64,
  pub y: f64,
}

#[derive(Clone, Debug)]
pub struct ComputedFlow {
  pub x: f64,
  pub y: f64,
  pub width: f64,
  pub height: f64,
  pub adjacent_margin_bottom: f64,
  pub hover_rect: Rect,
  // pub text_lines: Vec<TextLine>,
}

#[derive(Clone, Debug)]
pub struct DomElement {
  pub children: Vec<DomElement>,
  pub attributes: HashMap<String, String>,
  pub parent_node: *mut DomElement,
  pub node_value: String,
  pub node_type: NodeType,
  pub inner_html: String,
  pub outer_html: String,
  pub tag_name: String,
  pub style: HashMap<String, String>,
  pub is_hovered: bool,
  pub computed_flow: Option<ComputedFlow>,
  pub computed_style: Option<ComputedStyle>,
}

impl DomElement {
  pub fn new(node_type: NodeType) -> DomElement {
    DomElement {
      children: vec![],
      attributes: HashMap::new(),
      parent_node: std::ptr::null_mut(),
      node_type,
      inner_html: "".to_string(),
      outer_html: "".to_string(),
      node_value: "".to_string(),
      tag_name: "".to_string(),
      style: HashMap::new(),
      computed_flow: None,
      computed_style: None,
      is_hovered: false,
    }
  }
}

const SELF_CLOSING_TAGS: &[&str] = &[
  "AREA", "BASE", "BR", "COL", "COMMAND", "EMBED", "HR", "IMG", "INPUT", "KEYGEN", "LINK",
  "MENUITEM", "META", "PARAM", "SOURCE", "TRACK", "WBR",
];

fn get_tag_name(source: &str) -> String {
  return source
    .replace("<", "")
    .replace("/", "")
    .replace(">", "")
    .split(" ")
    .collect::<Vec<&str>>()[0]
    .to_uppercase()
    .trim()
    .to_string();
}

fn get_tag_type(token: &str, tag_name: &str) -> TagType {
  if token.starts_with("<") && token.ends_with(">") {
    if token.starts_with("</") {
      return TagType::Closing;
    } else if SELF_CLOSING_TAGS.contains(&tag_name) {
      return TagType::SelfClosing;
    } else {
      return TagType::Opening;
    }
  }

  return TagType::None;
}

fn get_node_type(token: &str) -> NodeType {
  if token.starts_with("<") && token.ends_with(">") {
    if token.starts_with("<!--") {
      return NodeType::Comment;
    } else if token.starts_with("<!") {
      return NodeType::DocumentType;
    } else {
      return NodeType::Element;
    }
  }

  return NodeType::Text;
}

pub fn tokenize(html: String) -> Vec<String> {
  let mut tokens: Vec<String> = vec![];

  let mut capturing = false;
  let mut captured_text = String::from("");

  let len = html.len();
  let chars = html.chars().enumerate();

  let mut ignore = false;
  let mut code_block = false;

  for (i, c) in chars {
    if (!code_block && c == '\n') || c == '\r' || c == '\t' {
      continue;
    }
    if (c == '<' || (code_block && c == '\n' && c != '<')) && !ignore {
      if capturing {
        captured_text = captured_text.to_string();
        if captured_text != "" {
          tokens.push(captured_text.clone().trim().to_string());
          if code_block && c == '\n' {
            tokens.push("<br/>".to_string());
          }
        }
      } else {
        capturing = true;
      }

      captured_text = String::from("");
    } else if c == '>' || i == len - 1 {
      if ignore
        && (captured_text.ends_with("--"))
        {
          ignore = false;
        }
        
        if ignore && (captured_text.ends_with("</script") || captured_text.ends_with("</style")) {
        ignore = false;
      }

      if code_block && captured_text == "</code" {
        code_block = false;
      }

      if !ignore {
        capturing = false;
        captured_text.push(c);
        captured_text = captured_text.to_string();
      }

      if captured_text.starts_with("<code") {
        code_block = true;
      }

      if !ignore && captured_text != "" {
        let mut add_suffix = "";
        
        if captured_text.ends_with("</script>") {
          add_suffix = "</script>";
          captured_text = captured_text.replace("</script>", "");
        }

        if captured_text.ends_with("</style>") {
          add_suffix = "</style>";
          captured_text = captured_text.replace("</style>", "");
        }

        tokens.push(captured_text.clone());

        if add_suffix != "" {
          tokens.push(add_suffix.to_string());
        }

        if captured_text.starts_with("<script") || captured_text.starts_with("<style") {
          ignore = true;
        }

 

        captured_text = String::from("");
      }
    } else if !capturing {
      captured_text = String::from("");
      capturing = true;
    }
    if capturing && (c != ' ' || (c == ' ' && captured_text != "")) {
      captured_text.push(c);

      if captured_text == "<!--" {
        ignore = true;
      }
    }
  }

  return tokens;
}

fn get_opening_tag<'a>(tag_name: &str, element: *const DomElement) -> Option<&'a DomElement> {
  if element == std::ptr::null_mut() {
    return None;
  }

  unsafe {
    if (*element).tag_name == tag_name {
      return Some(&*element);
    } else {
      return get_opening_tag(tag_name, (*element).parent_node);
    }
  }
}

fn get_attributes(source: String, tag_name: String) -> HashMap<String, String> {
  let mut map = HashMap::new();
  let mut attr = KeyValue::new();

  let mut capturing_value = false;
  let mut inside_quotes = false;

  let sliced = unsafe { source.get_unchecked(tag_name.len() + 1..source.len()) };
  let len = sliced.len();
  let chars = sliced.chars().enumerate();

  for (i, c) in chars {
    if c == '=' {
      capturing_value = true;
    } else if c == '"' {
      inside_quotes = !inside_quotes;
    } else if capturing_value {
      attr.1.push(c);
    } else if i != len - 1 && c != ' ' {
      attr.0.push(c);
    }

    if (c == '"' || c == ' ' || c == '>') && !inside_quotes {
      if attr.0.len() > 0 {
        if attr.1.len() == 0 {
          attr.1 = "true".to_string();
        }

        if attr.1.starts_with(" ") || attr.1.ends_with(" ") {
          attr.1 = attr.1.trim().to_string();
        }

        map.insert(attr.0.clone(), attr.1.clone());
      }

      attr = KeyValue::new();

      capturing_value = false;
      inside_quotes = false;
    }
  }

  return map;
}

fn build_tree(tokens: Vec<String>) -> Vec<DomElement> {
  let mut elements: Vec<DomElement> = vec![];

  let mut parent: *mut DomElement = std::ptr::null_mut();

  for token in tokens {
    let tag_name = get_tag_name(&token);
    let tag_type = get_tag_type(&token, &tag_name);
    let node_type = get_node_type(&token);

    match tag_type {
      TagType::Closing => {
        if parent != std::ptr::null_mut() {
          unsafe {
            if (*parent).tag_name == tag_name.to_string() {
              parent = (*parent).parent_node;
            } else {
              let opening_element = get_opening_tag(&tag_name, &*parent);
              match opening_element {
                Some(el) => parent = (*el).parent_node,
                None => {}
              }
            }
          }
        }
      }
      _ => {
        let mut element = DomElement::new(node_type.clone());
        let element_new_ptr: *mut DomElement;

        match node_type {
          NodeType::Element => {
            element.tag_name = tag_name.clone();
            element.attributes = get_attributes(token.clone(), tag_name.clone());
          }
          NodeType::Text => {
            element.node_value = token;
          }
          NodeType::Comment => {
            element.node_value =
              unsafe { token.get_unchecked(4..token.len() - 3).to_string() };
          }
          _ => {}
        }

        if parent != std::ptr::null_mut() {
          unsafe {
            element.parent_node = parent;
            (*parent).children.push(element);
            element_new_ptr = (*parent).children.last_mut().unwrap();
          }
        } else {
          elements.push(element);
          element_new_ptr = elements.last_mut().unwrap();
        }

        match node_type {
          NodeType::Element => match tag_type {
            TagType::Opening => {
              parent = element_new_ptr;
            }
            _ => {}
          },
          _ => {}
        }
      }
    }
  }

  return elements;
}

pub fn parse_html(html: &str) -> Vec<DomElement> {
  let tokens = tokenize(html.to_string());
  let elements = build_tree(tokens);

  return elements;
}
