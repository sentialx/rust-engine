// Layout pipeline - Build, Measure, Layout, Finalize passes

use crate::html::{DomElement, NodeType, ComputedFlow, TextSegment};
use crate::render_frame::TextMeasurer;
use crate::styles::ScalarEvaluationContext;
use crate::layout::{Rect, boxes::{LayoutBox, LayoutNode}};
use crate::layout::flow::{FormattingContext, get_formatting_context, ReflowContext, InlineContext};
use std::rc::Rc;
use std::cell::RefCell;

/// Preprocess text node: split into words and measure each
/// Caches measurements - only re-measures if font_size changed or segments are empty
fn preprocess_text_node(
    element: &mut DomElement,
    text_measurer: &mut dyn TextMeasurer,
    font_size: f32,
    font_path: &str,
) {
    // Check if we can reuse existing measurements
    // Segments are valid if they exist, have same font size, and text hasn't changed
    let can_reuse = !element.text_segments.is_empty()
        && element.cached_font_size == Some(font_size)
        && element.cached_font_path.as_deref() == Some(font_path);

    if can_reuse {
        // Just reset positions, keep measurements
        for seg in &mut element.text_segments {
            seg.x = 0.0;
            seg.y = 0.0;
        }
        return;
    }

    // Need to re-measure
    element.text_segments.clear();
    element.cached_font_size = Some(font_size);
    element.cached_font_path = Some(font_path.to_string());

    // Handle HTML entities
    let processed_value = element.node_value
        .replace("&nbsp;", " ")
        .replace("&gt;", ">")
        .replace("&lt;", "<");

    // Measure space width
    let (space_w, _) = text_measurer.measure(" ", font_size, font_path);
    element.space_width = space_w;

    // Get ascent for this font (distance from top to baseline)
    let ascent = text_measurer.ascent(font_size, font_path);

    // Split into words and measure each
    for word in processed_value.split_whitespace() {
        let (w, h) = text_measurer.measure(word, font_size, font_path);
        element.text_segments.push(TextSegment {
            text: word.to_string(),
            width: w,
            height: h,
            ascent,
            x: 0.0,  // Set during layout
            y: 0.0,
        });
    }

    // Store the processed value back
    element.node_value = processed_value;
}

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
/// Text nodes are split into measured segments here
pub fn measure_layout_tree(
    layout_nodes: &mut Vec<LayoutNode>,
    text_measurer: &mut dyn TextMeasurer,
    _max_width: f32,
) {
    for node in layout_nodes.iter_mut() {
        let comp_style = &node.box_data.computed_style;

        // For text nodes, preprocess into segments
        let (node_type, node_value_empty) = {
            let element = node.box_data.element.borrow();
            (element.node_type.clone(), element.node_value.is_empty())
        };

        if node_type == NodeType::Text && !node_value_empty {
            let font_size = comp_style.font_size;
            let font_path = {
                let element = node.box_data.element.borrow();
                let inherited_style = element.inherited_style.as_ref().unwrap();
                inherited_style.font.get_path()
            };

            // Preprocess text into segments
            {
                let mut element = node.box_data.element.borrow_mut();
                preprocess_text_node(&mut element, text_measurer, font_size, &font_path);
            }

            // Calculate intrinsic dimensions from segments
            let element = node.box_data.element.borrow();
            if !element.text_segments.is_empty() {
                // Intrinsic width is the sum of all segments + spaces (single line)
                let total_width: f32 = element.text_segments.iter().map(|s| s.width).sum::<f32>()
                    + element.space_width * (element.text_segments.len().saturating_sub(1) as f32);
                let max_height: f32 = element.text_segments.iter().map(|s| s.height).fold(0.0, f32::max);

                // Min-content width is the widest single word
                let min_width: f32 = element.text_segments.iter().map(|s| s.width).fold(0.0, f32::max);

                node.box_data.intrinsic_width = Some(total_width);
                node.box_data.intrinsic_height = Some(max_height);
                node.box_data.intrinsic_min_width = Some(min_width);
            }
        } else {
            // For non-text elements, use specified width/height if available
            if comp_style.width > 0.0 {
                node.box_data.content_width = comp_style.width;
            }
            if comp_style.height > 0.0 {
                node.box_data.content_height = comp_style.height;
            }
        }

        // Measure children first (for width/height calculation)
        if !node.children.is_empty() {
            let child_max_width = if node.box_data.computed_style.width > 0.0 {
                node.box_data.content_width - node.box_data.padding.left - node.box_data.padding.right
            } else {
                _max_width
            };
            measure_layout_tree(&mut node.children, text_measurer, child_max_width);
            if node.box_data.formatting_context != FormattingContext::TextNode {
                let mut min_width: f32 = 0.0;
                let mut max_width: f32 = 0.0;

                // Track inline children that are on the same "line" for min-content calculation
                let mut inline_line_min: f32 = 0.0;
                let mut inline_line_max: f32 = 0.0;

                for child in &node.children {
                    let child_padding = child.box_data.padding.left + child.box_data.padding.right;
                    let child_margin = child.box_data.margin.left + child.box_data.margin.right;
                    let child_non_content = child_padding + child_margin;

                    let is_inline_level = matches!(
                        child.box_data.computed_style.display.as_str(),
                        "inline" | "inline-block" | "inline-table" | "inline-flex" | "inline-grid"
                    ) || child.box_data.formatting_context == FormattingContext::TextNode;

                    if is_inline_level {
                        // Inline children on the same line - sum their widths
                        if let Some(child_min) = child.box_data.intrinsic_min_width {
                            inline_line_min += child_min + child_non_content;
                        }
                        if let Some(child_max) = child.box_data.intrinsic_width {
                            inline_line_max += child_max + child_non_content;
                        }
                    } else {
                        // Block child - flush any accumulated inline content first
                        if inline_line_min > 0.0 {
                            min_width = min_width.max(inline_line_min);
                            inline_line_min = 0.0;
                        }
                        if inline_line_max > 0.0 {
                            max_width = max_width.max(inline_line_max);
                            inline_line_max = 0.0;
                        }

                        // Block children - take max
                        if let Some(child_min) = child.box_data.intrinsic_min_width {
                            min_width = min_width.max(child_min + child_padding);
                        }
                        if let Some(child_max) = child.box_data.intrinsic_width {
                            max_width = max_width.max(child_max + child_padding);
                        }
                    }
                }

                // Flush any remaining inline content
                if inline_line_min > 0.0 {
                    min_width = min_width.max(inline_line_min);
                }
                if inline_line_max > 0.0 {
                    max_width = max_width.max(inline_line_max);
                }

                if min_width > 0.0 {
                    node.box_data.intrinsic_min_width = Some(min_width);
                }
                if max_width > 0.0 {
                    node.box_data.intrinsic_width = Some(max_width);
                }
            }
        }
    }
}

fn max_intrinsic_text_width(node: &LayoutNode) -> f32 {
    if node.box_data.formatting_context == FormattingContext::TextNode {
        return node.box_data.intrinsic_width.unwrap_or(0.0);
    }

    node.children
        .iter()
        .map(max_intrinsic_text_width)
        .fold(0.0_f32, |acc, w| acc.max(w))
}

fn min_intrinsic_text_width(node: &LayoutNode) -> f32 {
    if node.box_data.formatting_context == FormattingContext::TextNode {
        return node.box_data.intrinsic_min_width.unwrap_or(0.0);
    }

    node.children
        .iter()
        .map(min_intrinsic_text_width)
        .fold(0.0_f32, |acc, w| acc.max(w))
}

fn has_breakable_text(element: &DomElement) -> bool {
    if element.node_type == NodeType::Text {
        return element.text_segments.len() > 1;
    }

    for child in &element.children {
        if has_breakable_text(&child.borrow()) {
            return true;
        }
    }

    false
}

/// Layout pass: Assign positions and sizes using formatting context strategies
/// Text segments and inline elements are positioned by the unified inline layout algorithm
pub fn layout_tree(
    layout_nodes: &mut Vec<LayoutNode>,
    context: &ReflowContext,
) -> (f32, f32) { // Returns (reserved_block_y, adjacent_margin_bottom)
    use crate::layout::block::BlockLayoutStrategy;
    use crate::layout::inline::{layout_inline_content, advance_past_atomic_inline};
    use crate::layout::abspos::AbsoluteLayoutStrategy;
    use crate::layout::flow::uses_absolute_positioning;

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
                    let (_x, _y) = block_strategy.layout(
                        &mut node.box_data,
                        prev_box,
                        &mut reserved_block_y,
                        context,
                        adjacent_margin_bottom,
                    );
                    let mut update_context = ReflowContext {
                        x: context.x,
                        y: context.y,
                        rel_x: context.rel_x,
                        rel_y: context.rel_y,
                        font_size: context.font_size,
                        parent_width: context.parent_width,
                        parent_height: context.parent_height,
                        parent_max_width: context.parent_max_width,
                        layout_x_start: context.layout_x_start,
                        adjacent_margin_bottom,
                        shrink_to_fit: context.shrink_to_fit,
                        collapsible_margin_top: 0.0,
                    };
                    block_strategy.update_state(
                        &node.box_data,
                        &mut reserved_block_y,
                        &mut update_context,
                        node.box_data.margin.bottom,
                    );
                    adjacent_margin_bottom = node.box_data.margin.bottom;

                    // Reset inline context after block
                    inline_ctx = InlineContext::new(line_start_x, reserved_block_y, context.parent_max_width);
                    inline_ctx.active = true;
                }
                FormattingContext::InlineContainer | FormattingContext::TextNode => {
                    // Check if this is an atomic inline (inline-block) that needs ctx update after children
                    is_atomic_inline = formatting_context == FormattingContext::InlineContainer
                        && matches!(
                            node.box_data.computed_style.display.as_str(),
                            "inline-block" | "inline-table" | "inline-flex" | "inline-grid"
                        );

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
                    let (_x, _y) = block_strategy.layout(
                        &mut node.box_data,
                        prev_box,
                        &mut reserved_block_y,
                        context,
                        adjacent_margin_bottom,
                    );
                    adjacent_margin_bottom = node.box_data.margin.bottom;
                }
            }
        }

        // Layout children recursively
        // Calculate available width for children - account for element's position offset
        let content_x =
            node.box_data.x + node.box_data.margin.left + node.box_data.padding.left;
        let available_content_width =
            (context.parent_max_width - (content_x - context.x)).max(0.0);
        let max_content_width_for_line = (available_content_width
            - node.box_data.padding.right
            - node.box_data.margin.right)
            .max(0.0);

        // For block elements with auto width, fill the available content width
        let width_is_auto = {
            let element = node.box_data.element.borrow();
            let inherited_style = element.inherited_style.as_ref().unwrap();
            !inherited_style.width.has_numeric_value()
        };

        if width_is_auto
            && matches!(
                node.box_data.computed_style.display.as_str(),
                "block" | "list-item" | "table"
            )
            && !context.shrink_to_fit
        {
            let auto_width = (context.parent_max_width
                - node.box_data.margin.left
                - node.box_data.margin.right
                - node.box_data.padding.left
                - node.box_data.padding.right)
                .max(0.0);
            node.box_data.content_width = auto_width;
        }

        if !node.children.is_empty() {
            // Check if this node has explicit width
            let node_has_explicit_width = {
                let element = node.box_data.element.borrow();
                let inherited_style = element.inherited_style.as_ref().unwrap();
                inherited_style.width.has_numeric_value()
            };

            let child_shrink_to_fit = matches!(
                node.box_data.computed_style.display.as_str(),
                "inline" | "inline-block" | "inline-table" | "inline-flex" | "inline-grid"
            );
            let node_is_shrink_to_fit = matches!(
                node.box_data.computed_style.display.as_str(),
                "inline" | "inline-block" | "inline-table" | "inline-flex" | "inline-grid"
            );
            // If this node has explicit width, don't propagate shrink_to_fit to children
            // because children should fill the explicit width, not shrink to content
            let effective_shrink_to_fit = if node_has_explicit_width {
                false
            } else {
                child_shrink_to_fit
            };
            // If this node has explicit width, it's not shrink-to-fit for child layout purposes
            let effective_node_is_shrink_to_fit = if node_has_explicit_width {
                false
            } else {
                node_is_shrink_to_fit
            };
            let parent_is_shrink_to_fit = context.shrink_to_fit || effective_node_is_shrink_to_fit;
            let parent_content_width = if parent_is_shrink_to_fit {
                available_content_width
            } else if node.box_data.content_width > 0.0 {
                node.box_data.content_width
            } else {
                available_content_width
            };

            // Calculate content y
            // For block containers, box_data.y already includes margin (it's the border-box position)
            // For inline elements, box_data.y is the content position, so we need to add margin
            let is_block = matches!(
                node.box_data.formatting_context,
                FormattingContext::BlockContainer
            );

            // Margin collapsing: when a block has no padding.top, its margin collapses
            // with the first child's margin. We pass the parent's margin to children
            // so they can compute the collapsed margin.
            let can_collapse_margin = is_block && node.box_data.padding.top == 0.0;
            let collapsible_margin = if can_collapse_margin {
                node.box_data.margin.top
            } else {
                0.0
            };

            let content_y = if is_block {
                // Block: y is border-box position, content starts after padding
                // If margin can collapse, subtract the parent's margin from y
                // so the child's margin starts from the correct position
                if can_collapse_margin {
                    node.box_data.y + node.box_data.padding.top - node.box_data.margin.top
                } else {
                    node.box_data.y + node.box_data.padding.top
                }
            } else {
                // Inline: y is content position, add margin and padding
                node.box_data.y + node.box_data.margin.top + node.box_data.padding.top
            };

            let child_context = ReflowContext {
                x: content_x,
                y: content_y,
                rel_x: if node.box_data.computed_style.position == "relative" || uses_absolute_positioning(&node.box_data.computed_style) {
                    content_x
                } else {
                    context.rel_x
                },
                rel_y: if node.box_data.computed_style.position == "relative" || uses_absolute_positioning(&node.box_data.computed_style) {
                    content_y
                } else {
                    context.rel_y
                },
                font_size: node.box_data.computed_style.font_size,
                parent_width: node.box_data.content_width,
                parent_height: node.box_data.content_height,
                parent_max_width: parent_content_width,
                layout_x_start: if node.box_data.computed_style.display == "inline" {
                    Some(node.box_data.x)
                } else {
                    None
                },
                adjacent_margin_bottom: 0.0,
                shrink_to_fit: parent_is_shrink_to_fit || effective_shrink_to_fit,
                collapsible_margin_top: collapsible_margin,
            };

            layout_tree(&mut node.children, &child_context);

            // Update parent width/height from children if not explicitly set
            let element = node.box_data.element.borrow();
            let inherited_style = element.inherited_style.as_ref().unwrap();

            if !inherited_style.width.has_numeric_value() {
                let display = node.box_data.computed_style.display.as_str();
                let is_shrink_to_fit = matches!(
                    display,
                    "inline" | "inline-block" | "inline-table" | "inline-flex" | "inline-grid"
                );

                if is_shrink_to_fit || context.shrink_to_fit {
                    // Reset content_width and recompute from children
                    // This is needed because text wrapping may have reduced child widths
                    node.box_data.content_width = 0.0;
                    for child in &node.children {
                        // For all children, box_data.x is the margin box left position
                        // margin_box_width() gives the full margin box width
                        let child_x = child.box_data.x;
                        let mut child_right = child_x + child.box_data.margin_box_width();
                        if matches!(
                            display,
                            "inline-block" | "inline-table" | "inline-flex" | "inline-grid"
                        ) && child.box_data.formatting_context == FormattingContext::TextNode
                        {
                            if let Some(intrinsic_width) = child.box_data.intrinsic_width {
                                let non_content = child.box_data.padding.left
                                    + child.box_data.padding.right
                                    + child.box_data.margin.left
                                    + child.box_data.margin.right;
                                child_right = child_x + intrinsic_width + non_content;
                            }
                        }
                        if matches!(
                            child.box_data.computed_style.display.as_str(),
                            "block" | "list-item" | "table"
                        ) && child.box_data.computed_style.width <= 0.0
                        {
                            let max_text_width = max_intrinsic_text_width(child);
                            if max_text_width > 0.0 {
                                let non_content = child.box_data.padding.left
                                    + child.box_data.padding.right
                                    + child.box_data.margin.left
                                    + child.box_data.margin.right;
                                child_right =
                                    child_right.max(child_x + max_text_width + non_content);
                            }
                        }
                        // content_width excludes padding; measure from content box left
                        let content_left = node.box_data.x
                            + node.box_data.margin.left
                            + node.box_data.padding.left;
                        node.box_data.content_width = node.box_data.content_width.max(
                            child_right - content_left
                        );
                    }
                    let min_content_width = min_intrinsic_text_width(node);
                    if min_content_width > 0.0 {
                        node.box_data.intrinsic_min_width = Some(min_content_width);
                    }
                    if context.shrink_to_fit
                        && !matches!(
                            display,
                            "inline-block" | "inline-table" | "inline-flex" | "inline-grid"
                        )
                    {
                        node.box_data.content_width =
                            node.box_data.content_width.min(max_content_width_for_line);
                    } else if matches!(
                        display,
                        "inline-block" | "inline-table" | "inline-flex" | "inline-grid"
                    ) {
                        let has_breaks = {
                            let element = node.box_data.element.borrow();
                            has_breakable_text(&element)
                        };
                        if has_breaks {
                            node.box_data.content_width = node
                                .box_data
                                .content_width
                                .min(max_content_width_for_line);
                        }
                    }
                } else {
                    // Keep block auto width (already set earlier), but clamp defensively.
                    node.box_data.content_width =
                        node.box_data.content_width.min(available_content_width);
                }
            }

            if !inherited_style.height.has_numeric_value() {
                // Reset content_height and recompute from children
                node.box_data.content_height = 0.0;
                for child in &node.children {
                    // For inline elements, box_data.y is the margin box position
                    // For block elements, box_data.y is the border box position
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
                    // content_height excludes padding; measure from content box top
                    node.box_data.content_height = node.box_data.content_height.max(
                        child_bottom - content_y
                    );
                }
            }

            // Second pass: after shrink-to-fit width is known, let block children fill it.
            if !inherited_style.width.has_numeric_value()
                && matches!(
                    node.box_data.computed_style.display.as_str(),
                    "inline-block" | "inline-table" | "inline-flex" | "inline-grid"
                )
                && node_is_shrink_to_fit
            {
                let relayout_context = ReflowContext {
                    x: content_x,
                    y: content_y,
                    rel_x: if node.box_data.computed_style.position == "relative"
                        || uses_absolute_positioning(&node.box_data.computed_style)
                    {
                        content_x
                    } else {
                        context.rel_x
                    },
                    rel_y: if node.box_data.computed_style.position == "relative"
                        || uses_absolute_positioning(&node.box_data.computed_style)
                    {
                        content_y
                    } else {
                        context.rel_y
                    },
                    font_size: node.box_data.computed_style.font_size,
                    parent_width: node.box_data.content_width,
                    parent_height: node.box_data.content_height,
                    parent_max_width: node.box_data.content_width,
                    layout_x_start: None,
                    adjacent_margin_bottom: 0.0,
                    shrink_to_fit: false,
                    collapsible_margin_top: collapsible_margin,
                };

                layout_tree(&mut node.children, &relayout_context);

                if !inherited_style.height.has_numeric_value() {
                    // Recompute height after relayout.
                    node.box_data.content_height = 0.0;
                    for child in &node.children {
                        let child_is_block = matches!(
                            child.box_data.formatting_context,
                            FormattingContext::BlockContainer
                        );
                        let child_bottom = if child_is_block {
                            child.box_data.y + child.box_data.border_box_height() + child.box_data.margin.bottom
                        } else {
                            child.box_data.y + child.box_data.margin_box_height()
                        };
                        node.box_data.content_height = node.box_data.content_height.max(
                            child_bottom - content_y
                        );
                    }
                }
            }
        }

        // After children are laid out, update inline context with actual dimensions
        // for atomic inline boxes (inline-block, etc.)
        if is_atomic_inline {
            advance_past_atomic_inline(node, &mut inline_ctx);
            reserved_block_y = reserved_block_y.max(inline_ctx.y + inline_ctx.line_height);
        }

        prev_node_opt = Some(node);
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
