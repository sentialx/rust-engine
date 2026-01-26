// Block formatting context layout strategy

use crate::layout::flow::{ReflowContext, FormattingContext, PrevBlockData, uses_absolute_positioning};
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

/// Build child context for a block element's children
pub fn build_block_child_context(
    node: &LayoutNode,
    context: &ReflowContext,
    content_x: f32,
    content_y: f32,
    available_content_width: f32,
    collapsible_margin: f32,
) -> ReflowContext {
    let parent_content_width = if node.box_data.content_width > 0.0 {
        node.box_data.content_width
    } else {
        available_content_width
    };

    let rel_x_base = node.box_data.x + node.box_data.margin.left;
    let rel_y_base = node.box_data.y;

    ReflowContext {
        x: content_x,
        y: content_y,
        rel_x: if node.box_data.computed_style.position == "relative"
            || uses_absolute_positioning(&node.box_data.computed_style)
        {
            rel_x_base
        } else {
            context.rel_x
        },
        rel_y: if node.box_data.computed_style.position == "relative"
            || uses_absolute_positioning(&node.box_data.computed_style)
        {
            rel_y_base
        } else {
            context.rel_y
        },
        font_size: node.box_data.computed_style.font_size,
        parent_width: node.box_data.content_width,
        parent_height: node.box_data.content_height,
        parent_max_width: parent_content_width,
        layout_x_start: None,
        adjacent_margin_bottom: 0.0,
        shrink_to_fit: false,
        collapsible_margin_top: collapsible_margin,
    }
}

/// Finalize block dimensions after children are laid out
pub fn finalize_block_dimensions(
    node: &mut LayoutNode,
    content_y: f32,
    available_content_width: f32,
) {
    let width_has_value = {
        let element = node.box_data.element.borrow();
        let inherited_style = element.inherited_style.as_ref().unwrap();
        inherited_style.width.has_numeric_value()
    };
    let height_has_value = {
        let element = node.box_data.element.borrow();
        let inherited_style = element.inherited_style.as_ref().unwrap();
        inherited_style.height.has_numeric_value()
    };

    if !width_has_value {
        // Block auto width is clamped to available width
        node.box_data.content_width = node.box_data.content_width.min(available_content_width);
    }

    if !height_has_value {
        node.box_data.content_height = compute_block_height_from_children(&node.children, content_y);
    }
}

impl BlockLayoutStrategy {
    /// Layout a block element and update state
    pub fn layout_and_update(
        &self,
        box_data: &mut LayoutBox,
        prev_block: Option<PrevBlockData>,
        reserved_block_y: &mut f32,
        context: &ReflowContext,
    ) {
        let x_base = context.x;
        let y_base = context.y;

        if let Some(prev) = prev_block {
            let prev_border_box_end = prev.y + prev.border_box_height;
            let margin_collapse = prev.margin_bottom.max(box_data.margin.top);
            box_data.y = prev_border_box_end + margin_collapse;
            *reserved_block_y = (*reserved_block_y).max(prev_border_box_end);
        } else {
            let y_start = (*reserved_block_y).max(y_base);
            let collapsed_margin = context.collapsible_margin_top.max(box_data.margin.top);
            box_data.y = y_start + collapsed_margin;
        }

        box_data.x = x_base;
        *reserved_block_y = (*reserved_block_y).max(box_data.y + box_data.margin_box_height());
    }
}

/// Layout children of a block element
pub fn layout_block_children(
    node: &mut LayoutNode,
    context: &ReflowContext,
    layout_fn: &mut dyn FnMut(&mut Vec<LayoutNode>, &ReflowContext),
) {
    let content_x = node.box_data.x + node.box_data.margin.left + node.box_data.padding.left;
    let available_width = (context.parent_max_width - (content_x - context.x)).max(0.0);

    // Compute auto width for blocks (even childless ones)
    if let Some(auto_width) = compute_block_auto_width(&node.box_data, context) {
        node.box_data.content_width = auto_width;
    }

    if node.children.is_empty() {
        return;
    }

    let (content_y, collapsible_margin, _) = compute_block_content_y(&node.box_data);
    let child_context = build_block_child_context(node, context, content_x, content_y, available_width, collapsible_margin);

    layout_fn(&mut node.children, &child_context);

    finalize_block_dimensions(node, content_y, available_width);
}
