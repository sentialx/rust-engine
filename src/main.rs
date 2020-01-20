use std::cell::RefCell;
use std::rc::Rc;
use std::rc::Weak;

#[derive(Clone, Debug)]
pub enum NodeType {
    Element,
    Attribute,
    Text,
    Comment,
    DocumentType,
}

pub enum TagType {
    None,
    Opening,
    Closing,
    SelfClosing,
}

#[derive(Clone, Debug)]
pub struct DomElement {
    children: Vec<Rc<RefCell<DomElement>>>,
    parentNode: Option<Weak<RefCell<DomElement>>>,
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
            parentNode: None,
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
    let mut chars = token.chars();

    if chars.nth(0).unwrap() == '<' && chars.clone().last().unwrap() == '>' {
        if chars.nth(1).unwrap() == '/' {
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
    let mut chars = token.chars();

    if chars.nth(0).unwrap() == '<' && chars.clone().last().unwrap() == '>' {
        if token.starts_with("<!--") {
            return NodeType::Comment;
        } else if chars.nth(1).unwrap() == '!' {
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
            tokens.push(capturedText.clone());
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
        return getOpeningTag(tagName, unsafe {
            Some(&(*element.unwrap().parentNode.unwrap().into_raw()).into_inner())
        });
    }
}

fn build_tree<'a>(tokens: Vec<String>) -> Vec<DomElement<'a>> {
    let mut elements: Vec<DomElement> = vec![];
    let mut openTags: Vec<String> = vec![];

    let mut parent: Option<&DomElement> = None;

    for token in tokens {
        let tagName = getTagName(&token);
        let tagType = getTagType(&token, &tagName);
        let nodeType = getNodeType(&token, &tagName);

        match tagType {
            TagType::Closing => match parent {
                Some(item) => {
                    let openTagIndex = &openTags.iter().rev().position(|s| s == &tagName);
                    match openTagIndex {
                        Some(index) => {
                            if item.tagName == tagName.to_string() {
                                parent = *item.parentNode.as_ref();
                            } else {
                                let openingElement = getOpeningTag(&tagName, Some(item));
                                match openingElement {
                                    Some(el) => parent = *el.parentNode.as_ref(),
                                    None => {}
                                }
                            }

                            openTags.remove(*index);
                        }
                        None => {}
                    }
                }
                None => {}
            },
            _ => {
                let mut element = DomElement::new(nodeType.clone());

                match parent {
                    Some(item) => {
                        let mut new: DomElement = item.clone();
                        let mut el = element.clone();
                        el.parentNode = Box::from(Some(&new));
                        new.children.push(el);
                    }
                    None => {
                        elements.push(element);
                    }
                }

                /*match nodeType {
                    NodeType::Element => {
                        element.tagName = tagName.clone();
                        openTags.push(tagName.clone());

                        match tagType {
                            TagType::Opening => {
                                parent = Some(&element);
                            }
                            _ => {}
                        }
                    }
                    NodeType::Text => {
                        element.nodeValue = token;
                    }
                    NodeType::Comment => {
                        // TODO: getCommentText
                    }
                    _ => {}
                }*/
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

fn main() {
    let parsed = parse("<div>aha</div>");
    println!("{:#?}", parsed);
}
