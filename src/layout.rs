use crate::colors::*;
use crate::css::*;
use crate::css_value::CssValue;
use crate::dom::{DomElement, NodeType, TextSegment};
use crate::text::TextMeasurer;
use crate::styles::*;
use crate::utils::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Context for sibling-based CSS selectors (+ combinator, ~ combinator, :nth-child, etc.)
#[derive(Clone, Debug)]
pub struct SiblingContext {
    pub index: usize,                    // Current element's index among siblings
    pub total: usize,                    // Total number of siblings
    pub siblings: Vec<*mut DomElement>,  // All siblings for ~ combinator
}

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

#[derive(Clone, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub fn to_rect(&self) -> Rect {
        Rect { x: 0.0, y: 0.0, width: self.width, height: self.height }
    }
}

use crate::properties::border::ComputedBorder;
use crate::properties::box_shadow::ComputedBoxShadow;

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
    pub border: ComputedBorder,
    pub box_shadow: ComputedBoxShadow,
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

/// Check if rect is completely below the viewport (can skip descendants for vertical layouts)
#[inline]
pub fn is_below_viewport(rect: &Rect, viewport: &Rect) -> bool {
    rect.y > viewport.y + viewport.height
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
    element_matches_selector_with_siblings(element, selector, parents, None)
}

pub fn element_matches_selector_with_siblings(
    element: &DomElement,
    selector: &CssSelector,
    parents: &[*mut DomElement],
    siblings: Option<&SiblingContext>,
) -> bool {
    match selector {
        CssSelector::Tag(tag) => element.tag_name.eq_ignore_ascii_case(tag) || tag == "*",
        CssSelector::Id(id) => element.attributes.get("id").map_or(false, |v| v == id),
        CssSelector::Class(class) => {
            element.class_list.iter().any(|c| c == class)
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
            match pseudo.to_lowercase().as_str() {
                "hover" => element.pseudo_classes.hover,
                "focus" => element.pseudo_classes.focus,
                "active" => element.pseudo_classes.active,
                "first-child" => siblings.map_or(false, |ctx| ctx.index == 0),
                "last-child" => siblings.map_or(false, |ctx| ctx.index == ctx.total - 1),
                "only-child" => siblings.map_or(false, |ctx| ctx.total == 1),
                "empty" => element.children.is_empty(),
                _ => {
                    // Handle nth-child(expr) and not(selector)
                    if pseudo.starts_with("nth-child(") && pseudo.ends_with(")") {
                        let expr = &pseudo[10..pseudo.len() - 1];
                        parse_and_match_nth_child(expr, siblings)
                    } else if pseudo.starts_with("not(") && pseudo.ends_with(")") {
                        parse_and_match_not(pseudo, element, parents, siblings)
                    } else {
                        false
                    }
                }
            }
        }
        CssSelector::PseudoElement(_pseudo) => {
            // Handle pseudo-elements
            false // For now, we just return false
        }
        CssSelector::Combinator {
            combinator,
            selectors,
        } => {
            // For a combinator like ".a .b .c", selectors = [Combinator(".a .b"), Class("c")]
            // The rightmost selector must match the current element
            // The rest must match ancestors
            if selectors.is_empty() {
                return false;
            }

            // Split: rightmost selector matches element, rest match ancestors
            let (ancestor_selectors, element_selector) = selectors.split_at(selectors.len() - 1);

            // First check if element matches the rightmost selector
            if !element_matches_selector_with_siblings(element, &element_selector[0], parents, siblings) {
                return false;
            }

            // If no ancestor requirements, we're done
            if ancestor_selectors.is_empty() {
                return true;
            }

            // Check ancestors/siblings based on combinator type
            match combinator.as_str() {
                ">" => {
                    // Direct parent must match all ancestor selectors
                    parents.last().map_or(false, |parent| {
                        ancestor_selectors.iter().all(|selector| {
                            element_matches_selector_with_siblings(
                                unsafe { &**parent },
                                selector,
                                &parents[..parents.len() - 1],
                                None,
                            )
                        })
                    })
                }
                " " => {
                    // Some ancestor must match all ancestor selectors
                    for (i, parent) in parents.iter().enumerate().rev() {
                        let all_match = ancestor_selectors.iter().all(|selector| {
                            element_matches_selector_with_siblings(
                                unsafe { &**parent },
                                selector,
                                &parents[..i],
                                None,
                            )
                        });
                        if all_match {
                            return true;
                        }
                    }
                    false
                }
                "+" => {
                    // Adjacent sibling: previous sibling must match
                    siblings.map_or(false, |ctx| {
                        if ctx.index == 0 {
                            return false;
                        }
                        let prev = ctx.siblings[ctx.index - 1];
                        ancestor_selectors.iter().all(|sel| {
                            element_matches_selector_with_siblings(
                                unsafe { &*prev },
                                sel,
                                parents,
                                None,
                            )
                        })
                    })
                }
                "~" => {
                    // General sibling: any preceding sibling must match
                    siblings.map_or(false, |ctx| {
                        for i in 0..ctx.index {
                            let sib = ctx.siblings[i];
                            let all_match = ancestor_selectors.iter().all(|sel| {
                                element_matches_selector_with_siblings(
                                    unsafe { &*sib },
                                    sel,
                                    parents,
                                    None,
                                )
                            });
                            if all_match {
                                return true;
                            }
                        }
                        false
                    })
                }
                _ => false,
            }
        }
        CssSelector::OrGroup { selectors } => {
            selectors.len() > 0
                && selectors
                    .iter()
                    .any(|s| element_matches_selector_with_siblings(element, s, parents, siblings))
        }
        CssSelector::AndGroup { selectors } => {
            selectors.len() > 0
                && selectors
                    .iter()
                    .all(|s| element_matches_selector_with_siblings(element, s, parents, siblings))
        }
        _ => false,
    }
}

/// Parse an+b formula from :nth-child() expression
/// Returns (a, b) where the formula is an+b
fn parse_nth_child(expr: &str) -> Option<(i32, i32)> {
    let expr = expr.trim().to_lowercase();

    match expr.as_str() {
        "odd" => return Some((2, 1)),
        "even" => return Some((2, 0)),
        _ => {}
    }

    // Handle simple number case: "3" means (0, 3)
    if let Ok(b) = expr.parse::<i32>() {
        return Some((0, b));
    }

    // Handle an+b or an-b patterns
    if expr.contains('n') {
        let expr = expr.replace(" ", "");

        // Split on 'n'
        let parts: Vec<&str> = expr.split('n').collect();
        if parts.len() != 2 {
            return None;
        }

        // Parse 'a' coefficient
        let a: i32 = match parts[0] {
            "" | "+" => 1,
            "-" => -1,
            s => s.parse().ok()?,
        };

        // Parse 'b' offset
        let b: i32 = if parts[1].is_empty() {
            0
        } else {
            parts[1].parse().ok()?
        };

        Some((a, b))
    } else {
        None
    }
}

/// Check if 1-indexed position n matches the formula an+b
fn matches_nth(index: usize, a: i32, b: i32) -> bool {
    let n = (index + 1) as i32; // Convert to 1-indexed
    if a == 0 {
        return n == b;
    }
    let diff = n - b;
    // n = ak + b for some non-negative integer k
    // k = (n - b) / a, and k must be >= 0
    if a > 0 {
        diff >= 0 && diff % a == 0
    } else {
        diff <= 0 && diff % a == 0
    }
}

fn parse_and_match_nth_child(expr: &str, siblings: Option<&SiblingContext>) -> bool {
    let siblings = match siblings {
        Some(ctx) => ctx,
        None => return false,
    };

    match parse_nth_child(expr) {
        Some((a, b)) => matches_nth(siblings.index, a, b),
        None => false,
    }
}

fn parse_and_match_not(
    pseudo: &str,
    element: &DomElement,
    parents: &[*mut DomElement],
    siblings: Option<&SiblingContext>,
) -> bool {
    // Extract selector from "not(...)"
    let inner = &pseudo[4..pseudo.len() - 1];
    let tokens = tokenize_css_selector(inner);
    let selector = parse_css_selector(&tokens);
    !element_matches_selector_with_siblings(element, &selector, parents, siblings)
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

    // Build sibling context for this level
    let siblings_vec: Vec<*mut DomElement> = tree
        .iter()
        .map(|el| el.as_ptr() as *mut DomElement)
        .collect();
    let total_siblings = tree.len();

    for i in 0..tree.len() {
        let mut element = tree[i].borrow_mut();

        let sibling_ctx = SiblingContext {
            index: i,
            total: total_siblings,
            siblings: siblings_vec.clone(),
        };

        // Clear previous matched styles for re-computation
        element.matched_styles.clear();
        element.style = Style::new();

        // Apply pre-parsed inline style declarations (already marked important)
        let inline_decls = element.inline_declarations.clone();
        if !inline_decls.is_empty() {
            element.style.insert_declarations(&inline_decls, var_ctx.as_ref().unwrap());
        }

        // Use index to get candidate rules, or fall back to all rules
        let candidates: Vec<usize> = match index {
            Some(idx) => idx.get_candidates(&element),
            None => (0..style.len()).collect(),
        };

        for rule_idx in candidates {
            let style_rule = &style[rule_idx];
            if element_matches_selector_with_siblings(&element, &style_rule.selector, parents, Some(&sibling_ctx)) {
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

/// Build a new layout tree from the DOM tree.
/// This creates the tree structure and measures all nodes.
/// Call reflow() afterwards to compute positions.
pub fn create_layout_tree(
    dom_tree: &mut Vec<Rc<RefCell<DomElement>>>,
    text_measurer: &mut dyn TextMeasurer,
    viewport: &Rect,
) -> Vec<boxes::LayoutNode> {
    use crate::layout::pipeline::{build_layout_tree, measure_layout_tree};

    let context = ReflowContext {
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

    let mut layout_tree = build_layout_tree(dom_tree, &context);
    measure_layout_tree(&mut layout_tree, text_measurer, viewport.width);
    layout_tree
}

/// Reflow: remeasure text and recalculate layout.
/// Use when styles changed (font-size, etc.).
pub fn reflow(
    layout_tree: &mut Vec<boxes::LayoutNode>,
    text_measurer: &mut dyn TextMeasurer,
    viewport: &Rect,
) {
    use crate::layout::pipeline::{
        finalize_layout_tree, layout_tree as layout_tree_pass, measure_layout_tree,
        reset_layout_positions,
    };

    let context = make_reflow_context(viewport);
    reset_layout_positions(layout_tree);
    measure_layout_tree(layout_tree, text_measurer, viewport.width);
    layout_tree_pass(layout_tree, &context);
    finalize_layout_tree(layout_tree);
}

/// Relayout: recalculate positions without remeasuring text.
/// Use for viewport resize when styles haven't changed.
pub fn relayout(
    layout_tree: &mut Vec<boxes::LayoutNode>,
    viewport: &Rect,
) {
    use crate::layout::pipeline::{
        finalize_layout_tree, layout_tree as layout_tree_pass, reset_layout_positions,
    };

    let context = make_reflow_context(viewport);
    reset_layout_positions(layout_tree);
    layout_tree_pass(layout_tree, &context);
    finalize_layout_tree(layout_tree);
}

fn make_reflow_context(viewport: &Rect) -> ReflowContext {
    ReflowContext {
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
    }
}

pub fn get_render_array(
    tree: &mut Vec<Rc<RefCell<DomElement>>>,
    viewport: &Rect,
) -> Vec<RenderItem> {
    let mut stats = RenderStats::default();
    get_render_array_inner(tree, viewport, &mut stats)
}

#[derive(Default)]
struct RenderStats {
    traversed: usize,
    in_viewport: usize,
    skipped_below: usize,
    skipped_no_flow: usize,
}

fn get_render_array_inner(
    tree: &mut Vec<Rc<RefCell<DomElement>>>,
    viewport: &Rect,
    stats: &mut RenderStats,
) -> Vec<RenderItem> {
    let mut array: Vec<RenderItem> = vec![];

    for i in 0..tree.len() {
        stats.traversed += 1;
        let element = &mut tree[i].borrow_mut();
        let computed_flow = element.computed_flow.as_ref();
        if computed_flow.is_none() {
            stats.skipped_no_flow += 1;
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
            stats.skipped_no_flow += 1;
            continue;
        }
        let computed_style = computed_style.unwrap();
        if computed_style.display == "none" || computed_style.visibility == "hidden" {
            continue;
        }

        // Early termination: if element is completely below viewport, skip it and all children
        // (For vertical layouts, children are typically within parent bounds)
        if is_below_viewport(&rect, viewport) {
            stats.skipped_below += 1;
            continue;
        }

        let in_viewport = is_in_viewport(viewport, &rect);

        // Add the current element first (for correct z-order: parent before children)
        let computed_flow = element.computed_flow.as_ref().unwrap();
        let computed_style = element.computed_style.as_ref().unwrap();

        // Check if background is visible (has color AND rect overlaps viewport)
        let has_visible_background = computed_style.background_color.3 > 0.0 && in_viewport;

        // Filter text segments to only those in viewport
        let visible_text_segments: Vec<_> = element.text_segments.iter()
            .filter(|seg| {
                let seg_bottom = seg.y + seg.height;
                seg_bottom > viewport.y && seg.y < viewport.y + viewport.height
            })
            .cloned()
            .collect();

        // Only create RenderItem if there's something visible to render
        let has_visible_border = computed_style.border.has_visible_border();
        let has_visible_shadow = computed_style.box_shadow.is_visible();
        if has_visible_background || has_visible_border || has_visible_shadow || !visible_text_segments.is_empty() {
            stats.in_viewport += 1;

            if element.node_type != NodeType::Comment {
                let style = element.inherited_style.as_ref().unwrap();
                let item = RenderItem {
                    x: computed_flow.x,
                    y: computed_flow.y,
                    width: computed_flow.width,
                    height: computed_flow.height,
                    background_color: if has_visible_background {
                        computed_style.background_color
                    } else {
                        (0.0, 0.0, 0.0, 0.0)
                    },
                    text_segments: visible_text_segments,
                    font_size: computed_style.font_size,
                    font_path: style.font.get_path(),
                    color: computed_style.color,
                    underline: computed_style.text_decoration == "underline",
                    border: computed_style.border.clone(),
                    box_shadow: computed_style.box_shadow.clone(),
                    element: Some(tree[i].clone()),
                };
                array.push(item);
            }
        }

        // Always recurse into children (they may be visible even if parent box isn't)
        if element.children.len() > 0
            && element.tag_name != "SCRIPT"
            && element.tag_name != "STYLE"
        {
            let children_render_items = get_render_array_inner(&mut element.children, viewport, stats);
            array.extend(children_render_items);
        }
    }

    return array;
}
