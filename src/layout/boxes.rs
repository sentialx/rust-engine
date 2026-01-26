// Layout box model - Intermediate representation for layout computation

use crate::html::DomElement;
use crate::styles::{ComputedStyle, ComputedMargin};
use crate::layout::flow::FormattingContext;
use std::rc::Rc;
use std::cell::RefCell;

/// Layout box represents an element's layout properties and geometry
/// This is an intermediate representation used during layout computation
#[derive(Clone, Debug)]
pub struct LayoutBox {
    /// Reference to the DOM element this box represents
    pub element: Rc<RefCell<DomElement>>,
    
    /// Computed style for this box
    pub computed_style: ComputedStyle,
    
    /// Formatting context this box participates in
    pub formatting_context: FormattingContext,
    
    /// Intrinsic dimensions (before layout)
    pub intrinsic_width: Option<f32>,
    pub intrinsic_height: Option<f32>,
    pub intrinsic_min_width: Option<f32>,
    
    /// Layout dimensions (after layout)
    pub content_width: f32,
    pub content_height: f32,
    
    /// Position (relative to containing block)
    pub x: f32,
    pub y: f32,
    
    /// Margins (already computed)
    pub margin: ComputedMargin,
    
    /// Padding (already computed)
    pub padding: ComputedMargin,
}

impl LayoutBox {
    pub fn new(element: Rc<RefCell<DomElement>>, computed_style: ComputedStyle, formatting_context: FormattingContext) -> Self {
        LayoutBox {
            element,
            computed_style: computed_style.clone(),
            formatting_context,
            intrinsic_width: None,
            intrinsic_height: None,
            intrinsic_min_width: None,
            content_width: 0.0,
            content_height: 0.0,
            x: 0.0,
            y: 0.0,
            margin: computed_style.margin.clone(),
            padding: computed_style.padding.clone(),
        }
    }
    
    /// Get the margin box width (content + padding + margin)
    pub fn margin_box_width(&self) -> f32 {
        self.content_width + self.padding.left + self.padding.right + self.margin.left + self.margin.right
    }
    
    /// Get the margin box height (content + padding + margin)
    pub fn margin_box_height(&self) -> f32 {
        self.content_height + self.padding.top + self.padding.bottom + self.margin.top + self.margin.bottom
    }
    
    /// Get the border box width (content + padding)
    pub fn border_box_width(&self) -> f32 {
        self.content_width + self.padding.left + self.padding.right
    }
    
    /// Get the border box height (content + padding)
    pub fn border_box_height(&self) -> f32 {
        self.content_height + self.padding.top + self.padding.bottom
    }
}

/// Layout node represents a tree of layout boxes
#[derive(Clone, Debug)]
pub struct LayoutNode {
    pub box_data: LayoutBox,
    pub children: Vec<LayoutNode>,
}

impl LayoutNode {
    pub fn new(box_data: LayoutBox) -> Self {
        LayoutNode {
            box_data,
            children: Vec::new(),
        }
    }
}
