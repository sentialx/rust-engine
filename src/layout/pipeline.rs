// Layout pipeline - Build, Measure, Layout, Finalize passes
//
// This module orchestrates the layout process through four passes:
// 1. Build: Convert DOM tree to layout box tree
// 2. Measure: Preprocess text and compute intrinsic sizes
// 3. Layout: Position elements using formatting context strategies
// 4. Finalize: Copy results back to DOM
//
// Display-specific logic is delegated to:
// - block.rs: Block formatting context
// - inline.rs: Inline formatting context

use crate::html::{DomElement, NodeType, ComputedFlow};
use crate::render_frame::TextMeasurer;
use crate::styles::ScalarEvaluationContext;
use crate::layout::{Rect, boxes::{LayoutBox, LayoutNode}};
use crate::layout::flow::{FormattingContext, get_formatting_context, ReflowContext, InlineContext, uses_absolute_positioning};
use crate::layout::block::{BlockLayoutStrategy, layout_block_children};
use crate::layout::inline::{preprocess_text_node, compute_text_intrinsics, compute_container_intrinsics, layout_inline_content, advance_past_atomic_inline, is_atomic_inline_display, layout_inline_children};
use crate::layout::abspos::AbsoluteLayoutStrategy;
use std::rc::Rc;
use std::cell::RefCell;

/// Build pass: Convert DOM tree to layout box tree
/// This isolates style evaluation and computed style extraction
pub fn build_layout_tree(
    tree: &mut Vec<Rc<RefCell<DomElement>>>,
    context: &ReflowContext,
) -> Vec<LayoutNode> {
    let mut layout_nodes = Vec::new();

    for element_rc in tree {
        // First, evaluate styles and compute computed_style
        // We need to do this in steps to avoid borrow conflicts
        let (computed_style, formatting_context, has_children, child_context_opt) = {
            // Step 1: Evaluate styles (mutable borrow)
            let (computed_style, width_has_value, height_has_value, width_val, height_val) = {
                let mut element = element_rc.borrow_mut();

                // Skip script and style elements
                if element.tag_name == "SCRIPT" || element.tag_name == "STYLE" {
                    continue;
                }

                // Evaluate styles if needed
                let inherited_style = match &mut element.inherited_style {
                    Some(style) => style,
                    None => continue,
                };

                // Skip if display: none
                if inherited_style.display.get() == "none" {
                    continue;
                }

                // Evaluate scalar values
                let font_scalar_ctx = ScalarEvaluationContext::from_parent(
                    context.font_size,
                    context.font_size,
                );
                let parent_width_scalar_ctx = ScalarEvaluationContext::from_parent(
                    context.font_size,
                    context.parent_width,
                );
                let parent_height_scalar_ctx = ScalarEvaluationContext::from_parent(
                    context.font_size,
                    context.parent_height,
                );

                // Evaluate style properties
                inherited_style.margin.evaluate(&font_scalar_ctx);
                inherited_style.padding.evaluate(&font_scalar_ctx);
                inherited_style.font_size.evaluate(&font_scalar_ctx);
                inherited_style.inset.evaluate(&font_scalar_ctx);
                inherited_style.width.evaluate(&parent_width_scalar_ctx);
                inherited_style.height.evaluate(&parent_height_scalar_ctx);

                // Get computed style and width/height info
                let computed_style = inherited_style.to_computed_style();
                let width_has_value = inherited_style.width.has_numeric_value();
                let height_has_value = inherited_style.height.has_numeric_value();
                let width_val = if width_has_value { inherited_style.width.get() } else { 0.0 };
                let height_val = if height_has_value { inherited_style.height.get() } else { 0.0 };

                // Store computed style
                element.computed_style = Some(computed_style.clone());

                (computed_style, width_has_value, height_has_value, width_val, height_val)
            };

            // Step 2: Get formatting context and check children (immutable borrow)
            let (formatting_context, has_children) = {
                let element = element_rc.borrow();
                let formatting_context = get_formatting_context(&element);
                let has_children = element.children.len() > 0;
                (formatting_context, has_children)
            };

            // Step 3: Prepare child context if needed
            let child_context_opt = if has_children {
                let shrink_to_fit = context.shrink_to_fit
                    || matches!(
                        computed_style.display.as_str(),
                        "inline" | "inline-block" | "inline-table" | "inline-flex" | "inline-grid"
                    );
                Some(ReflowContext {
                    x: context.x,
                    y: context.y,
                    rel_x: context.rel_x,
                    rel_y: context.rel_y,
                    font_size: computed_style.font_size,
                    parent_width: if width_has_value { width_val } else { context.parent_width },
                    parent_height: if height_has_value { height_val } else { context.parent_height },
                    parent_max_width: if width_has_value { width_val } else { context.parent_max_width },
                    layout_x_start: context.layout_x_start,
                    adjacent_margin_bottom: context.adjacent_margin_bottom,
                    shrink_to_fit,
                    collapsible_margin_top: context.collapsible_margin_top,
                })
            } else {
                None
            };

            (computed_style, formatting_context, has_children, child_context_opt)
        };

        // Create layout box
        let box_data = LayoutBox::new(
            element_rc.clone(),
            computed_style,
            formatting_context,
        );

        let mut layout_node = LayoutNode::new(box_data);

        // Recursively build children
        if has_children {
            let child_context = child_context_opt.unwrap();
            let mut children = element_rc.borrow_mut().children.clone();
            layout_node.children = build_layout_tree(&mut children, &child_context);
            // Update the original children with any mutations
            element_rc.borrow_mut().children = children;
        }

        layout_nodes.push(layout_node);
    }

    layout_nodes
}

/// Measure pass: Compute intrinsic dimensions and preprocess text
pub fn measure_layout_tree(
    layout_nodes: &mut Vec<LayoutNode>,
    text_measurer: &mut dyn TextMeasurer,
    max_width: f32,
) {
    for node in layout_nodes.iter_mut() {
        let comp_style = &node.box_data.computed_style;

        // For text nodes, preprocess and compute intrinsics
        let (node_type, node_value_empty) = {
            let element = node.box_data.element.borrow();
            (element.node_type.clone(), element.node_value.is_empty())
        };

        if node_type == NodeType::Text && !node_value_empty {
            let font_size = comp_style.font_size;
            let font_path = {
                let element = node.box_data.element.borrow();
                element.inherited_style.as_ref().unwrap().font.get_path()
            };

            {
                let mut element = node.box_data.element.borrow_mut();
                preprocess_text_node(&mut element, text_measurer, font_size, &font_path);
            }
            compute_text_intrinsics(node);
        } else {
            // Non-text: use specified dimensions if available
            if comp_style.width > 0.0 {
                node.box_data.content_width = comp_style.width;
            }
            if comp_style.height > 0.0 {
                node.box_data.content_height = comp_style.height;
            }
        }

        // Measure children and compute container intrinsics
        if !node.children.is_empty() {
            let child_max_width = if node.box_data.computed_style.width > 0.0 {
                node.box_data.content_width - node.box_data.padding.left - node.box_data.padding.right
            } else {
                max_width
            };
            measure_layout_tree(&mut node.children, text_measurer, child_max_width);

            if node.box_data.formatting_context != FormattingContext::TextNode {
                compute_container_intrinsics(node);
            }
        }
    }
}


/// Layout pass: Assign positions and sizes using formatting context strategies
/// Text segments and inline elements are positioned by the unified inline layout algorithm
pub fn layout_tree(
    layout_nodes: &mut Vec<LayoutNode>,
    context: &ReflowContext,
) -> (f32, f32) { // Returns (reserved_block_y, adjacent_margin_bottom)
    let block_strategy = BlockLayoutStrategy;
    let abspos_strategy = AbsoluteLayoutStrategy;

    let mut reserved_block_y = context.y;
    let line_start_x = context.layout_x_start.unwrap_or(context.x);
    let mut inline_ctx = InlineContext::new(line_start_x, context.y, context.parent_max_width);
    inline_ctx.active = true;
    let mut adjacent_margin_bottom = 0.0;
    let mut prev_node_opt: Option<&LayoutNode> = None;

    for node in layout_nodes.iter_mut() {
        let formatting_context = node.box_data.formatting_context;
        let mut is_atomic_inline = false; // Track if this is an inline-block that needs ctx update after children

        // Skip if display: none
        if node.box_data.computed_style.display == "none" {
            continue;
        }

        // Layout based on formatting context
        if uses_absolute_positioning(&node.box_data.computed_style) {
            abspos_strategy.layout(&mut node.box_data, context);
        } else {
            match formatting_context {
                FormattingContext::BlockContainer => {
                    // Flush any pending inline content
                    if inline_ctx.x > inline_ctx.line_start_x {
                        reserved_block_y = reserved_block_y.max(inline_ctx.y + inline_ctx.line_height);
                    }

                    let prev_box = prev_node_opt.map(|n| &n.box_data);
                    adjacent_margin_bottom = block_strategy.layout_and_update(
                        &mut node.box_data,
                        prev_box,
                        &mut reserved_block_y,
                        context,
                    );

                    // Reset inline context after block
                    inline_ctx = InlineContext::new(line_start_x, reserved_block_y, context.parent_max_width);
                    inline_ctx.active = true;
                }
                FormattingContext::InlineContainer | FormattingContext::TextNode => {
                    // Check if this is an atomic inline (inline-block) that needs ctx update after children
                    is_atomic_inline = formatting_context == FormattingContext::InlineContainer
                        && is_atomic_inline_display(&node.box_data.computed_style.display);

                    // Use unified inline layout - handles text segments and inline elements
                    let mut single_node = std::slice::from_mut(node);
                    layout_inline_content(single_node, &mut inline_ctx);

                    // Update reserved_block_y to account for inline content
                    // Note: For atomic inlines, this will be updated again after children layout
                    reserved_block_y = reserved_block_y.max(inline_ctx.y + inline_ctx.line_height);
                    adjacent_margin_bottom = 0.0;
                }
                FormattingContext::FlexContainer | FormattingContext::GridContainer => {
                    // TODO: Implement flex/grid - treat as block for now
                    let prev_box = prev_node_opt.map(|n| &n.box_data);
                    adjacent_margin_bottom = block_strategy.layout_and_update(
                        &mut node.box_data,
                        prev_box,
                        &mut reserved_block_y,
                        context,
                    );
                }
            }
        }

        // Layout children based on formatting context
        let is_block = matches!(node.box_data.formatting_context, FormattingContext::BlockContainer);
        if is_block {
            layout_block_children(node, context, &mut |children, ctx| { layout_tree(children, ctx); });
        } else {
            layout_inline_children(node, context, &mut |children, ctx| { layout_tree(children, ctx); });
        }

        // After children are laid out, update inline context with actual dimensions
        // for atomic inline boxes (inline-block, etc.)
        if is_atomic_inline {
            advance_past_atomic_inline(node, &mut inline_ctx);
            reserved_block_y = reserved_block_y.max(inline_ctx.y + inline_ctx.line_height);
        }

        // Only track block elements as prev_node for block stacking
        if formatting_context == FormattingContext::BlockContainer {
            prev_node_opt = Some(node);
        }
    }

    (reserved_block_y, adjacent_margin_bottom)
}

/// Finalize pass: Copy layout results back to DOM ComputedFlow
pub fn finalize_layout_tree(
    layout_nodes: &Vec<LayoutNode>,
) {
    for node in layout_nodes {
        let mut element = node.box_data.element.borrow_mut();

        // Create ComputedFlow from layout box

        let flow_y = if node.box_data.formatting_context == FormattingContext::BlockContainer {
            node.box_data.y
        } else {
            node.box_data.y + node.box_data.margin.top
        };
        element.computed_flow = Some(ComputedFlow {
            x: node.box_data.x + node.box_data.margin.left,
            y: flow_y,
            width: node.box_data.border_box_width(),
            height: node.box_data.border_box_height(),
            continue_x: node.box_data.x + node.box_data.margin_box_width(),
            continue_y: node.box_data.y + node.box_data.margin_box_height(),
            adjacent_margin_bottom: node.box_data.margin.bottom,
            hover_rect: Rect {
                x: node.box_data.x,
                y: node.box_data.y,
                width: node.box_data.margin_box_width(),
                height: node.box_data.margin_box_height(),
            },
        });

        drop(element);

        // Finalize children
        if !node.children.is_empty() {
            finalize_layout_tree(&node.children);
        }
    }
}

/// Reset layout positions for re-layout (used when reusing cached tree)
/// Keeps intrinsic measurements, resets computed positions
pub fn reset_layout_positions(layout_nodes: &mut Vec<LayoutNode>) {
    for node in layout_nodes.iter_mut() {
        // Reset position and computed dimensions
        node.box_data.x = 0.0;
        node.box_data.y = 0.0;
        node.box_data.content_width = node.box_data.computed_style.width.max(0.0);
        node.box_data.content_height = node.box_data.computed_style.height.max(0.0);

        // Reset text segment positions (keep measurements)
        {
            let mut element = node.box_data.element.borrow_mut();
            for seg in &mut element.text_segments {
                seg.x = 0.0;
                seg.y = 0.0;
            }
        }

        // Recursively reset children
        if !node.children.is_empty() {
            reset_layout_positions(&mut node.children);
        }
    }
}
