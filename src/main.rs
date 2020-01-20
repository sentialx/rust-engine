#[derive(Clone, Debug)]
pub enum NodeType {
    Element,
    Attribute,
    Text,
    Comment,
    DocumentType,
}

#[derive(Clone, Debug)]
pub enum TagType {
    None,
    Opening,
    Closing,
    SelfClosing,
}

#[derive(Clone, Debug)]
pub struct DomElement {
    children: Vec<DomElement>,
    parentNode: *mut DomElement,
    nodeValue: String,
    nodeType: NodeType,
    innerHTML: String,
    outerHTML: String,
    tagName: String,
}

impl DomElement {
    pub fn new(nodeType: NodeType) -> DomElement {
        DomElement {
            children: vec![],
            parentNode: std::ptr::null_mut(),
            nodeType,
            innerHTML: "".to_string(),
            outerHTML: "".to_string(),
            nodeValue: "".to_string(),
            tagName: "".to_string(),
        }
    }
}

const SELF_CLOSING_TAGS: &[&str] = &[
    "AREA", "BASE", "BR", "COL", "COMMAND", "EMBED", "HR", "IMG", "INPUT", "KEYGEN", "LINK",
    "MENUITEM", "META", "PARAM", "SOURCE", "TRACK", "WBR",
];

fn getTagName(source: &str) -> String {
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

fn getTagType(token: &str, tagName: &str) -> TagType {
    if token.starts_with("<") && token.ends_with(">") {
        if token.starts_with("</") {
            return TagType::Closing;
        } else if SELF_CLOSING_TAGS.contains(&token) {
            return TagType::SelfClosing;
        } else {
            return TagType::Opening;
        }
    }

    return TagType::None;
}

fn getNodeType(token: &str, tagName: &str) -> NodeType {
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
    let mut capturedText = String::from("");

    for i in 0..html.len() {
        let c = html.chars().nth(i).unwrap();

        if c == '<' {
            if capturing {
                tokens.push(capturedText.to_string());
            } else {
                capturing = true;
            }
            capturedText = String::from("");
        } else if c == '>' || i == html.len() - 1 {
            capturing = false;
            capturedText.push(c);
            tokens.push(capturedText.to_string());
            capturedText = String::from("");
        } else if !capturing {
            capturedText = String::from("");
            capturing = true;
        }

        if capturing {
            capturedText.push(c);
        }
    }

    return tokens;
}

fn getOpeningTag<'a>(tagName: &str, element: Option<&'a DomElement>) -> Option<&'a DomElement> {
    if element.clone().unwrap().tagName == tagName {
        return element;
    } else {
        return getOpeningTag(tagName, unsafe { Some(&*element.unwrap().parentNode) });
    }
}

fn build_tree(tokens: Vec<String>) -> Vec<DomElement> {
    let mut elements: Vec<DomElement> = vec![];
    let mut openTags: Vec<String> = vec![];

    let mut parent: *mut DomElement = std::ptr::null_mut();

    for token in tokens {
        let tagName = getTagName(&token);
        let tagType = getTagType(&token, &tagName);
        let nodeType = getNodeType(&token, &tagName);

        match tagType {
            TagType::Closing => {
                if parent != std::ptr::null_mut() {
                    unsafe {
                        let openTagIndex = &openTags.iter().rev().position(|s| s == &tagName);
                        match openTagIndex {
                            Some(index) => {
                                if (*parent).tagName == tagName.to_string() {
                                    parent = (*parent).parentNode;
                                } else {
                                    let openingElement = getOpeningTag(&tagName, Some(&*parent));
                                    match openingElement {
                                        Some(el) => parent = (*el).parentNode,
                                        None => {}
                                    }
                                }
                                openTags.remove(*index);
                            }
                            None => {}
                        }
                    }
                }
            }
            _ => {
                let mut element = DomElement::new(nodeType.clone());
                let mut elementNewPtr: *mut DomElement = std::ptr::null_mut();

                match nodeType {
                    NodeType::Element => {
                        element.tagName = tagName.clone();
                        openTags.push(tagName.clone());
                    }
                    NodeType::Text => {
                        element.nodeValue = token;
                    }
                    NodeType::Comment => {
                        // TODO: getCommentText
                    }
                    _ => {}
                }

                if parent != std::ptr::null_mut() {
                    unsafe {
                        element.parentNode = parent;
                        (*parent).children.push(element);
                        elementNewPtr = (*parent).children.last_mut().unwrap();
                    }
                } else {
                    elements.push(element);
                    elementNewPtr = elements.last_mut().unwrap();
                }

                match nodeType {
                    NodeType::Element => match tagType {
                        TagType::Opening => {
                            parent = elementNewPtr;
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
    //println!("{:#?}", tokens);
    let elements = build_tree(tokens);

    return elements;
}

fn main() {
    let parsed = parse("<div>aha<div>b</div>");
    println!("{:#?}", parsed);
}
