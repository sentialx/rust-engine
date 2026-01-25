// Block formatting context layout strategy

use crate::layout::flow::ReflowContext;
use crate::layout::boxes::LayoutBox;

/// Block layout strategy - handles vertical stacking of block elements
pub struct BlockLayoutStrategy;

impl BlockLayoutStrategy {
    /// Layout a block element
    pub fn layout(
        &self,
        box_data: &mut LayoutBox,
        prev_box: Option<&LayoutBox>,
        reserved_block_y: &mut f32,
        context: &ReflowContext,
        previous_margin_bottom: f32,
    ) -> (f32, f32) {
        let x_base = context.x;
        let y_base = context.y;
        
        if let Some(prev) = prev_box {
            // Position after previous block
            *reserved_block_y = (*reserved_block_y).max(prev.y + prev.margin_box_height());
            
            // Collapse margins: use max of previous margin.bottom and current margin.top
            let margin_collapse = prev.margin.bottom.max(box_data.margin.top);
            box_data.y = *reserved_block_y + margin_collapse;
        } else {
            // First element
            let margin_top = box_data.margin.top.max(0.0) - previous_margin_bottom.max(0.0);
            box_data.y = y_base + margin_top.max(0.0);
        }
        
        box_data.x = x_base;
        
        // Update reserved_block_y for next element
        *reserved_block_y = (*reserved_block_y).max(
            box_data.y + box_data.margin_box_height()
        );
        
        (box_data.x, box_data.y)
    }
    
    /// Update layout state after laying out a block
    pub fn update_state(
        &self,
        box_data: &LayoutBox,
        reserved_block_y: &mut f32,
        context: &mut ReflowContext,
        adjacent_margin_bottom: f32,
    ) {
        *reserved_block_y = (*reserved_block_y).max(
            box_data.y + box_data.margin_box_height()
        );
        context.adjacent_margin_bottom = adjacent_margin_bottom;
    }
}
