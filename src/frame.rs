// Frame - manages DOM, styles, layout, and fonts

use std::{cell::{Cell, RefCell}, fs, rc::{Rc, Weak}, time::Instant};

use crate::{
    css::parse_css,
    dom::{DomElement, ElementKind, NodeType},
    html::{parse_html, HtmlNode, HtmlNodeType},
    layout::{compute_styles, create_layout_tree, get_render_array, propagate_styles, reflow, relayout, Rect, RenderItem, boxes::LayoutNode},
    styles::StyleRule,
    text::FontManager,
};

/// Recursively extract CSS content from <style> tags in the DOM tree
fn extract_style_tags(tree: &Vec<Rc<RefCell<DomElement>>>, css: &mut String) {
    for element in tree {
        let el = element.borrow();

        if el.tag_name == "STYLE" {
            for child in &el.children {
                let child_el = child.borrow();
                if child_el.node_type == NodeType::Text {
                    css.push_str(&child_el.node_value);
                    css.push('\n');
                }
            }
        }

        if !el.children.is_empty() {
            extract_style_tags(&el.children, css);
        }
    }
}

/// Frame manages the document structure, styles, layout, and fonts
/// It produces RenderItems that can be passed to any renderer
pub struct Frame {
    pub viewport: Rect,
    pub render_array: Vec<RenderItem>,
    pub page_height: f32,
    pub dom_tree: Vec<Rc<RefCell<DomElement>>>,
    pub parsed_css: Vec<StyleRule>,
    pub styles: Vec<StyleRule>,
    pub url: String,
    pub font_manager: FontManager,
    pub default_styles: Vec<StyleRule>,
    pub cached_layout_tree: Option<Vec<LayoutNode>>,
    /// Dirty flag for style recomputation
    styles_dirty: Cell<bool>,
    /// Delegate to notify when render is needed
    render_delegate: Option<Weak<dyn RenderDelegate>>,
    /// Weak self-reference for passing to DomElements
    self_ref: Option<Weak<RefCell<Frame>>>,
}

/// Trait for objects that can handle render requests from Frame
pub trait RenderDelegate {
    fn request_redraw(&self);
}

impl Frame {
    pub fn new(viewport: Rect) -> Rc<RefCell<Frame>> {
        let default_css =
            fs::read_to_string("default_styles.css").expect("error while reading default_styles.css");
        let default_styles = parse_css(&default_css);

        let frame = Rc::new(RefCell::new(Frame {
            viewport,
            render_array: vec![],
            page_height: 0.0,
            dom_tree: vec![],
            parsed_css: vec![],
            styles: vec![],
            url: "".to_string(),
            font_manager: FontManager::new(),
            default_styles,
            cached_layout_tree: None,
            styles_dirty: Cell::new(false),
            render_delegate: None,
            self_ref: None,
        }));

        // Set up weak self-reference
        frame.borrow_mut().self_ref = Some(Rc::downgrade(&frame));
        frame.borrow_mut().load_default_fonts();
        frame
    }

    /// Called by DomElements when they change.
    /// This method takes &self (not &mut self) so it can be called while Frame is borrowed.
    pub fn mark_styles_dirty(&self) {
        self.styles_dirty.set(true);
        self.request_redraw();
    }

    /// Set the delegate to be notified when re-render is needed
    pub fn set_render_delegate(&mut self, delegate: Weak<dyn RenderDelegate>) {
        self.render_delegate = Some(delegate);
    }

    /// Request a redraw from the delegate (public for direct Frame modifications)
    pub fn request_redraw(&self) {
        if let Some(ref weak) = self.render_delegate {
            if let Some(delegate) = weak.upgrade() {
                delegate.request_redraw();
            }
        }
    }

    /// Get a short display name for logging (derived from URL)
    fn log_name(&self) -> &str {
        if self.url.is_empty() {
            return "frame";
        }
        // Get filename from path, or last path segment from URL
        self.url
            .rsplit('/')
            .next()
            .unwrap_or(&self.url)
    }

    /// Create a new element with a reference to this frame
    pub fn create_element(&self, tag_name: &str) -> Rc<RefCell<DomElement>> {
        let frame_ref = self.self_ref.clone().expect("Frame self_ref not set");
        let mut el = DomElement::new(NodeType::Element, frame_ref);
        el.tag_name = tag_name.to_uppercase();
        el.element_kind = ElementKind::for_tag(&el.tag_name);
        Rc::new(RefCell::new(el))
    }

    /// Create a text node with a reference to this frame
    pub fn create_text_node(&self, text: &str) -> Rc<RefCell<DomElement>> {
        let frame_ref = self.self_ref.clone().expect("Frame self_ref not set");
        let mut el = DomElement::new(NodeType::Text, frame_ref);
        el.node_value = text.to_string();
        Rc::new(RefCell::new(el))
    }

    /// Load default fonts from assets folder
    fn load_default_fonts(&mut self) {
        let assets = find_folder::Search::ParentsThenKids(3, 3)
            .for_folder("assets")
            .expect("Could not find assets folder");

        let font_files = [
            "Times New Roman 400.ttf",
            "Times New Roman 700.ttf",
            "Times New Roman Italique 400.ttf",
            "Times New Roman Italique 700.ttf",
        ];

        for font_file in &font_files {
            let path = assets.join(font_file);
            if let Some(path_str) = path.to_str() {
                if path.exists() {
                    self.font_manager.load_font(font_file, path_str);
                }
            }
        }
    }

    pub fn load_url(&mut self, url: &str) {
        self.url = url.to_string();
        let contents = fs::read_to_string(&self.url).expect("error while reading the file");
        self.load_html_internal(&contents, true);
    }

    pub fn load_html(&mut self, html: &str) {
        self.url = String::new();
        self.load_html_internal(html, false);
    }

    fn load_html_internal(&mut self, html: &str, load_external_css: bool) {
        let ir_nodes = parse_html(html);
        self.dom_tree = self.build_dom_from_ir(&ir_nodes, None);

        let mut embedded_css = String::new();
        extract_style_tags(&self.dom_tree, &mut embedded_css);

        let embedded_styles = if !embedded_css.is_empty() {
            parse_css(&embedded_css)
        } else {
            vec![]
        };

        let external_styles = if load_external_css {
            fs::read_to_string("style.css")
                .map(|style| parse_css(&style))
                .unwrap_or_else(|_| vec![])
        } else {
            vec![]
        };

        self.parsed_css = [embedded_styles, external_styles].concat();
        self.styles = [self.default_styles.clone(), self.parsed_css.clone()].concat();

        self.cached_layout_tree = None;
        self.full_layout();
    }

    /// Build DomElements from the HtmlNode intermediate representation
    pub fn build_dom_from_ir(
        &self,
        ir_nodes: &[HtmlNode],
        parent: Option<Rc<RefCell<DomElement>>>,
    ) -> Vec<Rc<RefCell<DomElement>>> {
        let frame_ref = self.self_ref.clone().expect("Frame self_ref not set - call set_self_ref first");

        let mut result = Vec::new();
        for ir_node in ir_nodes {
            let node_type = match ir_node.node_type {
                HtmlNodeType::Element => NodeType::Element,
                HtmlNodeType::Text => NodeType::Text,
                HtmlNodeType::DocumentType => NodeType::DocumentType,
                HtmlNodeType::Comment => NodeType::Comment,
            };

            let mut element = DomElement::new(node_type.clone(), frame_ref.clone());

            match node_type {
                NodeType::Element => {
                    element.tag_name = ir_node.tag_name.clone();
                    element.element_kind = ElementKind::for_tag(&element.tag_name);

                    for (key, value) in &ir_node.attributes {
                        element.attributes.insert(key.clone(), value.clone());

                        if key == "class" {
                            element.class_list = value.split_whitespace().map(|s| s.to_string()).collect();
                        } else if key == "style" {
                            let val = format!("{{{}}}", value);
                            let rules = parse_css(&val);
                            for rule in rules {
                                for mut decl in rule.declarations {
                                    decl.important = true;
                                    element.inline_declarations.push(decl);
                                }
                            }
                        }
                    }
                }
                NodeType::Text => {
                    element.node_value = ir_node.text_content.clone();
                }
                NodeType::Comment => {
                    element.node_value = ir_node.text_content.clone();
                }
                NodeType::DocumentType => {
                    element.node_value = ir_node.text_content.clone();
                }
            }

            element.parent_node = parent.clone();
            let element_rc = Rc::new(RefCell::new(element));

            if !ir_node.children.is_empty() {
                let children = self.build_dom_from_ir(&ir_node.children, Some(element_rc.clone()));
                element_rc.borrow_mut().children = children;
            }

            result.push(element_rc);
        }
        result
    }

    pub fn refresh(&mut self) {
        let url = self.url.clone();
        self.load_url(&url);
    }

    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.viewport.width = width;
        self.viewport.height = height;

        let s = Instant::now();
        self.ensure_layout_tree();
        if let Some(ref mut tree) = self.cached_layout_tree {
            relayout(tree, &self.viewport);
            println!("[{}] Resize: {:?}", self.log_name(), s.elapsed());
        }
        self.build_render_array();
    }

    /// Perform full layout and generate render array
    pub fn full_layout(&mut self) {
        let s = Instant::now();

        self.compute_styles();
        self.cached_layout_tree = None;
        self.do_reflow();
        self.build_render_array();

        println!("[{}] Full layout: {:?}", self.log_name(), s.elapsed());
    }

    /// Ensure layout tree exists, creating it if needed.
    /// Returns true if tree was freshly created (already measured).
    fn ensure_layout_tree(&mut self) -> bool {
        if self.cached_layout_tree.is_none() {
            let s = Instant::now();
            let tree = create_layout_tree(&mut self.dom_tree, &mut self.font_manager, &self.viewport);
            self.cached_layout_tree = Some(tree);
            println!("[{}] Build layout tree: {:?}", self.log_name(), s.elapsed());
            true
        } else {
            false
        }
    }

    /// Reflow with remeasurement (when styles changed).
    fn do_reflow(&mut self) {
        let s = Instant::now();
        let freshly_built = self.ensure_layout_tree();
        if let Some(ref mut tree) = self.cached_layout_tree {
            if freshly_built {
                // Tree was just built and measured, only need layout pass
                relayout(tree, &self.viewport);
            } else {
                // Existing tree, need to remeasure
                reflow(tree, &mut self.font_manager, &self.viewport);
            }
            println!("[{}] Reflow: {:?}", self.log_name(), s.elapsed());
        }
    }

    /// Build the render array from the laid-out DOM
    pub fn build_render_array(&mut self) {
        let s = Instant::now();

        // Use a very tall viewport to get all items
        let mut full_viewport = self.viewport.clone();
        full_viewport.y = 0.0;
        full_viewport.height = 100000.0;

        self.render_array = get_render_array(&mut self.dom_tree, &full_viewport)
            .into_iter()
            .collect();

        // Calculate total page height
        self.page_height = self.render_array.iter()
            .map(|item| item.y + item.height)
            .fold(0.0_f32, |a, b| a.max(b));

        println!(
            "[{}] Redraw: {:?}, items: {:?}, page_height: {:?}",
            self.log_name(),
            s.elapsed(),
            self.render_array.len(),
            self.page_height
        );
    }

    /// Compute styles for all elements and return (needs_rebuild, needs_reflow).
    /// - needs_rebuild: display changed, layout tree must be rebuilt
    /// - needs_reflow: layout values changed, positions must be recalculated
    fn compute_styles(&mut self) -> (bool, bool) {
        use crate::layout::pipeline::evaluate_styles_recursive;

        let s = Instant::now();
        compute_styles(&mut self.dom_tree, &self.styles, &mut vec![], None);
        println!("[{}] Computing styles: {:?}", self.log_name(), s.elapsed());
        let s = Instant::now();
        propagate_styles(&mut self.dom_tree, None);
        println!("[{}] Propagating styles: {:?}", self.log_name(), s.elapsed());

        // Evaluate all scalar values (margin: 1em, width: 50%, etc.)
        // This must happen before to_computed_style() is called
        evaluate_styles_recursive(
            &mut self.dom_tree,
            16.0,
            self.viewport.width,
            self.viewport.height,
        );

        // Update computed_style for all elements and check what kind of layout is needed
        Self::update_computed_styles(&mut self.dom_tree)
    }

    /// Update computed_style from inherited_style for all elements.
    /// Returns (needs_rebuild, needs_reflow).
    fn update_computed_styles(tree: &mut [Rc<RefCell<DomElement>>]) -> (bool, bool) {
        let mut needs_rebuild = false;
        let mut needs_reflow = false;

        for elem in tree {
            let mut elem_ref = elem.borrow_mut();

            if let Some(ref inherited) = elem_ref.inherited_style {
                let new_computed = inherited.to_computed_style();

                // Compare with old computed style to see what kind of layout is needed
                if let Some(ref old_computed) = elem_ref.computed_style {
                    if old_computed.needs_rebuild(&new_computed) {
                        needs_rebuild = true;
                    }
                    if old_computed.needs_reflow(&new_computed) {
                        needs_reflow = true;
                    }
                } else {
                    // No previous computed style means this is new, needs full rebuild
                    needs_rebuild = true;
                }

                elem_ref.computed_style = Some(new_computed);
            }

            // Recursively update children
            let (child_rebuild, child_reflow) = Self::update_computed_styles(&mut elem_ref.children);
            if child_rebuild {
                needs_rebuild = true;
            }
            if child_reflow {
                needs_reflow = true;
            }
        }

        (needs_rebuild, needs_reflow)
    }

    /// Get the render items for rendering
    pub fn render_items(&self) -> &[RenderItem] {
        &self.render_array
    }

    /// Get the total page height
    pub fn get_page_height(&self) -> f32 {
        self.page_height
    }

    /// Get access to font manager for rendering
    pub fn fonts(&self) -> &FontManager {
        &self.font_manager
    }

    /// Hit test to find the element at a given point (in page coordinates)
    /// Returns the deepest element that contains the point
    pub fn hit_test(&self, x: f32, y: f32) -> Option<Rc<RefCell<DomElement>>> {
        use crate::layout::rect_contains;

        // Traverse DOM tree to find deepest element at point
        fn hit_test_tree(
            tree: &Vec<Rc<RefCell<DomElement>>>,
            x: f32,
            y: f32,
        ) -> Option<Rc<RefCell<DomElement>>> {
            let mut result: Option<Rc<RefCell<DomElement>>> = None;

            for element_rc in tree {
                let element = element_rc.borrow();

                // Skip text nodes
                if element.node_type == NodeType::Text {
                    continue;
                }

                if let Some(ref flow) = element.computed_flow {
                    // Use hover_rect (margin box) for hit testing
                    if rect_contains(&flow.hover_rect, x, y) {
                        result = Some(element_rc.clone());
                    }
                }

                // Always check children - they might be:
                // 1. Positioned outside parent bounds (overflow, absolute positioning)
                // 2. Inside a parent with incorrect/stale computed_flow
                // 3. Inside a parent with zero dimensions
                // We keep the deepest match that contains the point.
                if let Some(child_hit) = hit_test_tree(&element.children, x, y) {
                    result = Some(child_hit);
                }
            }

            result
        }

        hit_test_tree(&self.dom_tree, x, y)
    }

    /// Recompute styles and rebuild render array
    /// Does a full layout since hover can change any property including layout-affecting ones
    pub fn restyle(&mut self) {
        self.cached_layout_tree = None;
        self.full_layout();
    }

    /// Update styles if marked dirty. Called before rendering.
    /// Returns true if the frame was updated.
    pub fn update_styles_if_needed(&mut self) -> bool {
        if !self.styles_dirty.get() {
            return false;
        }
        self.styles_dirty.set(false);

        // Recompute styles for the whole tree and check what kind of layout is needed
        let (needs_rebuild, needs_reflow) = self.compute_styles();

        if needs_rebuild {
            // Display changed - layout tree structure must be rebuilt
            self.cached_layout_tree = None;
            self.do_reflow();
        } else if needs_reflow {
            // Layout values changed - refresh styles in cached tree and reflow
            if let Some(ref mut cached_tree) = self.cached_layout_tree {
                // Refresh margin/padding from updated computed_style
                for node in cached_tree.iter_mut() {
                    node.refresh_styles();
                }
            }
            self.do_reflow();
        }
        // If only visual properties changed, computed_style is already updated
        // and we just need to rebuild the render array

        self.build_render_array();
        true
    }

    /// Mutate a DOM node and trigger full layout
    pub fn mutate_dom<F>(&mut self, node: &Rc<RefCell<DomElement>>, f: F)
    where
        F: FnOnce(&mut DomElement),
    {
        f(&mut node.borrow_mut());
        self.full_layout();
    }

    /// Find an element by id attribute
    pub fn get_element_by_id(&self, id: &str) -> Option<Rc<RefCell<DomElement>>> {
        fn find_in_tree(
            tree: &Vec<Rc<RefCell<DomElement>>>,
            id: &str,
        ) -> Option<Rc<RefCell<DomElement>>> {
            for node in tree {
                let node_ref = node.borrow();
                if let Some(value) = node_ref.attributes.get("id") {
                    if value == id {
                        return Some(node.clone());
                    }
                }
                if !node_ref.children.is_empty() {
                    if let Some(found) = find_in_tree(&node_ref.children, id) {
                        return Some(found);
                    }
                }
            }
            None
        }

        find_in_tree(&self.dom_tree, id)
    }
}
