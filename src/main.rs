extern crate find_folder;
extern crate piston_window;

use piston_window::*;
use std::fs;
use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use std::{
    fs::File,
    io::{BufWriter, Write},
};

use std::time::Instant;

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
pub struct RenderItem {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    text: String,
    render: bool,
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
            if ignore
                && (captured_text == "</script"
                    || captured_text == "</style"
                    || captured_text.ends_with("--"))
            {
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
                if attr.value.len() == 0 {
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
                let element_new_ptr: *mut DomElement;

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
    let mut now = Instant::now();
    let tokens = tokenize(html.to_string());
    let elements = build_tree(tokens);
    println!("Parsed in {}", now.elapsed().as_secs_f64());

    return elements;
}

fn get_styles(tree: Vec<DomElement>) -> String {
    let mut style: String = "".to_string();

    for element in tree {
        if element.children.len() > 0 && element.tag_name != "SCRIPT" {
            style += &get_styles(element.children.clone());
        }
        match element.node_type {
            NodeType::Text => unsafe {
                if element.parent_node != std::ptr::null_mut()
                    && (*element.parent_node).tag_name == "STYLE"
                {
                    style += &element.node_value;
                }
            },
            _ => {}
        }
    }

    return style;
}

fn get_render_array(tree: Vec<DomElement>, y_base: Option<f64>) -> Vec<RenderItem> {
    let mut array: Vec<RenderItem> = vec![];

    let mut y = y_base.unwrap_or(0.0);
    let mut new_y: f64 = y.clone();

    for element in tree {
        // TODO: display: inline-block
        y = new_y.clone();

        if element.children.len() > 0 && element.tag_name != "SCRIPT" && element.tag_name != "STYLE"
        {
            let children_render_items =
                get_render_array(element.children.clone(), Some(new_y.clone()));
            array = [children_render_items.clone(), array.clone()].concat();

            for item in children_render_items {
                new_y += item.height;
            }
        }
        match element.node_type {
            NodeType::Comment => {}
            _ => {
                let item = RenderItem {
                    x: 0.0,
                    y: y,
                    width: 0.0,
                    height: if element.node_value != "" { 16.0 } else { 0.0 },
                    text: element.node_value.replace("&nbsp;", " "),
                    render: element.node_type == NodeType::Text && element.node_value != "",
                };
                new_y += item.height;
                array.push(item);
            }
        }
    }

    return array;
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
    let mut window = BrowserWindow::create();
    window.load_file("index.html");
    while true {}
}

#[derive(Clone, Debug)]
pub struct BrowserWindowInner {
    render_array: Vec<RenderItem>,
}

pub struct BrowserWindow {
    inner: Arc<Mutex<BrowserWindowInner>>,
}

impl BrowserWindow {
    pub fn create() -> BrowserWindow {
        let browser_window = BrowserWindow {
            inner: Arc::from(Mutex::new(BrowserWindowInner {
                render_array: vec![],
            })),
        };

        let inner = browser_window.inner.clone();

        thread::spawn(move || {
            let mut window: PistonWindow = WindowSettings::new("Graviton", [1024, 1024])
                .exit_on_esc(true)
                .build()
                .unwrap();

            let assets = find_folder::Search::ParentsThenKids(3, 3)
                .for_folder("assets")
                .unwrap();

            let mut glyphs = window.load_font(assets.join("lato.ttf")).unwrap();

            while let Some(event) = window.next() {
                window.draw_2d(&event, |context, graphics, device| {
                    clear([1.0; 4], graphics);

                    let arr = &inner.lock().unwrap().render_array;

                    for item in arr {
                        let transform = context.transform.trans(item.x, item.y + item.height);

                        text::Text::new_color([0.0, 0.0, 0.0, 1.0], 14)
                            .draw(
                                &item.text,
                                &mut glyphs,
                                &context.draw_state,
                                transform,
                                graphics,
                            )
                            .unwrap();

                        glyphs.factory.encoder.flush(device);
                    }
                });
            }
        });

        return browser_window;
    }

    pub fn load_file(&mut self, url: &str) {
        let write_file = File::create("out.txt").unwrap();
        let mut writer = BufWriter::new(&write_file);

        let contents = fs::read_to_string(url).expect("error while reading the file");

        let parsed = parse(&contents);
        let printed = print_dom(parsed.clone(), None);
        writeln!(&mut writer, "{}", printed);

        let now = Instant::now();

        let style = get_styles(parsed.clone());
        println!("{}", style);

        self.inner.lock().unwrap().render_array = get_render_array(parsed.clone(), None)
            .into_iter()
            .filter(|i| i.render)
            .collect();

        println!("Layouted in {}", now.elapsed().as_secs_f64());
    }
}
