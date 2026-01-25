// Absolute positioning layout strategy

use crate::layout::flow::ReflowContext;
use crate::layout::boxes::LayoutBox;

/// Absolute positioning strategy - handles absolute/fixed/sticky positioning
pub struct AbsoluteLayoutStrategy;

impl AbsoluteLayoutStrategy {
    /// Layout an absolutely positioned element
    pub fn layout(
        &self,
        box_data: &mut LayoutBox,
        context: &ReflowContext,
    ) -> (f32, f32) {
        // For absolute positioning, use inset values
        let element = box_data.element.borrow();
        let style = element.inherited_style.as_ref().unwrap();
        
        let mut x = context.x;
        let mut y = context.y;
        
        if style.inset.top.has_numeric_value() {
            y = style.inset.top.get() + context.rel_y;
        }
        if style.inset.left.has_numeric_value() {
            x = style.inset.left.get() + context.rel_x;
        }
        
        box_data.x = x;
        box_data.y = y;
        
        (x, y)
    }
}
