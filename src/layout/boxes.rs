// Layout box model - Intermediate representation for layout computation

use crate::dom::DomElement;
use crate::styles::{ComputedStyle, ComputedMargin};
use crate::layout::flow::FormattingContext;
use std::rc::Rc;
use std::cell::{Ref, RefCell};

/// Layout box represents an element's layout properties and geometry
/// This is an intermediate representation used during layout computation
#[derive(Clone, Debug)]
pub struct LayoutBox {
    /// Reference to the DOM element this box represents
    pub element: Rc<RefCell<DomElement>>,

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

    /// Margins (working copy, may be modified during layout)
    pub margin: ComputedMargin,

    /// Padding (working copy, may be modified during layout)
    pub padding: ComputedMargin,
}

impl LayoutBox {
    pub fn new(element: Rc<RefCell<DomElement>>, formatting_context: FormattingContext) -> Self {
        let (margin, padding) = {
            let el = element.borrow();
            let style = el.computed_style.as_ref().expect("computed_style must be set before layout");
            (style.margin.clone(), style.padding.clone())
        };
        LayoutBox {
            element,
            formatting_context,
            intrinsic_width: None,
            intrinsic_height: None,
            intrinsic_min_width: None,
            content_width: 0.0,
            content_height: 0.0,
            x: 0.0,
            y: 0.0,
            margin,
            padding,
        }
    }

    /// Access the computed style from the DOM element.
    /// This always returns the current style, so style updates are automatically reflected.
    pub fn computed_style(&self) -> Ref<'_, ComputedStyle> {
        Ref::map(self.element.borrow(), |el| {
            el.computed_style.as_ref().expect("computed_style must be set")
        })
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

    /// Refresh margin and padding from the current computed style.
    /// Call this when styles have changed but tree structure hasn't.
    pub fn refresh_box_model(&mut self) {
        let (margin, padding) = {
            let style = self.computed_style();
            (style.margin.clone(), style.padding.clone())
        };
        self.margin = margin;
        self.padding = padding;
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

    /// Recursively refresh box model (margin/padding) from current computed styles.
    /// Call this when styles have changed but tree structure hasn't.
    pub fn refresh_styles(&mut self) {
        self.box_data.refresh_box_model();
        for child in &mut self.children {
            child.refresh_styles();
        }
    }
}
