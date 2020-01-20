use std::fs;

use std::{
    fs::File,
    io::{BufWriter, Write},
};

use std::time::Instant;

#[derive(Clone, Debug)]
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
pub struct Attribute {
    name: String,
    value: String,
}

impl Attribute {
    pub fn new() -> Attribute {
        Attribute {
            name: "".to_string(),
            value: "".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DomElement {
    children: Vec<DomElement>,
    attributes: Vec<Attribute>,
    parent_node: *mut DomElement,
    node_value: String,
    node_type: NodeType,
    inner_html: String,
    outer_html: String,
    tag_name: String,
}

impl DomElement {
    pub fn new(node_type: NodeType) -> DomElement {
        DomElement {
            children: vec![],
            attributes: vec![],
            parent_node: std::ptr::null_mut(),
            node_type,
            inner_html: "".to_string(),
            outer_html: "".to_string(),
            node_value: "".to_string(),
            tag_name: "".to_string(),
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

fn tokenize(html: String) -> Vec<String> {
    let mut tokens: Vec<String> = vec![];

    let mut capturing = false;
    let mut captured_text = String::from("");

    let len = html.len();
    let chars = html.chars().enumerate();

    let mut ignore = false;

    for (i, c) in chars {
        if c == '\n' || c == '\r' || c == '\t' {
            continue;
        }
        if c == '<' {
            if capturing {
                captured_text = captured_text.trim().to_string();
                if captured_text != "" {
                    tokens.push(captured_text.clone());
                }
            } else {
                capturing = true;
            }

            captured_text = String::from("");
        } else if c == '>' || i == len - 1 {
            if ignore && (captured_text == "</script" || captured_text == "</style") {
                ignore = false;
            }

            if !ignore {
                capturing = false;
                captured_text.push(c);
                captured_text = captured_text.trim().to_string();
            }

            if !ignore && captured_text != "" {
                tokens.push(captured_text.clone());

                if captured_text.starts_with("<script") || captured_text.starts_with("<style") {
                    ignore = true;
                }

                captured_text = String::from("");
            }
        } else if !capturing {
            captured_text = String::from("");
            capturing = true;
        }
        if capturing && c != ' ' || (c == ' ' && captured_text != "") {
            captured_text.push(c);
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

fn get_attributes(source: String, tag_name: String) -> Vec<Attribute> {
    let mut list: Vec<Attribute> = vec![];
    let mut attr = Attribute::new();

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
            attr.value.push(c);
        } else if i != len - 1 && c != ' ' {
            attr.name.push(c);
        }

        if (c == '"' || c == ' ' || c == '>') && !inside_quotes {
            if attr.name.len() > 0 {
                if (attr.value.len() == 0) {
                    attr.value = "true".to_string();
                }

                if attr.value.starts_with(" ") || attr.value.ends_with(" ") {
                    attr.value = attr.value.trim().to_string();
                }

                list.push(attr);
            }

            attr = Attribute::new();

            capturing_value = false;
            inside_quotes = false;
        }
    }

    return list;
}

fn build_tree(tokens: Vec<String>) -> Vec<DomElement> {
    let mut elements: Vec<DomElement> = vec![];
    let mut open_tags: Vec<String> = vec![];

    let mut parent: *mut DomElement = std::ptr::null_mut();

    for token in tokens {
        let tag_name = get_tag_name(&token);
        let tag_type = get_tag_type(&token, &tag_name);
        let node_type = get_node_type(&token);

        match tag_type {
            TagType::Closing => {
                if parent != std::ptr::null_mut() {
                    unsafe {
                        let open_tag_index = &open_tags.iter().rev().position(|s| s == &tag_name);
                        match open_tag_index {
                            Some(index) => {
                                if (*parent).tag_name == tag_name.to_string() {
                                    parent = (*parent).parent_node;
                                } else {
                                    let opening_element = get_opening_tag(&tag_name, &*parent);
                                    match opening_element {
                                        Some(el) => parent = (*el).parent_node,
                                        None => {}
                                    }
                                }
                                open_tags.remove(*index);
                            }
                            None => {}
                        }
                    }
                }
            }
            _ => {
                let mut element = DomElement::new(node_type.clone());
                let mut element_new_ptr: *mut DomElement = std::ptr::null_mut();

                match node_type {
                    NodeType::Element => {
                        element.tag_name = tag_name.clone();
                        element.attributes = get_attributes(token.clone(), tag_name.clone());

                        open_tags.push(tag_name.clone());
                    }
                    NodeType::Text => {
                        element.node_value = token;
                    }
                    NodeType::Comment => {
                        element.node_value =
                            unsafe { token.get_unchecked(4..token.len() - 3).trim().to_string() };
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

fn parse(html: &str) -> Vec<DomElement> {
    let tokens = tokenize(html.to_string());
    let elements = build_tree(tokens);

    return elements;
}

fn print_dom(tree: Vec<DomElement>, level: Option<i32>) -> String {
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
                    result += &format!(" {}=\"{}\"", attr.name, attr.value);
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

fn main() {
    let contents = fs::read_to_string("index.html").expect("error while reading the file");

    let now = Instant::now();
    let parsed = parse(&contents);
    let time = now.elapsed().as_secs_f64();

    let printed = print_dom(parsed, None);

    let write_file = File::create("out.txt").unwrap();
    let mut writer = BufWriter::new(&write_file);

    writeln!(&mut writer, "{}", printed);
    println!("{}", printed);

    println!("Parsed in {}", time);
}
