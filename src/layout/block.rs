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
            // Position after previous block's border box
            let prev_border_box_end = prev.y + prev.border_box_height();

            // Collapse margins: use max of previous margin.bottom and current margin.top
            let margin_collapse = prev.margin.bottom.max(box_data.margin.top);
            box_data.y = prev_border_box_end + margin_collapse;

            // Update reserved_block_y
            *reserved_block_y = (*reserved_block_y).max(prev_border_box_end);
        } else {
            // First element - handle parent-child margin collapsing
            // context.collapsible_margin_top contains parent's margin that should collapse with this child
            let collapsed_margin = context.collapsible_margin_top.max(box_data.margin.top);
            box_data.y = y_base + collapsed_margin;
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
