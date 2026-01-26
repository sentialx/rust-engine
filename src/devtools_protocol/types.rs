// Chrome DevTools Protocol (CDP) compatible type definitions

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// CDP Request format
#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// CDP Response format
#[derive(Debug, Clone, Serialize)]
pub struct Response {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ProtocolError>,
}

impl Response {
    pub fn success(id: u64, result: Value) -> Self {
        Response {
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: u64, code: i32, message: &str) -> Self {
        Response {
            id,
            result: None,
            error: Some(ProtocolError {
                code,
                message: message.to_string(),
            }),
        }
    }
}

/// CDP Error format
#[derive(Debug, Clone, Serialize)]
pub struct ProtocolError {
    pub code: i32,
    pub message: String,
}

// CDP Error codes
pub const ERROR_INVALID_PARAMS: i32 = -32602;
pub const ERROR_METHOD_NOT_FOUND: i32 = -32601;
pub const ERROR_INTERNAL: i32 = -32603;

/// CDP Event format (no id)
#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// CDP DOM.Node type
/// attributes is a flat array [name1, value1, name2, value2, ...] per CDP spec
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub node_id: u64,
    pub backend_node_id: u64,
    pub node_type: u32,
    pub node_name: String,
    pub local_name: String,
    pub node_value: String,
    pub child_node_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<Node>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Vec<String>>,
}

// CDP Node types
pub const NODE_TYPE_ELEMENT: u32 = 1;
pub const NODE_TYPE_TEXT: u32 = 3;
pub const NODE_TYPE_COMMENT: u32 = 8;
pub const NODE_TYPE_DOCUMENT: u32 = 9;
pub const NODE_TYPE_DOCUMENT_TYPE: u32 = 10;

/// CSS.CSSComputedStyleProperty
#[derive(Debug, Clone, Serialize)]
pub struct CSSComputedStyleProperty {
    pub name: String,
    pub value: String,
}

/// CSS.RuleMatch for getMatchedStylesForNode
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleMatch {
    pub rule: CSSRule,
    pub matching_selectors: Vec<u32>,
}

/// CSS.CSSRule
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CSSRule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_sheet_id: Option<String>,
    pub selector_list: SelectorList,
    pub style: CSSStyle,
}

/// CSS.SelectorList
#[derive(Debug, Clone, Serialize)]
pub struct SelectorList {
    pub selectors: Vec<SelectorData>,
    pub text: String,
}

/// CSS.Value (selector)
#[derive(Debug, Clone, Serialize)]
pub struct SelectorData {
    pub text: String,
}

/// CSS.CSSStyle
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CSSStyle {
    pub css_properties: Vec<CSSProperty>,
    pub shorthand_entries: Vec<ShorthandEntry>,
}

/// CSS.CSSProperty
#[derive(Debug, Clone, Serialize)]
pub struct CSSProperty {
    pub name: String,
    pub value: String,
}

/// CSS.ShorthandEntry
#[derive(Debug, Clone, Serialize)]
pub struct ShorthandEntry {
    pub name: String,
    pub value: String,
}

/// DOM.BoxModel
#[derive(Debug, Clone, Serialize)]
pub struct BoxModel {
    pub content: Vec<f64>,  // [x1,y1,x2,y2,x3,y3,x4,y4] quad
    pub padding: Vec<f64>,
    pub border: Vec<f64>,
    pub margin: Vec<f64>,
    pub width: i32,
    pub height: i32,
}

/// Page.LayoutMetrics
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutMetrics {
    pub layout_viewport: LayoutViewport,
    pub visual_viewport: VisualViewport,
    pub content_size: ContentSize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutViewport {
    pub page_x: i32,
    pub page_y: i32,
    pub client_width: i32,
    pub client_height: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualViewport {
    pub offset_x: f64,
    pub offset_y: f64,
    pub page_x: f64,
    pub page_y: f64,
    pub client_width: f64,
    pub client_height: f64,
    pub scale: f64,
    pub zoom: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContentSize {
    pub width: f64,
    pub height: f64,
}
