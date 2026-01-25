// Formatting context and flow types

use crate::html::{DomElement, NodeType};
use crate::styles::ComputedStyle;

/// FormattingContext classifies elements to determine layout behavior.
/// This makes the layout algorithm extensible (flexbox, grid, etc. can be added).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FormattingContext {
    BlockContainer,  // Creates Block Formatting Context (BFC) - vertical stacking
    InlineContainer, // Creates Inline Formatting Context (IFC) - horizontal with line boxes
    TextNode,        // Text content (participates in IFC)
    FlexContainer,   // Flexbox formatting context (placeholder for future)
    GridContainer,   // Grid formatting context (placeholder for future)
}

/// Determines the formatting context for an element based on CSS properties
#[cfg(test)]
pub fn get_formatting_context(element: &DomElement) -> FormattingContext {
    get_formatting_context_impl(element)
}

#[cfg(not(test))]
pub fn get_formatting_context(element: &DomElement) -> FormattingContext {
    get_formatting_context_impl(element)
}

fn get_formatting_context_impl(element: &DomElement) -> FormattingContext {
    if element.computed_style.is_none() {
        return FormattingContext::BlockContainer;
    }
    
    let style = element.computed_style.as_ref().unwrap();
    
    // Text nodes participate in inline formatting
    if element.node_type == NodeType::Text {
        return FormattingContext::TextNode;
    }
    
    // Floated elements create block formatting context
    if style.float != "none" {
        return FormattingContext::BlockContainer;
    }
    
    // Determine based on display type
    match style.display.as_str() {
        "flex" => FormattingContext::FlexContainer,
        "inline-flex" => FormattingContext::FlexContainer, // TODO: inline-flex should be inline container with flex children
        "grid" => FormattingContext::GridContainer,
        "inline-grid" => FormattingContext::GridContainer, // TODO: inline-grid should be inline container with grid children
        "block" | "list-item" | "table" => FormattingContext::BlockContainer,
        "inline-block" | "inline-table" => FormattingContext::InlineContainer, // Flows horizontally but creates BFC for children
        "inline" => FormattingContext::InlineContainer,
        "none" => FormattingContext::BlockContainer, // Hidden, but still a container
        _ => FormattingContext::BlockContainer, // Default to block
    }
}

/// Tracks inline layout state during layout
#[derive(Clone, Debug)]
pub struct InlineContext {
    pub x: f32,
    pub y: f32,
    pub line_height: f32,
    pub active: bool,
}

impl InlineContext {
    pub fn new(x_base: f32, y_base: f32) -> Self {
        InlineContext {
            x: x_base,
            y: y_base,
            line_height: 0.0,
            active: false,
        }
    }

    pub fn start_new_line(&mut self, x_base: f32, y: f32) {
        self.x = x_base;
        self.y = y;
        self.line_height = 0.0;
        self.active = true;
    }

    pub fn continue_line(&mut self, prev_y: f32, prev_height: f32) {
        self.y = prev_y;
        self.line_height = prev_height;
        self.active = true;
    }

    pub fn break_context(&mut self) {
        self.active = false;
    }
}

/// Context passed through layout recursion
#[derive(Clone, Debug)]
pub struct ReflowContext {
    pub x: f32,
    pub y: f32,
    pub rel_x: f32,
    pub rel_y: f32,
    pub font_size: f32,
    pub parent_width: f32,
    pub parent_height: f32,
    pub parent_max_width: f32,
    pub layout_x_start: Option<f32>,
    pub adjacent_margin_bottom: f32,
    pub shrink_to_fit: bool,
}

/// Helper to check if element uses absolute positioning
pub fn uses_absolute_positioning(computed_style: &ComputedStyle) -> bool {
    computed_style.position == "absolute"
        || computed_style.position == "fixed" 
        || computed_style.position == "sticky"
}
