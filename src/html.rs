// HTML parsing - converts HTML string to intermediate representation

use std::collections::HashMap;

use html5ever::{parse_document, tendril::TendrilSink, ParseOpts, tree_builder::TreeBuilderOpts};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

/// Node type in the parsed HTML tree
#[derive(Clone, Debug, PartialEq)]
pub enum HtmlNodeType {
    Element,
    Text,
    DocumentType,
    Comment,
}

/// Intermediate representation of a parsed HTML node.
/// This is a simple data structure with no references to Frame.
#[derive(Clone, Debug)]
pub struct HtmlNode {
    pub node_type: HtmlNodeType,
    pub tag_name: String,
    pub attributes: HashMap<String, String>,
    pub children: Vec<HtmlNode>,
    pub text_content: String,
}

impl HtmlNode {
    pub fn element(tag_name: &str) -> Self {
        Self {
            node_type: HtmlNodeType::Element,
            tag_name: tag_name.to_uppercase(),
            attributes: HashMap::new(),
            children: Vec::new(),
            text_content: String::new(),
        }
    }

    pub fn text(content: &str) -> Self {
        Self {
            node_type: HtmlNodeType::Text,
            tag_name: String::new(),
            attributes: HashMap::new(),
            children: Vec::new(),
            text_content: content.to_string(),
        }
    }

    pub fn comment(content: &str) -> Self {
        Self {
            node_type: HtmlNodeType::Comment,
            tag_name: String::new(),
            attributes: HashMap::new(),
            children: Vec::new(),
            text_content: content.to_string(),
        }
    }

    pub fn doctype(name: &str) -> Self {
        Self {
            node_type: HtmlNodeType::DocumentType,
            tag_name: String::new(),
            attributes: HashMap::new(),
            children: Vec::new(),
            text_content: name.to_string(),
        }
    }
}

/// Parse HTML string into an intermediate representation.
/// Returns a tree of HtmlNodes that can be converted to DomElements by Frame.
pub fn parse_html(html: &str) -> Vec<HtmlNode> {
    let opts = ParseOpts {
        tree_builder: TreeBuilderOpts {
            scripting_enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let dom = parse_document(RcDom::default(), opts).one(html);

    let mut nodes = Vec::new();
    for child in dom.document.children.borrow().iter() {
        if let Some(node) = build_node_from_handle(child) {
            nodes.push(node);
        }
    }
    nodes
}

fn build_node_from_handle(handle: &Handle) -> Option<HtmlNode> {
    let node = match &handle.data {
        NodeData::Document => return None,

        NodeData::Doctype { name, .. } => {
            HtmlNode::doctype(&name.to_string())
        }

        NodeData::Text { contents } => {
            HtmlNode::text(&contents.borrow().to_string())
        }

        NodeData::Comment { contents } => {
            HtmlNode::comment(&contents.to_string())
        }

        NodeData::Element { name, attrs, .. } => {
            let mut node = HtmlNode::element(&name.local.to_string());
            for attr in attrs.borrow().iter() {
                node.attributes.insert(
                    attr.name.local.to_string(),
                    attr.value.to_string(),
                );
            }
            node
        }

        _ => return None,
    };

    let mut result = node;
    for child in handle.children.borrow().iter() {
        if let Some(child_node) = build_node_from_handle(child) {
            result.children.push(child_node);
        }
    }

    Some(result)
}
