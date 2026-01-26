// Frame - manages DOM, styles, layout, and fonts

use std::{cell::RefCell, fs, rc::Rc, time::Instant};

use crate::{
    css::parse_css,
    html::{parse_html, DomElement, NodeType},
    layout::{compute_styles, get_render_array, propagate_styles, reflow_and_cache, reflow_with_cache, Rect, RenderItem, boxes::LayoutNode},
    styles::StyleRule,
    text::FontManager,
};

/// Information about a hovered element is represented by its DOM node

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
}

impl Frame {
    pub fn new(viewport: Rect) -> Frame {
        let default_css =
            fs::read_to_string("default_styles.css").expect("error while reading default_styles.css");
        let default_styles = parse_css(&default_css);

        let mut frame = Frame {
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
        };

        frame.load_default_fonts();
        frame
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
        self.dom_tree = parse_html(&contents);

        let mut embedded_css = String::new();
        extract_style_tags(&self.dom_tree, &mut embedded_css);

        let embedded_styles = if !embedded_css.is_empty() {
            parse_css(&embedded_css)
        } else {
            vec![]
        };

        let external_styles = fs::read_to_string("style.css")
            .map(|style| parse_css(&style))
            .unwrap_or_else(|_| vec![]);

        self.parsed_css = [embedded_styles, external_styles].concat();
        self.styles = [self.default_styles.clone(), self.parsed_css.clone()].concat();

        self.cached_layout_tree = None;
        self.full_layout();
    }

    pub fn refresh(&mut self) {
        let url = self.url.clone();
        self.load_url(&url);
    }

    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.viewport.width = width;
        self.viewport.height = height;
    }

    /// Perform full layout and generate render array
    pub fn full_layout(&mut self) {
        let s = Instant::now();

        self.compute_styles();
        self.cached_layout_tree = None;
        self.reflow();
        self.build_render_array();

        println!("Full layout took: {:?}", s.elapsed());
    }

    /// Reflow layout (e.g., after viewport resize)
    pub fn reflow(&mut self) {
        let s = Instant::now();

        if let Some(ref mut cached_tree) = self.cached_layout_tree {
            reflow_with_cache(&mut self.dom_tree, &mut self.font_manager, None, &self.viewport, Some(cached_tree));
            println!("Reflow (cached) took: {:?}", s.elapsed());
        } else {
            let layout_tree = reflow_and_cache(&mut self.dom_tree, &mut self.font_manager, &self.viewport);
            self.cached_layout_tree = Some(layout_tree);
            println!("Reflow (full) took: {:?}", s.elapsed());
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
            "Build render array took: {:?}, items: {:?}, page_height: {:?}",
            s.elapsed(),
            self.render_array.len(),
            self.page_height
        );
    }

    fn compute_styles(&mut self) {
        let s = Instant::now();
        compute_styles(&mut self.dom_tree, &self.styles, &mut vec![], None);
        println!("Computing styles took: {:?}", s.elapsed());
        let s = Instant::now();
        propagate_styles(&mut self.dom_tree, None);
        println!("Propagating styles took: {:?}", s.elapsed());
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
