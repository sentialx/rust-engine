// Block formatting context layout strategy

use crate::layout::flow::{ReflowContext, FormattingContext};
use crate::layout::boxes::{LayoutBox, LayoutNode};

/// Block layout strategy - handles vertical stacking of block elements
pub struct BlockLayoutStrategy;

/// Check if display value is a block-level display
#[inline]
pub fn is_block_level_display(display: &str) -> bool {
    matches!(display, "block" | "list-item" | "table")
}

/// Compute auto width for block elements
/// Returns the computed width, or None if width is not auto
pub fn compute_block_auto_width(
    box_data: &LayoutBox,
    context: &ReflowContext,
) -> Option<f32> {
    // Check if width is auto
    let width_is_auto = {
        let element = box_data.element.borrow();
        let inherited_style = element.inherited_style.as_ref()?;
        !inherited_style.width.has_numeric_value()
    };

    if !width_is_auto {
        return None;
    }

    if !is_block_level_display(&box_data.computed_style.display) {
        return None;
    }

    if context.shrink_to_fit {
        return None;
    }

    let auto_width = (context.parent_max_width
        - box_data.margin.left
        - box_data.margin.right
        - box_data.padding.left
        - box_data.padding.right)
        .max(0.0);

    Some(auto_width)
}

/// Compute content_y for a block element, handling margin collapsing
pub fn compute_block_content_y(box_data: &LayoutBox) -> (f32, f32, bool) {
    // Margin collapsing: when a block has no padding.top, its margin collapses
    // with the first child's margin
    let can_collapse_margin = box_data.padding.top == 0.0;
    let collapsible_margin = if can_collapse_margin {
        box_data.margin.top
    } else {
        0.0
    };

    let content_y = if can_collapse_margin {
        // If margin can collapse, subtract the parent's margin from y
        // so the child's margin starts from the correct position
        box_data.y + box_data.padding.top - box_data.margin.top
    } else {
        box_data.y + box_data.padding.top
    };

    (content_y, collapsible_margin, can_collapse_margin)
}

/// Compute block height from children
pub fn compute_block_height_from_children(
    children: &[LayoutNode],
    content_y: f32,
) -> f32 {
    let mut height = 0.0_f32;

    for child in children {
        let child_is_block = matches!(
            child.box_data.formatting_context,
            FormattingContext::BlockContainer
        );

        let child_bottom = if child_is_block {
            // Block: y is border box, add border_box_height + margin.bottom
            child.box_data.y + child.box_data.border_box_height() + child.box_data.margin.bottom
        } else {
            // Inline: y is margin box top, margin_box_height gives full height
            child.box_data.y + child.box_data.margin_box_height()
        };

        height = height.max(child_bottom - content_y);
    }

    height
}

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
            // First block element - use reserved_block_y which accounts for any preceding inline content
            // (e.g., when a <div> follows a <strong> inside the same parent)
            let y_start = (*reserved_block_y).max(y_base);

            // Handle parent-child margin collapsing
            // context.collapsible_margin_top contains parent's margin that should collapse with this child
            let collapsed_margin = context.collapsible_margin_top.max(box_data.margin.top);
            box_data.y = y_start + collapsed_margin;
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
