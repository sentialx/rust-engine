extern crate find_folder;
extern crate piston_window;

mod colors;

use piston_window::character::CharacterCache;
use piston_window::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
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
pub struct StyleRule {
    css: String,
    selector: String,
    declarations: Vec<Attribute>,
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

#[derive(Clone, Debug)]
pub struct RenderItem {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    text: String,
    font_size: f64,
    font_path: String,
    render: bool,
    background: String,
    margin_bottom: f64,
    color: colors::ColorTupleA,
}

impl RenderItem {
    pub fn new() -> RenderItem {
        RenderItem {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            font_size: 16.0,
            margin_bottom: 0.0,
            render: false,
            color: (0.0, 0.0, 0.0, 1.0),
            background: S("none"),
            font_path: S(""),
            text: S(""),
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
    style: HashMap<String, String>,
    render_item: RenderItem,
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
            style: HashMap::new(),
            render_item: RenderItem::new(),
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
    let mut codeBlock = false;

    for (i, c) in chars {
        if (!codeBlock && c == '\n') || c == '\r' || c == '\t' {
            continue;
        }
        if c == '<' || (codeBlock && c == '\n' && c != '<') {
            if capturing {
                captured_text = captured_text.trim().to_string();
                if captured_text != "" {
                    tokens.push(captured_text.clone());
                    if codeBlock && c == '\n' {
                        tokens.push("<br/>".to_string());
                    }
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

            if codeBlock && captured_text == "</code" {
                codeBlock = false;
            }

            if !ignore {
                capturing = false;
                captured_text.push(c);
                captured_text = captured_text.trim().to_string();
            }

            if captured_text.starts_with("<code") {
                codeBlock = true;
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

fn parse_html(html: &str) -> Vec<DomElement> {
    let mut now = Instant::now();
    let tokens = tokenize(html.to_string());
    let elements = build_tree(tokens);
    println!("Parsed in {}", now.elapsed().as_secs_f64());

    return elements;
}

fn get_styles(tree: Vec<DomElement>, parent: Option<DomElement>) -> String {
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

fn get_declaration_value(
    declarations: &HashMap<String, String>,
    key: &str,
    default: &str,
) -> String {
    (*declarations.get(key).unwrap_or(&default.to_string())).to_string()
}

fn get_inheritable_declaration_option(
    declarations: &HashMap<String, String>,
    inherit_declarations: &HashMap<String, CssValue>,
    key: &str,
) -> Option<CssValue> {
    match declarations.get(key) {
        Some(v) => Some(CssValue::String(v.clone())),
        None => match inherit_declarations.get(key) {
            Some(v) => Some(v.clone()),
            None => None,
        },
    }
}

fn get_inheritable_declaration_value(
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

fn parse_numeric_css_value(value: &str, base_font_size: f64) -> f64 {
    let chars = value.chars().enumerate();

    let mut unit: String = "".to_string();
    let mut val_str: String = "".to_string();

    let mut capturing_unit = false;

    for (i, c) in chars {
        if c == 'p' || c == 'e' {
            capturing_unit = true;
        }

        if capturing_unit {
            unit.push(c);
        } else {
            val_str.push(c);
        }
    }

    let val_num: f64 = val_str.parse().unwrap();

    if unit == "em" {
        return val_num * base_font_size;
    }

    return val_num;
}

#[derive(Clone)]
pub enum CssValue {
    String(String),
    Number(f64),
    Color(colors::ColorTupleA),
}

impl CssValue {
    pub fn to_number(&self) -> f64 {
        match &self {
            CssValue::Number(obj) => *obj,
            _ => 0.0,
        }
    }

    pub fn to_string(&self) -> String {
        match &self {
            CssValue::String(obj) => obj.to_string(),
            _ => "".to_string(),
        }
    }
}

fn S(st: &str) -> String {
    st.to_string()
}

fn css_string(st: &str) -> CssValue {
    CssValue::String(st.to_string())
}

fn get_render_array(
    tree: Vec<DomElement>,
    style: Vec<StyleRule>,
    measure_text: &dyn Fn(String, f64, String) -> (f64, f64),
    inherit_declarations: Option<HashMap<String, CssValue>>,
) -> Vec<RenderItem> {
    let mut array: Vec<RenderItem> = vec![];

    let mut elements = tree.clone();

    let inherit_declarations = inherit_declarations.unwrap_or(HashMap::new());

    let x_base = (*inherit_declarations
        .get("x")
        .unwrap_or(&CssValue::Number(0.0)))
    .to_number();

    let y_base = (*inherit_declarations
        .get("y")
        .unwrap_or(&CssValue::Number(0.0)))
    .to_number();

    let mut reserved_block_y = y_base.clone();

    for i in 0..elements.clone().len() {
        let mut x = x_base.clone();
        let mut y = y_base.clone();

        let mut declarations: HashMap<String, String> = HashMap::new();
        let mut new_inherit_declarations = inherit_declarations.clone();

        {
            let mut element = &mut elements[i];
            for style_rule in style.clone() {
                if style_rule.selector.to_uppercase() == element.tag_name
                    || style_rule.selector == "*"
                {
                    for declaration in style_rule.declarations.clone() {
                        declarations.insert(declaration.name, declaration.value);
                    }
                }
            }

            element.style = declarations.clone();
        }

        let mut width = 0.0;
        let mut height = 0.0;

        let get_inherit_value = |k: &str, d: CssValue| {
            get_inheritable_declaration_value(&declarations, &inherit_declarations, k, d)
        };

        let display = get_declaration_value(&declarations, "display", "inline-block");
        let background = get_declaration_value(&declarations, "background", "none");

        let font_weight_css = get_inherit_value("font-weight", css_string("normal"));
        let font_weight = font_weight_css.to_string();

        let font_style_css = get_inherit_value("font-style", css_string("normal"));
        let font_style = font_style_css.to_string();

        let font_family_css = get_inherit_value("font-family", css_string("Times New Roman"));
        let font_family = font_family_css.to_string();

        let color_css = get_inherit_value("color", css_string("#000"));

        let color = match color_css {
            CssValue::Color(c) => c,
            CssValue::String(c) => match colors::parse_css_color(&c) {
                Ok(c) => c,
                Err(e) => {
                    println!("{}", e);
                    (0.0, 0.0, 0.0, 1.0)
                }
            },
            CssValue::Number(_) => (0.0, 0.0, 0.0, 1.0),
        };

        let mut base_font_size = 16.0;
        let mut font_size: f64 = base_font_size.clone();
        let mut font_path: String = "Times New Roman 400.ttf".to_string();

        new_inherit_declarations.insert(S("font-family"), font_family_css);
        new_inherit_declarations.insert(S("font-weight"), font_weight_css);
        new_inherit_declarations.insert(S("font-style"), font_style_css);
        new_inherit_declarations.insert(S("color"), CssValue::Color(color));

        if font_family.to_lowercase() == "times new roman" {
            if font_weight == "bold" {
                if font_style == "italic" {
                    font_path = "Times New Roman Italique 700.ttf".to_string();
                } else {
                    font_path = "Times New Roman 700.ttf".to_string();
                }
            } else {
                if font_style == "italic" {
                    font_path = "Times New Roman Italique 400.ttf".to_string();
                } else {
                    font_path = "Times New Roman 400.ttf".to_string();
                }
            }
        }

        // TODO: simplify
        {
            let font_size_inherit = inherit_declarations.get("font-size");
            let font_size_str = declarations.get("font-size");

            match font_size_str {
                Some(f) => {
                    match font_size_inherit {
                        Some(f_i) => {
                            base_font_size = (*f_i).to_number();
                        }
                        None => {}
                    }

                    font_size = parse_numeric_css_value(&f, base_font_size);
                }
                None => match font_size_inherit {
                    Some(f_i) => {
                        font_size = (*f_i).to_number();
                    }
                    None => {}
                },
            }

            new_inherit_declarations.insert("font-size".to_string(), CssValue::Number(font_size));
        }

        if display == "none" {
            continue;
        }

        let margin_top = parse_numeric_css_value(
            &get_declaration_value(&declarations, "margin-top", "0"),
            base_font_size,
        );

        let margin_bottom = parse_numeric_css_value(
            &get_declaration_value(&declarations, "margin-bottom", "0"),
            base_font_size,
        );

        if i > 0 {
            let previous_element = &elements[i - 1];

            // TODO: simplify
            match display.as_str() {
                "block" => {
                    y = reserved_block_y;
                    y += f64::max(previous_element.render_item.margin_bottom, margin_top);
                }
                "inline-block" => {
                    y = previous_element.render_item.y;
                    if previous_element
                        .style
                        .get("display")
                        .unwrap_or(&"inline-block".to_string())
                        .to_string()
                        == "inline-block"
                    {
                        x = previous_element.render_item.x + previous_element.render_item.width;
                    } else {
                        y = reserved_block_y;
                        y += f64::max(previous_element.render_item.margin_bottom, margin_top);
                    }
                }
                _ => {}
            };
        }

        let element = &mut elements[i];

        if element.children.len() > 0 && element.tag_name != "SCRIPT" && element.tag_name != "STYLE"
        {
            new_inherit_declarations.insert(S("x"), CssValue::Number(x.clone()));
            new_inherit_declarations.insert(S("y"), CssValue::Number(y.clone()));

            let children_render_items = get_render_array(
                element.children.clone(),
                style.clone(),
                measure_text,
                Some(new_inherit_declarations),
            );
            array = [array.clone(), children_render_items.clone()].concat();

            for item in &children_render_items {
                width = f64::max(item.width + (item.x - x), width);
                height = f64::max(item.height + (item.y - y), height);
            }
        }

        match element.node_type {
            NodeType::Text => {
                element.node_value = element
                    .node_value
                    .replace("&nbsp;", " ")
                    .replace("&gt;", ">")
                    .replace("&lt;", "<");
                let size =
                    measure_text(element.node_value.clone(), font_size, font_path.to_string());
                width = size.0;
                height = size.1;
            }
            _ => {}
        }

        match element.node_type {
            NodeType::Comment => {}
            _ => {
                let item = RenderItem {
                    x: x,
                    y: y,
                    width: width,
                    height: height,
                    background: background.to_string(),
                    text: element.node_value.clone(),
                    font_size: font_size,
                    font_path: font_path.to_string(),
                    margin_bottom: margin_bottom,
                    render: (element.node_type == NodeType::Text && element.node_value != "")
                        || *background != "none".to_string(),
                    color: color,
                };
                element.render_item = item.clone();
                array = [vec![item], array.clone()].concat();
                reserved_block_y = f64::max(height + y, reserved_block_y);
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

fn parse_css(css: &str) -> Vec<StyleRule> {
    let mut list: Vec<StyleRule> = vec![];

    let mut captured_text = "".to_string();
    let mut captured_code = "".to_string();

    let mut style_rule = StyleRule::new();
    let mut declaration = Attribute::new();

    let chars = css.chars().enumerate();

    for (i, c) in chars {
        captured_code.push(c);

        if c == '{' {
            style_rule.selector = captured_text.trim().to_string();
            captured_text = "".to_string();
        } else if c == ':' {
            declaration.name = captured_text.trim().to_string();
            captured_text = "".to_string();
        } else if c == ';' {
            declaration.value = captured_text.trim().to_string();
            style_rule.declarations.push(declaration);
            declaration = Attribute::new();
            captured_text = "".to_string();
        } else if c == '}' {
            style_rule.css = captured_code.trim().to_string();
            list.push(style_rule);
            style_rule = StyleRule::new();
            captured_code = "".to_string();
        } else {
            captured_text.push(c);
        }
    }

    return list;
}

fn main() {
    let mut window = BrowserWindow::create();
    window.load_file("index.html");
    while true {}
}

#[derive(Clone, Debug)]
pub struct BrowserWindowInner {
    url: String,
}

pub struct BrowserWindow {
    inner: Arc<Mutex<BrowserWindowInner>>,
}

impl BrowserWindow {
    pub fn create() -> BrowserWindow {
        let browser_window = BrowserWindow {
            inner: Arc::from(Mutex::new(BrowserWindowInner {
                url: "".to_string(),
            })),
        };

        let inner = browser_window.inner.clone();

        thread::spawn(move || {
            let mut url = "".to_string();

            let mut window: PistonWindow = WindowSettings::new("Graviton", [1024, 1024])
                .exit_on_esc(true)
                .build()
                .unwrap();

            let assets = find_folder::Search::ParentsThenKids(3, 3)
                .for_folder("assets")
                .unwrap();

            let mut glyphs_map: HashMap<String, Glyphs> = HashMap::new();

            let mut add_font = |name: &str| {
                glyphs_map.insert(
                    name.to_string(),
                    window.load_font(assets.join(name)).unwrap(),
                )
            };

            add_font("Times New Roman 400.ttf");
            add_font("Times New Roman 700.ttf");
            add_font("Times New Roman Italique 400.ttf");
            add_font("Times New Roman Italique 700.ttf");

            let mut render_array: Vec<RenderItem> = vec![];

            let default_css =
                fs::read_to_string("default_styles.css").expect("error while reading the file");
            let default_styles = parse_css(&default_css);

            while let Some(event) = window.next() {
                let new_url = (&inner.lock().unwrap()).url.clone();

                let mut refresh = |u: String| {
                    render_array = vec![];
                    let contents =
                        fs::read_to_string(u.clone()).expect("error while reading the file");
                    let dom_tree = parse_html(&contents);
                    let style = get_styles(dom_tree.clone(), None);
                    let parsed_css = parse_css(&style);
                    let closure_ref =
                        RefCell::new(|text: String, font_size: f64, font_family: String| {
                            let mut glyphs = glyphs_map.remove(font_family.as_str()).unwrap();
                            let res = glyphs.width(font_size.round() as u32, &text).unwrap();
                            glyphs_map.insert(font_family.clone(), glyphs);
                            return res;
                        });
                    render_array = get_render_array(
                        dom_tree.clone(),
                        [default_styles.clone(), parsed_css].concat(),
                        &move |text, font_size, font_family| {
                            /*match glyphs_map.get(font_family) {
                                Some(g) => {}
                                None => {
                                    glyphs_map.insert()
                                }
                            }*/
                            return (
                                (&mut *closure_ref.borrow_mut())(text, font_size, font_family),
                                font_size + 8.0,
                            );
                        },
                        None,
                    )
                    .into_iter()
                    .filter(|i| i.render)
                    .collect();
                };
                if url != new_url {
                    url = new_url;
                    refresh(url.clone());
                }
                if let Some(Button::Keyboard(key)) = event.press_args() {
                    if key == Key::F5 {
                        refresh(url.clone());
                    }
                };

                window.draw_2d(&event, |context, graphics, device| {
                    clear([1.0, 1.0, 1.0, 1.0], graphics);

                    for item in &render_array {
                        if item.background == "red" {
                            rectangle(
                                [1.0, 0.0, 0.0, 1.0],
                                [0.0, 0.0, item.width, item.height],
                                context.transform.trans(item.x, item.y),
                                graphics,
                            );
                        }

                        let mut glyphs = glyphs_map.remove(&item.font_path).unwrap();

                        let color = &item.color;

                        text::Text::new_color(
                            [
                                (color.0 / 255.0) as f32,
                                (color.1 / 255.0) as f32,
                                (color.2 / 255.0) as f32,
                                (color.3 / 255.0) as f32,
                            ],
                            2 * (item.font_size.round() as u32),
                        )
                        .draw(
                            &item.text,
                            &mut glyphs,
                            &context.draw_state,
                            context
                                .transform
                                .trans(item.x, item.y + item.height - 4.0)
                                .zoom(0.5),
                            graphics,
                        )
                        .unwrap();

                        glyphs.factory.encoder.flush(device);

                        glyphs_map.insert((&item).font_path.clone(), glyphs);
                    }
                });
            }
        });

        return browser_window;
    }

    pub fn load_file(&mut self, url: &str) {
        /*let write_file = File::create("out.txt").unwrap();
        let mut writer = BufWriter::new(&write_file);

        let printed = print_dom(parsed.clone(), None);
        writeln!(&mut writer, "{}", printed);

        let now = Instant::now();*/

        self.inner.lock().unwrap().url = url.to_string();

        // println!("Layouted in {}", now.elapsed().as_secs_f64());
    }
}
