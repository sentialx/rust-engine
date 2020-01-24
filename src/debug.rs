use crate::html::*;

pub fn print_dom(tree: Vec<DomElement>, level: Option<i32>) -> String {
  let mut result: String = "".to_string();
  let l = level.unwrap_or(3);
  let mut gap: String = (0..(l - 3)).map(|_| "|  ").collect::<String>();

  if gap.len() > 3 {
    gap += "|--";
  } else {
    gap += "|--";
  }

  for element in tree {
    match element.node_type {
      NodeType::Element => {
        result += &format!("{}{}", gap, element.tag_name);
        for attr in element.attributes {
          result += &format!(" {}=\"{}\"", attr.0, attr.1);
        }
        result += "\n";
      }
      _ => {
        result += &format!("{}\"{}\"\n", gap, element.node_value);
      }
    }

    if element.children.len() > 0 {
      result += &print_dom(element.children, Some(l + 1));
    }
  }

  return result;
}
