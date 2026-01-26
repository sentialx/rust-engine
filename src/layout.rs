use crate::colors::*;
use crate::css::*;
use crate::css_value::CssValue;
use crate::html::{DomElement, NodeType, TextSegment};
use crate::render_frame::TextMeasurer;
use crate::styles::*;
use crate::utils::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// Declare layout submodules (files in src/layout/ directory)
#[path = "layout/boxes.rs"]
pub mod boxes;
#[path = "layout/flow.rs"]
pub mod flow;
#[path = "layout/block.rs"]
pub mod block;
#[path = "layout/inline.rs"]
pub mod inline;
#[path = "layout/abspos.rs"]
pub mod abspos;
#[path = "layout/pipeline.rs"]
pub mod pipeline;


// Re-export types from submodules for backward compatibility
pub use flow::{FormattingContext, get_formatting_context, ReflowContext, InlineContext};

#[derive(Clone, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug)]
pub struct RenderItem {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub text_segments: Vec<TextSegment>,  // Preprocessed words with positions
    pub font_size: f32,
    pub font_path: String,
    pub background_color: ColorTupleA,
    pub color: ColorTupleA,
    pub underline: bool,
    pub element: Option<Rc<RefCell<DomElement>>>,
}

pub fn rect_contains(rect: &Rect, x: f32, y: f32) -> bool {
    return x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height;
}

pub fn is_in_viewport(rect: &Rect, viewport: &Rect) -> bool {
    return rect.x + rect.width >= viewport.x
        && rect.x <= viewport.x + viewport.width
        && rect.y + rect.height >= viewport.y
        && rect.y <= viewport.y + viewport.height;
}

pub fn get_element_at(
    render_items: &Vec<RenderItem>,
    x: f32,
    y: f32,
) -> Option<&Rc<RefCell<DomElement>>> {
    for i in (0..render_items.len()).rev() {
        let item = &render_items[i];

        let element = match &item.element {
            Some(e) => e,
            None => continue,
        };
        let element = element.borrow();

        if element.node_type == NodeType::Text {
            continue;
        }

        let computed_flow = element.computed_flow.as_ref().unwrap();
        let rect = Rect {
            x: computed_flow.x,
            y: computed_flow.y,
            width: computed_flow.width,
            height: computed_flow.height,
        };

        if rect_contains(&rect, x, y) {
            return Some(item.element.as_ref().unwrap());
        }
    }

    return None;
}

pub fn element_matches_selector(
    element: &DomElement,
    selector: &CssSelector,
    parents: &[*mut DomElement],
) -> bool {
    match selector {
        CssSelector::Tag(tag) => element.tag_name.eq_ignore_ascii_case(tag) || tag == "*",
        CssSelector::Id(id) => element.attributes.get("id").map_or(false, |v| v == id),
        CssSelector::Class(class) => {
            element.class_list.iter().any(|c| c == class)
            // element.attributes.get("class").map_or(false, |v| v.split_whitespace().any(|c| c == class))
        }
        CssSelector::Attribute {
            name,
            operator,
            value,
        } => match operator {
            Some(op) => match element.attributes.get(name) {
                Some(attr_value) => match op.as_str() {
                    "=" => value.is_some() && attr_value == value.as_deref().unwrap(),
                    "~" => attr_value
                        .split_whitespace()
                        .any(|part| part == value.as_deref().unwrap()),
                    "|" => {
                        attr_value == value.as_deref().unwrap()
                            || attr_value.starts_with(&(value.clone().unwrap() + "-"))
                    }
                    "^" => attr_value.starts_with(value.as_deref().unwrap()),
                    "$" => attr_value.ends_with(value.as_deref().unwrap()),
                    "*" => attr_value.contains(value.as_deref().unwrap()),
                    _ => false,
                },
                None => false,
            },
            None => element.attributes.contains_key(name),
        },
        CssSelector::PseudoClass(pseudo) => {
            // Handle pseudo-classes
            false // For now, we just return true
        }
        CssSelector::PseudoElement(pseudo) => {
            // Handle pseudo-elements
            false // For now, we just return true
        }
        CssSelector::Combinator {
            combinator,
            selectors,
        } => match combinator.as_str() {
            ">" => parents.last().map_or(false, |parent| {
                selectors.iter().all(|selector| {
                    element_matches_selector(
                        unsafe { &**parent },
                        selector,
                        &parents[..parents.len() - 1],
                    )
                })
            }),
            " " => parents.iter().rev().any(|parent| {
                selectors.iter().all(|selector| {
                    element_matches_selector(
                        unsafe { &**parent },
                        selector,
                        &parents[..parents.len() - 1],
                    )
                })
            }),
            // "+" => parents.last().map_or(false, |parent| element_matches_selector(parent, selector, &parents[..parents.len()-1])),
            // "~" => parents.iter().rev().any(|parent| element_matches_selector(parent, selector, &parents[..parents.len()-1])),
            _ => false,
        },
        CssSelector::OrGroup { selectors } => {
            selectors.len() > 0
                && selectors
                    .iter()
                    .any(|s| element_matches_selector(element, s, parents))
        }
        CssSelector::AndGroup { selectors } => {
            selectors.len() > 0
                && selectors
                    .iter()
                    .all(|s| element_matches_selector(element, s, parents))
        }
        _ => false,
    }
}

#[derive(Clone, Debug)]
pub struct CssVariablesContext {
    pub variables: HashMap<String, CssValue>,
}

impl CssVariablesContext {
    pub fn new() -> CssVariablesContext {
        CssVariablesContext {
            variables: HashMap::new(),
        }
    }
}

/// Index for fast style rule lookup
/// Groups rules by their key selector (rightmost simple selector)
#[derive(Debug)]
pub struct StyleRuleIndex {
    pub by_tag: HashMap<String, Vec<usize>>,    // tag name -> rule indices
    pub by_class: HashMap<String, Vec<usize>>,  // class name -> rule indices
    pub by_id: HashMap<String, Vec<usize>>,     // id -> rule indices
    pub universal: Vec<usize>,                   // rules that match anything (* or complex)
}

impl StyleRuleIndex {
    pub fn new(rules: &Vec<StyleRule>) -> StyleRuleIndex {
        let mut index = StyleRuleIndex {
            by_tag: HashMap::new(),
            by_class: HashMap::new(),
            by_id: HashMap::new(),
            universal: Vec::new(),
        };

        for (i, rule) in rules.iter().enumerate() {
            index.index_selector(&rule.selector, i);
        }

        index
    }

    fn index_selector(&mut self, selector: &CssSelector, rule_index: usize) {
        match selector {
            CssSelector::Tag(tag) => {
                if tag == "*" {
                    self.universal.push(rule_index);
                } else {
                    self.by_tag
                        .entry(tag.to_lowercase())
                        .or_insert_with(Vec::new)
                        .push(rule_index);
                }
            }
            CssSelector::Class(class) => {
                self.by_class
                    .entry(class.clone())
                    .or_insert_with(Vec::new)
                    .push(rule_index);
            }
            CssSelector::Id(id) => {
                self.by_id
                    .entry(id.clone())
                    .or_insert_with(Vec::new)
                    .push(rule_index);
            }
            CssSelector::AndGroup { selectors } => {
                // For AndGroup, index by the most specific selector
                // Priority: ID > Class > Tag
                let mut indexed = false;
                for sel in selectors {
                    if let CssSelector::Id(id) = sel {
                        self.by_id
                            .entry(id.clone())
                            .or_insert_with(Vec::new)
                            .push(rule_index);
                        indexed = true;
                        break;
                    }
                }
                if !indexed {
                    for sel in selectors {
                        if let CssSelector::Class(class) = sel {
                            self.by_class
                                .entry(class.clone())
                                .or_insert_with(Vec::new)
                                .push(rule_index);
                            indexed = true;
                            break;
                        }
                    }
                }
                if !indexed {
                    for sel in selectors {
                        if let CssSelector::Tag(tag) = sel {
                            if tag == "*" {
                                self.universal.push(rule_index);
                            } else {
                                self.by_tag
                                    .entry(tag.to_lowercase())
                                    .or_insert_with(Vec::new)
                                    .push(rule_index);
                            }
                            indexed = true;
                            break;
                        }
                    }
                }
                if !indexed {
                    self.universal.push(rule_index);
                }
            }
            CssSelector::OrGroup { selectors } => {
                // OrGroup matches if ANY selector matches - index by all
                for sel in selectors {
                    self.index_selector(sel, rule_index);
                }
            }
            CssSelector::Combinator { selectors, .. } => {
                // For combinators like "div > .foo", index by rightmost (last) selector
                if let Some(last) = selectors.last() {
                    self.index_selector(last, rule_index);
                } else {
                    self.universal.push(rule_index);
                }
            }
            _ => {
                // Attribute selectors, pseudo-classes, etc. go to universal
                self.universal.push(rule_index);
            }
        }
    }

    /// Get candidate rule indices for an element
    pub fn get_candidates(&self, element: &DomElement) -> Vec<usize> {
        let mut candidates: Vec<usize> = Vec::new();

        // Add rules by tag
        if let Some(indices) = self.by_tag.get(&element.tag_name.to_lowercase()) {
            candidates.extend(indices);
        }

        // Add rules by classes
        for class in &element.class_list {
            if let Some(indices) = self.by_class.get(class) {
                candidates.extend(indices);
            }
        }

        // Add rules by ID
        if let Some(id) = element.attributes.get("id") {
            if let Some(indices) = self.by_id.get(id) {
                candidates.extend(indices);
            }
        }

        // Add universal rules
        candidates.extend(&self.universal);

        // Sort and deduplicate to maintain rule order
        candidates.sort_unstable();
        candidates.dedup();

        candidates
    }
}

pub fn compute_styles(
    tree: &mut Vec<Rc<RefCell<DomElement>>>,
    style: &Vec<StyleRule>,
    parents: &mut Vec<*mut DomElement>,
    var_ctx: Option<CssVariablesContext>,
) {
    // Build index on first call (when parents is empty)
    let index = if parents.is_empty() {
        Some(StyleRuleIndex::new(style))
    } else {
        None
    };

    compute_styles_with_index(tree, style, parents, var_ctx, index.as_ref());
}

fn compute_styles_with_index(
    tree: &mut Vec<Rc<RefCell<DomElement>>>,
    style: &Vec<StyleRule>,
    parents: &mut Vec<*mut DomElement>,
    var_ctx: Option<CssVariablesContext>,
    index: Option<&StyleRuleIndex>,
) {
    let mut var_ctx = var_ctx;

    if var_ctx.is_none() {
        var_ctx = Some(CssVariablesContext::new());

        for rule in style {
            if rule.selector.to_string() == ":root" {
                for decl in &rule.declarations {
                    if decl.key.starts_with("--") {
                        var_ctx.as_mut().unwrap().variables.insert(
                            decl.key.clone(),
                            decl.value.clone(),
                        );
                    }
                }
            }
        }
    }

    for i in 0..tree.len() {
        let mut element = tree[i].borrow_mut();

        // Use index to get candidate rules, or fall back to all rules
        let candidates: Vec<usize> = match index {
            Some(idx) => idx.get_candidates(&element),
            None => (0..style.len()).collect(),
        };

        for rule_idx in candidates {
            let style_rule = &style[rule_idx];
            if element_matches_selector(&element, &style_rule.selector, parents) {
                element.style.insert_declarations(&style_rule.declarations, var_ctx.as_ref().unwrap());
                element.matched_styles.push(style_rule.clone());
            }
        }

        if element.children.len() > 0 && element.tag_name != "SCRIPT" && element.tag_name != "STYLE"
        {
            parents.push(tree[i].as_ptr());
            compute_styles_with_index(&mut element.children, style, parents, var_ctx.clone(), index);
            parents.pop();
        }
    }
}

pub fn propagate_styles(tree: &mut Vec<Rc<RefCell<DomElement>>>, parent_style: Option<&Style>) {
    let parent_style = match parent_style {
        Some(s) => s,
        None => &Style::new(),
    };

    for i in 0..tree.len() {
        let mut element = tree[i].borrow_mut();
        let inherited_styles = element.style.create_inherited(&parent_style);

        if element.children.len() > 0 && element.tag_name != "SCRIPT" && element.tag_name != "STYLE"
        {
            propagate_styles(&mut element.children, Some(&inherited_styles));
        }

        element.inherited_style = Some(inherited_styles);
    }
}

// ============================================================================
// Formatting Context - Generalized Layout Algorithm
// ============================================================================
// Types are now defined in layout::flow module and re-exported above

// ============================================================================
// Positioning Helpers
// ============================================================================

/// Computes element position using FormattingContext classification
/// This is the core of the generalized layout algorithm
/// Reflow using pipeline architecture - single pass with unified text/inline layout
pub fn reflow(
    tree: &mut Vec<Rc<RefCell<DomElement>>>,
    text_measurer: &mut dyn TextMeasurer,
    context: Option<&mut ReflowContext>,
    viewport: &Rect,
) {
    reflow_with_cache(tree, text_measurer, context, viewport, None);
}

/// Reflow with optional cached layout tree for resize optimization
/// If cached_tree is Some, skips build+measure and only runs layout+finalize
pub fn reflow_with_cache(
    tree: &mut Vec<Rc<RefCell<DomElement>>>,
    text_measurer: &mut dyn TextMeasurer,
    context: Option<&mut ReflowContext>,
    viewport: &Rect,
    cached_tree: Option<&mut Vec<boxes::LayoutNode>>,
) {
    use crate::layout::pipeline::{
        build_layout_tree, finalize_layout_tree, layout_tree, measure_layout_tree,
        reset_layout_positions,
    };

    let mut default_context = ReflowContext {
        x: 0.0,
        y: 0.0,
        rel_x: 0.0,
        rel_y: 0.0,
        font_size: 16.0,
        parent_width: viewport.width,
        parent_height: viewport.height,
        parent_max_width: viewport.width,
        layout_x_start: None,
        adjacent_margin_bottom: 0.0,
        shrink_to_fit: false,
        collapsible_margin_top: 0.0,
    };

    let sibling_context = context.unwrap_or(&mut default_context);

    match cached_tree {
        Some(layout_nodes) => {
            // Fast path: reuse cached tree, only re-layout
            reset_layout_positions(layout_nodes);
            layout_tree(layout_nodes, sibling_context);
            finalize_layout_tree(layout_nodes);
        }
        None => {
            // Full path: build + measure + layout + finalize
            let mut layout_nodes = build_layout_tree(tree, sibling_context);
            let max_width = sibling_context.parent_max_width;
            measure_layout_tree(&mut layout_nodes, text_measurer, max_width);
            layout_tree(&mut layout_nodes, sibling_context);
            finalize_layout_tree(&layout_nodes);
        }
    }
}

/// Full reflow that returns the layout tree for caching
pub fn reflow_and_cache(
    tree: &mut Vec<Rc<RefCell<DomElement>>>,
    text_measurer: &mut dyn TextMeasurer,
    viewport: &Rect,
) -> Vec<boxes::LayoutNode> {
    use crate::layout::pipeline::{
        build_layout_tree, finalize_layout_tree, layout_tree, measure_layout_tree,
    };

    let mut context = ReflowContext {
        x: 0.0,
        y: 0.0,
        rel_x: 0.0,
        rel_y: 0.0,
        font_size: 16.0,
        parent_width: viewport.width,
        parent_height: viewport.height,
        parent_max_width: viewport.width,
        layout_x_start: None,
        adjacent_margin_bottom: 0.0,
        shrink_to_fit: false,
        collapsible_margin_top: 0.0,
    };

    let mut layout_nodes = build_layout_tree(tree, &context);
    let max_width = context.parent_max_width;
    measure_layout_tree(&mut layout_nodes, text_measurer, max_width);
    layout_tree(&mut layout_nodes, &context);
    finalize_layout_tree(&layout_nodes);

    layout_nodes
}

pub fn get_render_array(
    tree: &mut Vec<Rc<RefCell<DomElement>>>,
    viewport: &Rect,
) -> Vec<RenderItem> {
    let mut array: Vec<RenderItem> = vec![];

    for i in 0..tree.len() {
        let element = &mut tree[i].borrow_mut();
        let computed_flow = element.computed_flow.as_ref();
        if computed_flow.is_none() {
            continue;
        }
        let computed_flow = computed_flow.unwrap();
        let rect = Rect {
            x: computed_flow.x,
            y: computed_flow.y,
            width: computed_flow.width,
            height: computed_flow.height,
        };

        let computed_style = element.computed_style.as_ref();
        if computed_style.is_none() {
            continue;
        }
        let computed_style = computed_style.unwrap();
        if computed_style.display == "none" || computed_style.visibility == "hidden" {
            continue;
        }

        let is_in_viewport = is_in_viewport(viewport, &rect);

        // Add the current element first (for correct z-order: parent before children)
        let computed_flow = element.computed_flow.as_ref().unwrap();
        let computed_style = element.computed_style.as_ref().unwrap();

        let has_something_to_render =
            element.node_value != "" || computed_style.background_color != (0.0, 0.0, 0.0, 0.0);

        if is_in_viewport {
            let style = element.inherited_style.as_ref().unwrap();

            match element.node_type {
                NodeType::Comment => {}
                _ => {
                    let item = RenderItem {
                        x: computed_flow.x,
                        y: computed_flow.y,
                        width: computed_flow.width,
                        height: computed_flow.height,
                        background_color: computed_style.background_color,
                        text_segments: element.text_segments.clone(),
                        font_size: computed_style.font_size,
                        font_path: style.font.get_path(),
                        color: computed_style.color,
                        underline: computed_style.text_decoration == "underline",
                        element: Some(tree[i].clone()),
                    };
                    array.push(item);
                }
            }
        }

        // Then add children (in DOM order, after parent)
        if element.children.len() > 0 && element.tag_name != "SCRIPT" && element.tag_name != "STYLE"
        {
            let children_render_items = get_render_array(&mut element.children, viewport);
            array.extend(children_render_items);
        }
    }

    return array;
}
