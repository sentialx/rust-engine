// Layout pipeline - Build, Measure, Layout, Finalize passes

use crate::html::{DomElement, NodeType, ComputedFlow};
use crate::render_frame::TextMeasurer;
use crate::styles::ScalarEvaluationContext;
use crate::layout::{Rect, boxes::{LayoutBox, LayoutNode}};
use crate::layout::flow::{FormattingContext, get_formatting_context, ReflowContext};
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

/// Measure pass: Compute intrinsic dimensions (before layout)
/// For text, this just gets a rough size. Actual text wrapping happens after layout.
pub fn measure_layout_tree(
    layout_nodes: &mut Vec<LayoutNode>,
    text_measurer: &mut dyn TextMeasurer,
    _max_width: f32,
) {
    for node in layout_nodes.iter_mut() {
        let element = node.box_data.element.borrow();
        let comp_style = &node.box_data.computed_style;
        
        // For text nodes, get basic measurement (wrapping happens later with actual positions)
        if element.node_type == NodeType::Text && !element.node_value.is_empty() {
            let font_size = comp_style.font_size;
            // Use the same font path format as the old code
            let inherited_style = element.inherited_style.as_ref().unwrap();
            let font_path = inherited_style.font.get_path();
            
            // Simple measurement - actual wrapping happens in layout pass
            let (text_width, text_height) = text_measurer.measure(
                &element.node_value,
                font_size,
                &font_path,
            );
            
            node.box_data.intrinsic_width = Some(text_width);
            node.box_data.intrinsic_height = Some(text_height);
            // Don't set content_width/height yet - that happens after text wrapping in layout
        } else {
            // For non-text elements, use specified width/height if available
            if comp_style.width > 0.0 {
                node.box_data.content_width = comp_style.width;
            }
            if comp_style.height > 0.0 {
                node.box_data.content_height = comp_style.height;
            }
        }
        
        drop(element);
        
        // Measure children first (for width/height calculation)
        if !node.children.is_empty() {
            let child_max_width = if node.box_data.computed_style.width > 0.0 {
                node.box_data.content_width - node.box_data.padding.left - node.box_data.padding.right
            } else {
                _max_width
            };
            measure_layout_tree(&mut node.children, text_measurer, child_max_width);
        }
    }
}

/// Measure text and handle wrapping after layout positions are known
/// This is called after layout_tree to handle text wrapping with actual positions
pub fn measure_text_after_layout(
    layout_nodes: &mut Vec<LayoutNode>,
    text_measurer: &mut dyn TextMeasurer,
    max_width: f32,
    layout_x_start: f32,
) {
    use crate::layout::wrap_text;
    
    for node in layout_nodes.iter_mut() {
        let comp_style = node.box_data.computed_style.clone();
        let (node_type, node_value, white_space, font_path) = {
            let element = node.box_data.element.borrow();
            let inherited_style = element.inherited_style.as_ref().unwrap();
            (
                element.node_type.clone(),
                element.node_value.clone(),
                inherited_style.white_space.get(),
                inherited_style.font.get_path(),
            )
        };
        
        // Measure and wrap text content (now that positions are known)
        if node_type == NodeType::Text && !node_value.is_empty() {
            // Handle HTML entities
            let mut processed_value = node_value
                .replace("&nbsp;", " ")
                .replace("&gt;", ">")
                .replace("&lt;", "<");
            
            let font_size = comp_style.font_size;
            
            // Handle text wrapping with actual positions
            let lines = if white_space != "nowrap" {
                wrap_text(
                    processed_value.clone(),
                    max_width,
                    text_measurer,
                    font_size,
                    font_path.clone(),
                    node.box_data.x,
                    node.box_data.y,
                    layout_x_start,
                )
            } else {
                let size = text_measurer.measure(
                    &processed_value,
                    font_size,
                    &font_path,
                );
                vec![crate::html::TextLine {
                    text: processed_value.clone(),
                    x: node.box_data.x,
                    y: node.box_data.y,
                    width: size.0,
                    height: size.1,
                }]
            };
            
            // Store lines and processed value in element
            let mut element = node.box_data.element.borrow_mut();
            element.node_value = processed_value;
            element.lines = lines.clone();
            drop(element);
            
            // Calculate dimensions from lines
            if !lines.is_empty() {
                let max_line_width = lines
                    .iter()
                    .map(|l| l.width)
                    .fold(0.0_f32, |acc, w| acc.max(w));
                node.box_data.content_width = max_line_width;
                node.box_data.content_height = lines.iter().map(|l| l.height).sum::<f32>() + 8.0;
            }
        }
        
        // Measure children text after their layout
        if !node.children.is_empty() {
            // All elements establish their own layout context for text wrapping
            let child_layout_x_start =
                node.box_data.x + node.box_data.margin.left + node.box_data.padding.left;
            
            let child_max_width = if node.box_data.computed_style.width > 0.0 {
                // Element has explicit width - use content width directly
                node.box_data.content_width
            } else if matches!(
                node.box_data.computed_style.display.as_str(),
                "inline" | "inline-block" | "inline-table" | "inline-flex" | "inline-grid"
            ) {
                // Avoid shrink feedback loops for shrink-to-fit containers
                max_width
            } else if node.box_data.content_width > 0.0 {
                // Use computed content width if available (e.g. block auto width)
                node.box_data.content_width
            } else {
                // max_width is already in the parent's content coordinate space
                max_width
            };
            measure_text_after_layout(&mut node.children, text_measurer, child_max_width, child_layout_x_start);
        }
    }
}

/// Layout pass: Assign positions and sizes using formatting context strategies
/// Note: Text measurement/wrapping happens after this pass (in measure_text_after_layout)
pub fn layout_tree(
    layout_nodes: &mut Vec<LayoutNode>,
    context: &ReflowContext,
) -> (f32, f32) { // Returns (reserved_block_y, adjacent_margin_bottom)
    use crate::layout::block::BlockLayoutStrategy;
    use crate::layout::inline::InlineLayoutStrategy;
    use crate::layout::abspos::AbsoluteLayoutStrategy;
    use crate::layout::flow::uses_absolute_positioning;
    
    let block_strategy = BlockLayoutStrategy;
    let inline_strategy = InlineLayoutStrategy;
    let abspos_strategy = AbsoluteLayoutStrategy;
    
    let mut reserved_block_y = context.y;
    let mut inline_ctx = crate::layout::InlineContext::new(context.x, context.y);
    let mut adjacent_margin_bottom = 0.0;
    let mut prev_node_opt: Option<&LayoutNode> = None;
    
    for node in layout_nodes.iter_mut() {
        let formatting_context = node.box_data.formatting_context;
        
        // Skip if display: none (shouldn't be in tree, but check anyway)
        // display is already a String in ComputedStyle, so this is fine
        if node.box_data.computed_style.display == "none" {
            continue;
        }
        
        // Layout based on formatting context
        if uses_absolute_positioning(&node.box_data.computed_style) {
            // Absolute positioning
            abspos_strategy.layout(&mut node.box_data, context);
        } else {
            match formatting_context {
                FormattingContext::BlockContainer => {
                    let prev_box = prev_node_opt.map(|n| &n.box_data);
                    let (x, y) = block_strategy.layout(
                        &mut node.box_data,
                        prev_box,
                        &mut reserved_block_y,
                        context,
                        adjacent_margin_bottom,
                    );
                    // Update state - create a mutable context for this
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
                    };
                    block_strategy.update_state(
                        &node.box_data,
                        &mut reserved_block_y,
                        &mut update_context,
                        node.box_data.margin.bottom,
                    );
                    adjacent_margin_bottom = update_context.adjacent_margin_bottom;
                    adjacent_margin_bottom = node.box_data.margin.bottom;
                }
                FormattingContext::InlineContainer | FormattingContext::TextNode => {
                    let prev_box = prev_node_opt.map(|n| &n.box_data);
                    let (_x, _y) = inline_strategy.layout(
                        &mut node.box_data,
                        prev_box,
                        &mut inline_ctx,
                        context,
                    );
                    // Defer inline state update until after children/size adjustments
                }
                FormattingContext::FlexContainer | FormattingContext::GridContainer => {
                    // TODO: Implement flex/grid
                    // For now, treat as block
                    let prev_box = prev_node_opt.map(|n| &n.box_data);
                    let (x, y) = block_strategy.layout(
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
            let child_shrink_to_fit = context.shrink_to_fit
                || matches!(
                    node.box_data.computed_style.display.as_str(),
                    "inline" | "inline-block" | "inline-table" | "inline-flex" | "inline-grid"
                );
            let parent_content_width = if context.shrink_to_fit {
                // Avoid shrink feedback loops: measure children against available width.
                available_content_width
            } else if node.box_data.content_width > 0.0 {
                node.box_data.content_width
            } else {
                available_content_width
            };

            let child_context = ReflowContext {
                x: content_x,
                y: node.box_data.y + node.box_data.padding.top,
                rel_x: if node.box_data.computed_style.position == "relative" || uses_absolute_positioning(&node.box_data.computed_style) {
                    content_x
                } else {
                    context.rel_x
                },
                rel_y: if node.box_data.computed_style.position == "relative" || uses_absolute_positioning(&node.box_data.computed_style) {
                    node.box_data.y + node.box_data.padding.top
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
                shrink_to_fit: child_shrink_to_fit,
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
                        let child_x = child.box_data.x + child.box_data.margin.left;
                        let child_right = child_x + child.box_data.margin_box_width();
                        // content_width excludes padding; measure from content box left
                        let content_left = node.box_data.x
                            + node.box_data.margin.left
                            + node.box_data.padding.left;
                        node.box_data.content_width = node.box_data.content_width.max(
                            child_right - content_left
                        );
                    }
                    // Also clamp to available content width
                    node.box_data.content_width =
                        node.box_data.content_width.min(available_content_width);
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
                    let child_y = child.box_data.y + child.box_data.margin.top;
                    let child_bottom = child_y + child.box_data.margin_box_height();
                    // content_height excludes padding; measure from content box top
                    let content_top = node.box_data.y + node.box_data.padding.top;
                    node.box_data.content_height = node.box_data.content_height.max(
                        child_bottom - content_top
                    );
                }
            }
        }

        // Update inline layout state after children sizing is known
        if matches!(
            node.box_data.formatting_context,
            FormattingContext::InlineContainer | FormattingContext::TextNode
        ) {
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
                adjacent_margin_bottom: 0.0,
                shrink_to_fit: context.shrink_to_fit,
            };
            inline_strategy.update_state(
                &node.box_data,
                &mut inline_ctx,
                &mut reserved_block_y,
                &mut update_context,
            );
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
        
        // Finalize children
        if !node.children.is_empty() {
            finalize_layout_tree(&node.children);
        }
    }
}
