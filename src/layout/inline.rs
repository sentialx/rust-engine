// Inline formatting context layout strategy

use crate::layout::flow::{FormattingContext, ReflowContext, InlineContext};
use crate::layout::boxes::LayoutBox;
use crate::html::NodeType;

/// Inline layout strategy - handles horizontal flow of inline elements
pub struct InlineLayoutStrategy;

impl InlineLayoutStrategy {
    /// Layout an inline element
    pub fn layout(
        &self,
        box_data: &mut LayoutBox,
        prev_box: Option<&LayoutBox>,
        inline_ctx: &mut InlineContext,
        context: &ReflowContext,
    ) -> (f32, f32) {
        let x_base = context.x;
        let y_base = context.y;
        
        if let Some(prev) = prev_box {
            let prev_is_inline = matches!(
                prev.formatting_context,
                FormattingContext::InlineContainer | FormattingContext::TextNode
            );
            
            if prev_is_inline {
                // Continue inline flow
                if inline_ctx.active {
                    box_data.x = inline_ctx.x;
                    box_data.y = inline_ctx.y;
                } else {
                    // Start new inline line
                    let prev_style = &prev.computed_style;
                    let next_x = prev.x + prev.margin_box_width();
                    inline_ctx.start_new_line(next_x, prev.y);
                    box_data.x = inline_ctx.x;
                    box_data.y = inline_ctx.y;
                }
            } else {
                // Block before inline - start new line
                inline_ctx.start_new_line(x_base, prev.y + prev.margin_box_height());
                box_data.x = inline_ctx.x;
                box_data.y = inline_ctx.y;
            }
        } else {
            // First inline element
            inline_ctx.start_new_line(x_base, y_base);
            box_data.x = inline_ctx.x;
            box_data.y = inline_ctx.y;
        }
        
        (box_data.x, box_data.y)
    }
    
    /// Update layout state after laying out an inline element
    pub fn update_state(
        &self,
        box_data: &LayoutBox,
        inline_ctx: &mut InlineContext,
        reserved_block_y: &mut f32,
        context: &mut ReflowContext,
    ) {
        // If this is a text node with wrapped lines, continue from the last line.
        let element = box_data.element.borrow();
        if element.node_type == NodeType::Text && !element.lines.is_empty() {
            let mut max_line_height: f32 = 0.0;
            for line in &element.lines {
                max_line_height = max_line_height.max(line.height);
            }

            let last_line = element.lines.last().unwrap();
            inline_ctx.x = last_line.x + last_line.width;
            inline_ctx.y = last_line.y;
            inline_ctx.line_height = inline_ctx.line_height.max(max_line_height);
            inline_ctx.active = true;
            *reserved_block_y = (*reserved_block_y).max(last_line.y + max_line_height);
            context.adjacent_margin_bottom = 0.0;
            return;
        }

        // Default inline continuation for non-text or unwrapped content
        inline_ctx.x = box_data.x + box_data.margin_box_width();
        inline_ctx.line_height = inline_ctx.line_height.max(box_data.margin_box_height());
        inline_ctx.active = true;
        *reserved_block_y = (*reserved_block_y).max(box_data.y + box_data.margin_box_height());
        context.adjacent_margin_bottom = 0.0;
    }
}
