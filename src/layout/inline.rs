// Unified inline formatting context layout
//
// All inline content (text segments, inline elements, inline-block) flows through
// the same algorithm. Text nodes are treated as sequences of inline items (words).

use crate::html::{DomElement, NodeType, TextSegment};
use crate::layout::flow::{FormattingContext, InlineContext, ReflowContext, uses_absolute_positioning};
use crate::layout::boxes::LayoutNode;
use crate::layout::block::compute_block_height_from_children;
use crate::text::TextMeasurer;

/// Preprocess text node: split into words and measure each
/// Caches measurements - only re-measures if font_size changed or segments are empty
pub fn preprocess_text_node(
    element: &mut DomElement,
    text_measurer: &mut dyn TextMeasurer,
    font_size: f32,
    font_path: &str,
) {
    // Check if we can reuse existing measurements
    let can_reuse = !element.text_segments.is_empty()
        && element.cached_font_size == Some(font_size)
        && element.cached_font_path.as_deref() == Some(font_path);

    if can_reuse {
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

    let (space_w, _) = text_measurer.measure(" ", font_size, font_path);
    element.space_width = space_w;

    let ascent = text_measurer.ascent(font_size, font_path);

    for word in processed_value.split_whitespace() {
        let (w, h) = text_measurer.measure(word, font_size, font_path);
        element.text_segments.push(TextSegment {
            text: word.to_string(),
            width: w,
            height: h,
            ascent,
            x: 0.0,
            y: 0.0,
        });
    }

    element.node_value = processed_value;
}

/// Compute intrinsic dimensions for a text node from its segments
pub fn compute_text_intrinsics(node: &mut LayoutNode) {
    let element = node.box_data.element.borrow();
    if element.text_segments.is_empty() {
        return;
    }

    // Max-content width: sum of all segments + spaces (single line)
    let total_width: f32 = element.text_segments.iter().map(|s| s.width).sum::<f32>()
        + element.space_width * (element.text_segments.len().saturating_sub(1) as f32);
    let max_height: f32 = element.text_segments.iter().map(|s| s.height).fold(0.0, f32::max);

    // Min-content width: widest single word
    let min_width: f32 = element.text_segments.iter().map(|s| s.width).fold(0.0, f32::max);

    drop(element);

    node.box_data.intrinsic_width = Some(total_width);
    node.box_data.intrinsic_height = Some(max_height);
    node.box_data.intrinsic_min_width = Some(min_width);
}

/// Compute intrinsic widths for a container from its children
pub fn compute_container_intrinsics(node: &mut LayoutNode) {
    let mut min_width: f32 = 0.0;
    let mut max_width: f32 = 0.0;
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
            if let Some(child_min) = child.box_data.intrinsic_min_width {
                inline_line_min += child_min + child_non_content;
            }
            if let Some(child_max) = child.box_data.intrinsic_width {
                inline_line_max += child_max + child_non_content;
            }
        } else {
            // Block child - flush inline content
            if inline_line_min > 0.0 {
                min_width = min_width.max(inline_line_min);
                inline_line_min = 0.0;
            }
            if inline_line_max > 0.0 {
                max_width = max_width.max(inline_line_max);
                inline_line_max = 0.0;
            }

            if let Some(child_min) = child.box_data.intrinsic_min_width {
                min_width = min_width.max(child_min + child_padding);
            }
            if let Some(child_max) = child.box_data.intrinsic_width {
                max_width = max_width.max(child_max + child_padding);
            }
        }
    }

    // Flush remaining inline content
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

/// Check if display value uses shrink-to-fit sizing
#[inline]
pub fn is_shrink_to_fit_display(display: &str) -> bool {
    matches!(
        display,
        "inline" | "inline-block" | "inline-table" | "inline-flex" | "inline-grid"
    )
}

/// Check if display value is atomic inline (inline-block, etc.)
#[inline]
pub fn is_atomic_inline_display(display: &str) -> bool {
    matches!(
        display,
        "inline-block" | "inline-table" | "inline-flex" | "inline-grid"
    )
}

/// Get max intrinsic text width (sum of all segments on single line)
pub fn max_intrinsic_text_width(node: &LayoutNode) -> f32 {
    if node.box_data.formatting_context == FormattingContext::TextNode {
        return node.box_data.intrinsic_width.unwrap_or(0.0);
    }

    node.children
        .iter()
        .map(max_intrinsic_text_width)
        .fold(0.0_f32, |acc, w| acc.max(w))
}

/// Get min intrinsic text width (widest single word)
pub fn min_intrinsic_text_width(node: &LayoutNode) -> f32 {
    if node.box_data.formatting_context == FormattingContext::TextNode {
        return node.box_data.intrinsic_min_width.unwrap_or(0.0);
    }

    node.children
        .iter()
        .map(min_intrinsic_text_width)
        .fold(0.0_f32, |acc, w| acc.max(w))
}

/// Check if element has breakable text (multiple words)
pub fn has_breakable_text(element: &DomElement) -> bool {
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

/// Compute shrink-to-fit width from children
/// Returns the computed content width
pub fn compute_shrink_to_fit_width(
    node: &LayoutNode,
    children: &[LayoutNode],
    max_content_width: f32,
    context_shrink_to_fit: bool,
) -> f32 {
    let display = node.box_data.computed_style.display.as_str();
    let mut content_width = 0.0_f32;

    for child in children {
        let child_x = child.box_data.x;
        let mut child_right = child_x + child.box_data.margin_box_width();

        // For atomic inline parents with text children, use intrinsic width
        if is_atomic_inline_display(display)
            && child.box_data.formatting_context == FormattingContext::TextNode
        {
            if let Some(intrinsic_width) = child.box_data.intrinsic_width {
                let non_content = child.box_data.padding.left
                    + child.box_data.padding.right
                    + child.box_data.margin.left
                    + child.box_data.margin.right;
                child_right = child_x + intrinsic_width + non_content;
            }
        }

        // For block children with auto width, use max text width
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
                child_right = child_right.max(child_x + max_text_width + non_content);
            }
        }

        // content_width excludes padding; measure from content box left
        let content_left = node.box_data.x
            + node.box_data.margin.left
            + node.box_data.padding.left;
        content_width = content_width.max(child_right - content_left);
    }

    // Apply width constraints based on display type
    if context_shrink_to_fit && !is_atomic_inline_display(display) {
        content_width = content_width.min(max_content_width);
    } else if is_atomic_inline_display(display) {
        let has_breaks = {
            let element = node.box_data.element.borrow();
            has_breakable_text(&element)
        };
        if has_breaks {
            content_width = content_width.min(max_content_width);
        }
    }

    content_width
}

/// Compute content_y for an inline element
pub fn compute_inline_content_y(box_data: &crate::layout::boxes::LayoutBox) -> f32 {
    box_data.y + box_data.margin.top + box_data.padding.top
}

/// Build child context for an inline element's children (including inline-block)
pub fn build_inline_child_context(
    node: &LayoutNode,
    context: &ReflowContext,
    content_x: f32,
    content_y: f32,
    available_content_width: f32,
) -> ReflowContext {
    let display = node.box_data.computed_style.display.as_str();
    let node_is_shrink_to_fit = is_shrink_to_fit_display(display);

    // Check if this node has explicit width
    let node_has_explicit_width = {
        let element = node.box_data.element.borrow();
        let inherited_style = element.inherited_style.as_ref().unwrap();
        inherited_style.width.has_numeric_value()
    };

    let effective_shrink_to_fit = if node_has_explicit_width { false } else { node_is_shrink_to_fit };
    let parent_is_shrink_to_fit = context.shrink_to_fit || effective_shrink_to_fit;

    let parent_content_width = if parent_is_shrink_to_fit {
        available_content_width
    } else if node.box_data.content_width > 0.0 {
        node.box_data.content_width
    } else {
        available_content_width
    };

    let rel_x_base = node.box_data.x + node.box_data.margin.left;
    let rel_y_base = node.box_data.y + node.box_data.margin.top;

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
        layout_x_start: if display == "inline" {
            Some(node.box_data.x)
        } else {
            None
        },
        adjacent_margin_bottom: 0.0,
        shrink_to_fit: parent_is_shrink_to_fit,
        collapsible_margin_top: 0.0,
    }
}

/// Finalize inline element dimensions after children are laid out
pub fn finalize_inline_dimensions(
    node: &mut LayoutNode,
    content_y: f32,
    max_content_width: f32,
    context_shrink_to_fit: bool,
) {
    let display = node.box_data.computed_style.display.clone();

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
        if is_shrink_to_fit_display(&display) || context_shrink_to_fit {
            node.box_data.content_width = compute_shrink_to_fit_width(
                node,
                &node.children,
                max_content_width,
                context_shrink_to_fit,
            );
            let min_content_width = min_intrinsic_text_width(node);
            if min_content_width > 0.0 {
                node.box_data.intrinsic_min_width = Some(min_content_width);
            }
        }
    }

    if !height_has_value {
        node.box_data.content_height = compute_block_height_from_children(&node.children, content_y);
    }
}

/// Check if inline-block needs relayout and build the relayout context
pub fn build_relayout_context(
    node: &LayoutNode,
    context: &ReflowContext,
    content_x: f32,
    content_y: f32,
    collapsible_margin: f32,
) -> Option<ReflowContext> {
    let display = node.box_data.computed_style.display.as_str();
    let node_is_shrink_to_fit = is_shrink_to_fit_display(display);

    let width_has_value = {
        let element = node.box_data.element.borrow();
        let inherited_style = element.inherited_style.as_ref().unwrap();
        inherited_style.width.has_numeric_value()
    };

    // Only relayout atomic inlines without explicit width that use shrink-to-fit
    if width_has_value || !is_atomic_inline_display(display) || !node_is_shrink_to_fit {
        return None;
    }

    let rel_x_base = node.box_data.x + node.box_data.margin.left;
    let rel_y_base = node.box_data.y + node.box_data.margin.top;

    Some(ReflowContext {
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
        parent_max_width: node.box_data.content_width,
        layout_x_start: None,
        adjacent_margin_bottom: 0.0,
        shrink_to_fit: false,
        collapsible_margin_top: collapsible_margin,
    })
}

/// Finalize dimensions after relayout
pub fn finalize_after_relayout(node: &mut LayoutNode, content_y: f32) {
    let height_has_value = {
        let element = node.box_data.element.borrow();
        let inherited_style = element.inherited_style.as_ref().unwrap();
        inherited_style.height.has_numeric_value()
    };

    if !height_has_value {
        node.box_data.content_height = compute_block_height_from_children(&node.children, content_y);
    }
}

/// Layout children of an inline element (including inline-block)
pub fn layout_inline_children(
    node: &mut LayoutNode,
    context: &ReflowContext,
    layout_fn: &mut dyn FnMut(&mut Vec<LayoutNode>, &ReflowContext),
) {
    if node.children.is_empty() {
        return;
    }

    let content_x = node.box_data.x + node.box_data.margin.left + node.box_data.padding.left;
    let available_width = (context.parent_max_width - (content_x - context.x)).max(0.0);
    let max_content_width = (available_width
        - node.box_data.padding.right
        - node.box_data.margin.right)
        .max(0.0);

    let content_y = compute_inline_content_y(&node.box_data);
    let child_context = build_inline_child_context(node, context, content_x, content_y, available_width);

    layout_fn(&mut node.children, &child_context);

    finalize_inline_dimensions(node, content_y, max_content_width, context.shrink_to_fit);

    // Inline-block may need relayout after shrink-to-fit width is determined
    if let Some(relayout_ctx) = build_relayout_context(node, context, content_x, content_y, 0.0) {
        layout_fn(&mut node.children, &relayout_ctx);
        finalize_after_relayout(node, content_y);
    }
}

/// Unified inline formatting context layout
///
/// This is the single entry point for laying out inline content.
/// All inline items (text words, inline elements, inline-blocks) use the same
/// positioning and wrapping logic.
pub fn layout_inline_content(
    children: &mut [LayoutNode],
    ctx: &mut InlineContext,
) {
    for child in children.iter_mut() {
        match child.box_data.formatting_context {
            FormattingContext::TextNode => {
                // Text node: each word is an inline item
                layout_text_node(child, ctx);
            }
            FormattingContext::InlineContainer => {
                let display = child.box_data.computed_style.display.as_str();
                let is_atomic = matches!(
                    display,
                    "inline-block" | "inline-table" | "inline-flex" | "inline-grid"
                );

                if is_atomic {
                    // Atomic inline box (inline-block etc) - wraps as a unit
                    layout_atomic_inline(child, ctx);
                } else {
                    // display: inline - transparent wrapper, children flow directly
                    layout_transparent_inline(child, ctx);
                }
            }
            FormattingContext::BlockContainer => {
                // Block in inline context breaks the line
                if ctx.x > ctx.line_start_x {
                    ctx.wrap_to_next_line();
                }
                // Block positioning is handled by the caller
            }
            _ => {}
        }
    }
}

/// Layout a text node by positioning each word segment
fn layout_text_node(node: &mut LayoutNode, ctx: &mut InlineContext) {
    let mut element = node.box_data.element.borrow_mut();
    if element.text_segments.is_empty() {
        return;
    }

    let space_w = element.space_width;

    for (i, seg) in element.text_segments.iter_mut().enumerate() {
        // Determine if we need a space before this word
        let needs_space = i > 0 || ctx.x > ctx.line_start_x;
        let space_before = if needs_space { space_w } else { 0.0 };

        // Check if this word fits on current line
        let item_width = seg.width + space_before;
        if should_wrap(ctx, item_width) {
            ctx.wrap_to_next_line();
        }

        // Position this word
        let space_to_add = if ctx.x > ctx.line_start_x { space_w } else { 0.0 };
        seg.x = ctx.x + space_to_add;
        seg.y = ctx.y;

        // Advance inline position
        ctx.x = seg.x + seg.width;
        ctx.line_height = ctx.line_height.max(seg.height);
    }

    // Update box dimensions based on segments
    if !element.text_segments.is_empty() {
        let min_x = element.text_segments.iter().map(|s| s.x).fold(f32::MAX, f32::min);
        let max_x = element.text_segments.iter().map(|s| s.x + s.width).fold(0.0_f32, f32::max);
        let min_y = element.text_segments.iter().map(|s| s.y).fold(f32::MAX, f32::min);
        let max_y = element.text_segments.iter().map(|s| s.y + s.height).fold(0.0_f32, f32::max);

        drop(element);

        node.box_data.x = min_x;
        node.box_data.y = min_y;
        node.box_data.content_width = max_x - min_x;
        node.box_data.content_height = max_y - min_y;
    }
}

/// Layout an atomic inline box (inline-block, etc) - cannot break internally
///
/// For inline-blocks, we need to:
/// 1. Estimate width for wrap decision (using intrinsic width if available)
/// 2. Position the box
/// 3. Layout children to determine actual size
/// 4. Advance context by actual size
fn layout_atomic_inline(node: &mut LayoutNode, ctx: &mut InlineContext) {
    // Use the best available width estimate for wrap decision
    // Priority: explicit content_width > intrinsic_width > intrinsic_min_width > 0
    let base_width = if node.box_data.content_width > 0.0 {
        node.box_data.content_width
    } else if let Some(w) = node.box_data.intrinsic_width {
        w
    } else if let Some(w) = node.box_data.intrinsic_min_width {
        w
    } else {
        0.0
    };

    let estimated_width = base_width
        + node.box_data.padding.left + node.box_data.padding.right
        + node.box_data.margin.left + node.box_data.margin.right;

    // Check if this box fits (with small tolerance for rounding)
    if should_wrap_with_tolerance(ctx, estimated_width, 0.5) {
        ctx.wrap_to_next_line();
    }

    // Position the box at current inline position
    node.box_data.x = ctx.x;
    node.box_data.y = ctx.y;

    // Note: Children will be laid out by the caller (layout_tree in pipeline.rs)
    // The caller must call `advance_past_atomic_inline` after laying out children
    // to properly advance the inline context.
    //
    // For now, we advance by estimated width. This will be corrected by the caller
    // if the actual width differs.
    ctx.x += estimated_width;

    let height = node.box_data.margin_box_height();
    if height > 0.0 {
        ctx.line_height = ctx.line_height.max(height);
    }
}

/// Advance inline context past an atomic inline box after its children have been laid out
/// This should be called after laying out children of an inline-block
pub fn advance_past_atomic_inline(node: &LayoutNode, ctx: &mut InlineContext) {
    let actual_width = node.box_data.margin_box_width();
    let height = node.box_data.margin_box_height();

    // Update x to be after this element (node.box_data.x + actual_width)
    ctx.x = node.box_data.x + actual_width;
    ctx.line_height = ctx.line_height.max(height);
}

/// Layout a transparent inline wrapper (display: inline)
/// Children participate directly in the inline flow
fn layout_transparent_inline(node: &mut LayoutNode, ctx: &mut InlineContext) {
    let start_x = ctx.x;
    let start_y = ctx.y;

    // Recurse into children - they flow as part of this inline context
    layout_inline_content(&mut node.children, ctx);

    // Calculate position and dimensions from actual child positions
    // x: use min of children (accounts for wrap resetting x to line start)
    // y: use start_y (inline element starts on the line where it began)
    if !node.children.is_empty() {
        let mut min_x: f32 = f32::MAX;
        let mut max_x: f32 = f32::MIN;
        let mut max_y: f32 = f32::MIN;
        for child in &node.children {
            min_x = min_x.min(child.box_data.x);
            max_x = max_x.max(child.box_data.x + child.box_data.margin_box_width());
            max_y = max_y.max(child.box_data.y + child.box_data.margin_box_height());
        }
        node.box_data.x = min_x;
        node.box_data.y = start_y;
        node.box_data.content_width = max_x - min_x;
        node.box_data.content_height = max_y - start_y;
    } else {
        node.box_data.x = start_x;
        node.box_data.y = start_y;
    }
}

/// Check if an item should wrap to the next line
#[inline]
fn should_wrap(ctx: &InlineContext, item_width: f32) -> bool {
    should_wrap_with_tolerance(ctx, item_width, 0.01)
}

/// Check if an item should wrap, with configurable tolerance
#[inline]
fn should_wrap_with_tolerance(ctx: &InlineContext, item_width: f32, tolerance: f32) -> bool {
    let max_right = ctx.line_start_x + ctx.max_width;
    // Only wrap if we're not at the start of a line and item doesn't fit
    ctx.x > ctx.line_start_x && ctx.x + item_width > max_right + tolerance
}
