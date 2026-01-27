use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use crate::css::parse_css;
use crate::frame::Frame;
use crate::layout::{CssVariablesContext, Rect};
use crate::styles::{ComputedStyle, Declaration, Style, StyleRule};

#[derive(Clone, Debug, PartialEq)]
pub enum NodeType {
    Element,
    Text,
    DocumentType,
    Comment,
}

#[derive(Clone, Debug)]
pub struct ComputedFlow {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub adjacent_margin_bottom: f32,
    pub hover_rect: Rect,
    pub continue_x: f32,
    pub continue_y: f32,
}

/// A preprocessed text segment (word) with measured dimensions and position
#[derive(Clone, Debug)]
pub struct TextSegment {
    pub text: String,
    pub width: f32,
    pub height: f32,
    /// Distance from top of text box to baseline
    pub ascent: f32,
    // Position set during layout:
    pub x: f32,
    pub y: f32,
}

pub struct DomEvent {
    default_prevented: bool,
}

impl DomEvent {
    pub fn new() -> Self {
        Self { default_prevented: false }
    }

    pub fn prevent_default(&mut self) {
        self.default_prevented = true;
    }

    pub fn default_prevented(&self) -> bool {
        self.default_prevented
    }
}

#[derive(Clone, Debug, Default)]
pub struct PseudoClassState {
    pub hover: bool,
    pub focus: bool,
    pub active: bool,
}

pub trait HTMLElement {
    fn on_click(&mut self, element: &mut DomElement, event: &mut DomEvent);
}

#[derive(Clone, Debug, Default)]
pub struct HTMLInputElement {
    pub clicks: u32,
}

impl HTMLElement for HTMLInputElement {
    fn on_click(&mut self, _element: &mut DomElement, _event: &mut DomEvent) {
        self.clicks += 1;
    }
}

#[derive(Clone, Debug, Default)]
pub struct HTMLCustomRenderElement;

impl HTMLElement for HTMLCustomRenderElement {
    fn on_click(&mut self, _element: &mut DomElement, _event: &mut DomEvent) {}
}

#[derive(Clone, Debug, Default)]
pub enum ElementKind {
    #[default]
    Generic,
    Input(HTMLInputElement),
    CustomRender(HTMLCustomRenderElement),
}

impl ElementKind {
    pub fn for_tag(tag_name: &str) -> Self {
        match tag_name {
            "INPUT" => ElementKind::Input(HTMLInputElement::default()),
            "CUSTOM-RENDER" => ElementKind::CustomRender(HTMLCustomRenderElement::default()),
            _ => ElementKind::Generic,
        }
    }

    pub fn on_click(&mut self, element: &mut DomElement, event: &mut DomEvent) -> bool {
        match self {
            ElementKind::Input(input) => {
                input.on_click(element, event);
                true
            }
            ElementKind::CustomRender(custom) => {
                custom.on_click(element, event);
                true
            }
            ElementKind::Generic => false,
        }
    }
}

pub struct DomElement {
    pub children: Vec<Rc<RefCell<DomElement>>>,
    pub attributes: HashMap<String, String>,
    pub parent_node: Option<Rc<RefCell<DomElement>>>,
    pub node_value: String,
    pub node_type: NodeType,
    pub inner_html: String,
    pub outer_html: String,
    pub tag_name: String,
    pub element_kind: ElementKind,
    pub style: Style,
    pub inherited_style: Option<Style>,
    pub pseudo_classes: PseudoClassState,
    pub computed_flow: Option<ComputedFlow>,
    pub computed_style: Option<ComputedStyle>,
    pub text_segments: Vec<TextSegment>,
    pub space_width: f32,
    pub cached_font_size: Option<f32>,
    pub cached_font_path: Option<String>,
    pub class_list: Vec<String>,
    pub matched_styles: Vec<StyleRule>,
    pub var_contexts: Vec<CssVariablesContext>,
    pub inline_declarations: Vec<Declaration>,
    /// Weak reference to owning Frame for notifications
    frame: Option<Weak<RefCell<Frame>>>,
}

// Manual Debug impl since Frame doesn't implement Debug
impl std::fmt::Debug for DomElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DomElement")
            .field("tag_name", &self.tag_name)
            .field("node_type", &self.node_type)
            .field("children", &self.children.len())
            .finish()
    }
}

// Manual Clone impl to ensure dirty_flag is shared
impl Clone for DomElement {
    fn clone(&self) -> Self {
        Self {
            children: self.children.clone(),
            attributes: self.attributes.clone(),
            parent_node: self.parent_node.clone(),
            node_value: self.node_value.clone(),
            node_type: self.node_type.clone(),
            inner_html: self.inner_html.clone(),
            outer_html: self.outer_html.clone(),
            tag_name: self.tag_name.clone(),
            element_kind: self.element_kind.clone(),
            style: self.style.clone(),
            inherited_style: self.inherited_style.clone(),
            pseudo_classes: self.pseudo_classes.clone(),
            computed_flow: self.computed_flow.clone(),
            computed_style: self.computed_style.clone(),
            text_segments: self.text_segments.clone(),
            space_width: self.space_width,
            cached_font_size: self.cached_font_size,
            cached_font_path: self.cached_font_path.clone(),
            class_list: self.class_list.clone(),
            matched_styles: self.matched_styles.clone(),
            var_contexts: self.var_contexts.clone(),
            inline_declarations: self.inline_declarations.clone(),
            frame: self.frame.clone(),
        }
    }
}

impl DomElement {
    pub fn new(node_type: NodeType, frame: Weak<RefCell<Frame>>) -> DomElement {
        DomElement {
            children: vec![],
            attributes: HashMap::new(),
            parent_node: None,
            node_type,
            inner_html: String::new(),
            outer_html: String::new(),
            node_value: String::new(),
            tag_name: String::new(),
            element_kind: ElementKind::Generic,
            style: Style::new(),
            inherited_style: None,
            computed_flow: None,
            computed_style: None,
            pseudo_classes: PseudoClassState::default(),
            text_segments: vec![],
            space_width: 0.0,
            cached_font_size: None,
            cached_font_path: None,
            class_list: vec![],
            matched_styles: vec![],
            var_contexts: vec![],
            inline_declarations: vec![],
            frame: Some(frame),
        }
    }

    /// Notify the Frame that styles need to be recalculated.
    /// Uses try_borrow() to avoid panicking if Frame is already borrowed
    /// during event dispatch. If we can't borrow, the frame is already
    /// being processed and will handle dirty state on completion.
    fn mark_dirty(&self) {
        if let Some(ref weak_frame) = self.frame {
            if let Some(frame) = weak_frame.upgrade() {
                if let Ok(borrowed) = frame.try_borrow() {
                    borrowed.mark_styles_dirty();
                }
            }
        }
    }

    pub fn set_tag_name(&mut self, tag_name: &str) {
        self.tag_name = tag_name.to_uppercase();
        self.element_kind = ElementKind::for_tag(&self.tag_name);
    }

    pub fn set_text_content(&mut self, text: &str) {
        self.children.clear();
        if let Some(ref frame_ref) = self.frame {
            let mut text_node = DomElement::new(NodeType::Text, frame_ref.clone());
            text_node.node_value = text.to_string();
            self.children.push(Rc::new(RefCell::new(text_node)));
        }
        self.mark_dirty();
    }

    pub fn set_attribute(&mut self, key: &str, value: &str) {
        if key == "style" {
            let val = format!("{{{}}}", value);
            let rules = parse_css(&val);
            self.inline_declarations.clear();
            for rule in rules {
                for mut decl in rule.declarations {
                    decl.important = true;
                    self.inline_declarations.push(decl);
                }
            }
        } else if key == "class" {
            self.class_list = value.split(' ').map(|x| x.to_string()).collect();
        }

        self.attributes.insert(key.to_string(), value.to_string());
        self.mark_dirty();
    }

    pub fn set_hover(&mut self, hover: bool) {
        if self.pseudo_classes.hover != hover {
            self.pseudo_classes.hover = hover;
            self.mark_dirty();
        }
    }

    pub fn set_focus(&mut self, focus: bool) {
        if self.pseudo_classes.focus != focus {
            self.pseudo_classes.focus = focus;
            self.mark_dirty();
        }
    }

    pub fn set_active(&mut self, active: bool) {
        if self.pseudo_classes.active != active {
            self.pseudo_classes.active = active;
            self.mark_dirty();
        }
    }

    pub fn append_child(&mut self, child: Rc<RefCell<DomElement>>) {
        self.children.push(child);
        self.mark_dirty();
    }

    pub fn remove_child(&mut self, child: &Rc<RefCell<DomElement>>) {
        self.children.retain(|existing| !Rc::ptr_eq(existing, child));
        self.mark_dirty();
    }
}
