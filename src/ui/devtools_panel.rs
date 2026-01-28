use crate::devtools::{DevtoolsAgent, SelectionObserver};
use crate::events::EventSink;
use crate::dom::{DomElement, NodeType};
use crate::layout::Size;
use crate::renderer::{CompositeFrame, HybridRenderer};
use crate::ui::web_contents::WebContents;

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

pub struct DevtoolsPanel {
    web_contents: WebContents,
    agent: Rc<RefCell<DevtoolsAgent>>,
    loaded: bool,
    /// Self-reference for observer callback (set after construction)
    self_ref: Weak<RefCell<Self>>,
}

impl DevtoolsPanel {
    /// Create a new DevtoolsPanel wrapped in Rc<RefCell<>>, registered as observer.
    pub fn new(
        viewport: Size,
        frame_id: usize,
        overlay_frame_id: usize,
        agent: Rc<RefCell<DevtoolsAgent>>,
    ) -> Rc<RefCell<Self>> {
        let panel = Rc::new(RefCell::new(Self {
            web_contents: WebContents::new(viewport, frame_id, overlay_frame_id),
            agent: agent.clone(),
            loaded: false,
            self_ref: Weak::new(),
        }));

        // Set self-reference and register as observer
        panel.borrow_mut().self_ref = Rc::downgrade(&panel);
        agent.borrow_mut().add_observer(Rc::downgrade(&panel) as _);

        panel
    }

    /// Rebuild UI based on current agent state.
    fn rebuild_ui(&mut self) {
        let agent = self.agent.borrow();
        let selected = agent.get_selected_element();
        let main_dom_tree = agent.frame().map(|f| f.borrow().dom_tree.clone());
        drop(agent);

        if let Some(dom_tree) = main_dom_tree {
            self.update_content(selected.as_ref(), &dom_tree);
        }
    }

    /// Get the DevtoolsAgent
    pub fn agent(&self) -> &Rc<RefCell<DevtoolsAgent>> {
        &self.agent
    }

    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.web_contents.set_viewport(width, height);
    }

    /// Render the panel
    pub fn render(&mut self, renderer: &mut HybridRenderer) {
        self.web_contents.render(renderer);
    }

    /// Get composite frames for compositing
    pub fn get_composite_frames<'a>(&self, renderer: &'a HybridRenderer, dest_x: f32) -> Vec<CompositeFrame<'a>> {
        self.web_contents.get_composite_frames(renderer, dest_x)
    }

    /// Set render delegate
    pub fn set_render_delegate(&mut self, delegate: std::rc::Weak<dyn crate::frame::RenderDelegate>) {
        self.web_contents.set_render_delegate(delegate);
    }

    pub fn load(&mut self) {
        self.web_contents.frame_mut().load_url("devtools.html");
        self.loaded = true;
    }

    /// Show the panel and enable devtools inspection.
    pub fn show(&mut self) {
        self.agent.borrow_mut().enable();
    }

    /// Hide the panel and disable devtools inspection.
    pub fn hide(&mut self) {
        self.agent.borrow_mut().disable();
    }

    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    pub fn viewport(&self) -> Size {
        self.web_contents.frame().viewport.clone()
    }

    pub fn sink_rc(&self) -> Rc<RefCell<EventSink>> {
        self.web_contents.sink_rc()
    }

    pub fn scroll_y(&self) -> f32 {
        self.web_contents.scroll_y()
    }

    fn update_content(
        &mut self,
        selected_element: Option<&Rc<RefCell<DomElement>>>,
        main_dom_tree: &Vec<Rc<RefCell<DomElement>>>,
    ) {
        // Build breadcrumb (ancestor path)
        let breadcrumb_html = if let Some(node) = selected_element {
            build_breadcrumb(node)
        } else {
            String::new()
        };

        // Build DOM tree (focused on current element and siblings)
        let tree_html = if let Some(node) = selected_element {
            build_dom_tree(main_dom_tree, Some(node))
        } else {
            build_dom_tree(main_dom_tree, None)
        };

        // Build matched styles
        let styles_html = if let Some(node) = selected_element {
            build_matched_styles(node)
        } else {
            String::new()
        };

        // Build computed styles
        let computed_html = if let Some(node) = selected_element {
            build_computed_styles(node)
        } else {
            String::new()
        };

        // Build box dimensions
        let box_dims = if let Some(node) = selected_element {
            let el = node.borrow();
            el.computed_flow
                .as_ref()
                .map(|f| format!("{:.0} x {:.0}", f.width, f.height))
                .unwrap_or_else(|| "-- x --".to_string())
        } else {
            "-- x --".to_string()
        };

        // Update DOM elements
        let breadcrumb_el = self.web_contents.frame_mut().get_element_by_id("breadcrumb");
        let tree_el = self.web_contents.frame_mut().get_element_by_id("elements-tree");
        let styles_el = self.web_contents.frame_mut().get_element_by_id("matched-styles");
        let computed_el = self.web_contents.frame_mut().get_element_by_id("computed-styles");
        let dims_el = self.web_contents.frame_mut().get_element_by_id("box-dimensions");

        if let Some(el) = breadcrumb_el {
            el.borrow_mut().set_inner_html(&breadcrumb_html);
        }
        if let Some(el) = tree_el {
            el.borrow_mut().set_inner_html(&tree_html);
        }
        if let Some(el) = styles_el {
            el.borrow_mut().set_inner_html(&styles_html);
        }
        if let Some(el) = computed_el {
            el.borrow_mut().set_inner_html(&computed_html);
        }
        if let Some(el) = dims_el {
            el.borrow_mut().set_text_content(&box_dims);
        }
    }
}

// Helper: Build breadcrumb showing ancestor path
fn build_breadcrumb(node: &Rc<RefCell<DomElement>>) -> String {
    let mut path = Vec::new();
    let mut current = Some(node.clone());

    while let Some(el) = current {
        let el_ref = el.borrow();
        if el_ref.node_type == NodeType::Element {
            let tag = el_ref.tag_name.to_lowercase();
            if tag != "html" && tag != "#document" {
                let mut label = tag.clone();
                if let Some(id) = el_ref.attributes.get("id") {
                    label = format!("{}#{}", label, id);
                } else if !el_ref.class_list.is_empty() {
                    label = format!("{}.{}", label, el_ref.class_list.first().unwrap());
                }
                path.push(label);
            }
        }
        current = el_ref.parent_node.clone();
    }

    path.reverse();

    if path.is_empty() {
        return String::new();
    }

    let last_idx = path.len() - 1;
    path.iter()
        .enumerate()
        .map(|(i, p)| {
            let class = if i == last_idx { "crumb current" } else { "crumb" };
            format!("<span class=\"{}\">{}</span>", class, p)
        })
        .collect::<Vec<_>>()
        .join("<span class=\"crumb-sep\">></span>")
}

// Helper: Build DOM tree HTML
fn build_dom_tree(
    tree: &Vec<Rc<RefCell<DomElement>>>,
    selected: Option<&Rc<RefCell<DomElement>>>,
) -> String {
    fn render_node(
        node: &Rc<RefCell<DomElement>>,
        depth: usize,
        selected: Option<&Rc<RefCell<DomElement>>>,
        max_depth: usize,
    ) -> String {
        if depth > max_depth {
            return String::new();
        }

        let el = node.borrow();
        let indent = "  ".repeat(depth);

        let is_selected = selected
            .map(|s| Rc::ptr_eq(node, s))
            .unwrap_or(false);

        let node_class = if is_selected { "tree-node selected" } else { "tree-node" };

        match el.node_type {
            NodeType::Element => {
                let tag = el.tag_name.to_lowercase();

                // Skip script, style, head elements
                if tag == "script" || tag == "style" || tag == "head" {
                    return String::new();
                }

                // Build attributes string
                let mut attrs = String::new();
                if let Some(id) = el.attributes.get("id") {
                    attrs.push_str(&format!(" <span class=\"attr-name\">id</span>=<span class=\"attr-value\">\"{}\"</span>", id));
                }
                if !el.class_list.is_empty() {
                    attrs.push_str(&format!(" <span class=\"attr-name\">class</span>=<span class=\"attr-value\">\"{}\"</span>",
                        el.class_list.join(" ")));
                }

                let has_children = el.children.iter().any(|c| {
                    let c_ref = c.borrow();
                    c_ref.node_type == NodeType::Element
                });

                let toggle = if has_children { "▼" } else { " " };

                let mut html = format!(
                    "<div class=\"{}\"><span class=\"tree-toggle\">{}</span>{}&lt;<span class=\"tag-name\">{}</span>{}&gt;",
                    node_class, toggle, indent, tag, attrs
                );

                // Render children
                if has_children && depth < max_depth {
                    for child in &el.children {
                        html.push_str(&render_node(child, depth + 1, selected, max_depth));
                    }
                    html.push_str(&format!(
                        "<div class=\"tree-node\">{}&lt;/<span class=\"tag-name\">{}</span>&gt;</div>",
                        "  ".repeat(depth), tag
                    ));
                } else if has_children {
                    html.push_str("...");
                    html.push_str(&format!("&lt;/<span class=\"tag-name\">{}</span>&gt;</div>", tag));
                } else {
                    html.push_str(&format!("&lt;/<span class=\"tag-name\">{}</span>&gt;</div>", tag));
                }

                html
            }
            NodeType::Text => {
                let text = el.node_value.trim();
                if text.is_empty() || text.len() > 50 {
                    return String::new();
                }
                let escaped = text.replace("<", "&lt;").replace(">", "&gt;");
                let truncated = if escaped.len() > 30 {
                    format!("{}...", &escaped[..30])
                } else {
                    escaped
                };
                format!("<div class=\"tree-node\"><span class=\"tree-toggle\"> </span>{}<span class=\"text-content\">\"{}\"</span></div>",
                    indent, truncated)
            }
            _ => String::new(),
        }
    }

    let mut html = String::new();
    for node in tree {
        html.push_str(&render_node(node, 0, selected, 4));
    }
    html
}

// Helper: Build matched styles HTML
fn build_matched_styles(node: &Rc<RefCell<DomElement>>) -> String {
    let el = node.borrow();
    let mut html = String::new();

    for rule in &el.matched_styles {
        let selector = rule.selector.to_string();
        html.push_str(&format!("<div class=\"style-rule\"><div class=\"style-selector\">{}</div>", selector));

        for decl in &rule.declarations {
            let value_str = format!("{:?}", decl.value);
            // Clean up the debug output a bit
            let clean_value = value_str
                .replace("Multiple([", "")
                .replace("])", "")
                .replace("String(\"", "")
                .replace("\")", "");
            html.push_str(&format!(
                "<div class=\"style-prop\">{}: <span class=\"style-value\">{}</span>;</div>",
                decl.key, clean_value
            ));
        }
        html.push_str("</div>");
    }

    if html.is_empty() {
        html = "<div class=\"style-rule\">No matched styles</div>".to_string();
    }

    html
}

// Helper: Build computed styles HTML
fn build_computed_styles(node: &Rc<RefCell<DomElement>>) -> String {
    let el = node.borrow();
    let mut html = String::new();

    if let Some(style) = &el.computed_style {
        // Display
        html.push_str(&format_computed_prop("display", &style.display));
        html.push_str(&format_computed_prop("position", &style.position));
        html.push_str(&format_computed_prop("visibility", &style.visibility));
        html.push_str(&format_computed_prop("float", &style.float));

        // Dimensions
        if style.width > 0.0 {
            html.push_str(&format_computed_prop("width", &format!("{}px", style.width as i32)));
        }
        if style.height > 0.0 {
            html.push_str(&format_computed_prop("height", &format!("{}px", style.height as i32)));
        }

        // Font
        html.push_str(&format_computed_prop("font-family", &style.font_family));
        html.push_str(&format_computed_prop("font-size", &format!("{}px", style.font_size as i32)));
        html.push_str(&format_computed_prop("font-weight", &style.font_weight.to_string()));
        if style.font_style != "normal" {
            html.push_str(&format_computed_prop("font-style", &style.font_style));
        }

        // Color
        let (r, g, b, a) = style.color;
        let color_str = if a < 1.0 {
            format!("rgba({}, {}, {}, {:.2})", r as u8, g as u8, b as u8, a)
        } else {
            format!("rgb({}, {}, {})", r as u8, g as u8, b as u8)
        };
        html.push_str(&format_computed_prop_with_swatch("color", &color_str, style.color));

        // Background color
        let (bg_r, bg_g, bg_b, bg_a) = style.background_color;
        if bg_a > 0.0 {
            let bg_str = if bg_a < 1.0 {
                format!("rgba({}, {}, {}, {:.2})", bg_r as u8, bg_g as u8, bg_b as u8, bg_a)
            } else {
                format!("rgb({}, {}, {})", bg_r as u8, bg_g as u8, bg_b as u8)
            };
            html.push_str(&format_computed_prop_with_swatch("background-color", &bg_str, style.background_color));
        }

        // Margin
        let m = &style.margin;
        html.push_str(&format_computed_prop("margin-top", &format!("{}px", m.top as i32)));
        html.push_str(&format_computed_prop("margin-right", &format!("{}px", m.right as i32)));
        html.push_str(&format_computed_prop("margin-bottom", &format!("{}px", m.bottom as i32)));
        html.push_str(&format_computed_prop("margin-left", &format!("{}px", m.left as i32)));

        // Padding
        let p = &style.padding;
        html.push_str(&format_computed_prop("padding-top", &format!("{}px", p.top as i32)));
        html.push_str(&format_computed_prop("padding-right", &format!("{}px", p.right as i32)));
        html.push_str(&format_computed_prop("padding-bottom", &format!("{}px", p.bottom as i32)));
        html.push_str(&format_computed_prop("padding-left", &format!("{}px", p.left as i32)));

        // Border
        let b = &style.border;
        if b.top.width > 0.0 {
            html.push_str(&format_computed_prop("border-top", &format!("{}px {} rgb({}, {}, {})",
                b.top.width as i32, b.top.style, b.top.color.0 as u8, b.top.color.1 as u8, b.top.color.2 as u8)));
        }
        if b.right.width > 0.0 {
            html.push_str(&format_computed_prop("border-right", &format!("{}px {} rgb({}, {}, {})",
                b.right.width as i32, b.right.style, b.right.color.0 as u8, b.right.color.1 as u8, b.right.color.2 as u8)));
        }
        if b.bottom.width > 0.0 {
            html.push_str(&format_computed_prop("border-bottom", &format!("{}px {} rgb({}, {}, {})",
                b.bottom.width as i32, b.bottom.style, b.bottom.color.0 as u8, b.bottom.color.1 as u8, b.bottom.color.2 as u8)));
        }
        if b.left.width > 0.0 {
            html.push_str(&format_computed_prop("border-left", &format!("{}px {} rgb({}, {}, {})",
                b.left.width as i32, b.left.style, b.left.color.0 as u8, b.left.color.1 as u8, b.left.color.2 as u8)));
        }

        // Text decoration
        if style.text_decoration != "none" {
            html.push_str(&format_computed_prop("text-decoration", &style.text_decoration));
        }

        // White space
        if style.white_space != "normal" {
            html.push_str(&format_computed_prop("white-space", &style.white_space));
        }
    } else {
        html.push_str("<div class=\"computed-prop\">No computed styles</div>");
    }

    html
}

fn format_computed_prop(name: &str, value: &str) -> String {
    format!(
        "<div class=\"computed-prop\"><span class=\"computed-name\">{}</span>: <span class=\"computed-value\">{}</span></div>",
        name, value
    )
}

fn format_computed_prop_with_swatch(name: &str, value: &str, color: (f32, f32, f32, f32)) -> String {
    let (r, g, b, _) = color;
    format!(
        "<div class=\"computed-prop\"><span class=\"computed-name\">{}</span>: <span class=\"color-swatch\" style=\"background:rgb({},{},{})\"></span><span class=\"computed-value\">{}</span></div>",
        name, r as u8, g as u8, b as u8, value
    )
}

/// Observer implementation - rebuilds UI when selection changes.
impl SelectionObserver for RefCell<DevtoolsPanel> {
    fn on_selection_changed(&self) {
        self.borrow_mut().rebuild_ui();
    }
}
