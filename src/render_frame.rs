use std::{cell::RefCell, collections::HashMap, fs, rc::Rc, time::Instant};

use piston_window::graphics::character::CharacterCache;
use rusttype::Scale;

use crate::{
    css::parse_css,
    html::{parse_html, DomElement, NodeType},
    layout::{compute_styles, get_render_array, propagate_styles, reflow, Rect, RenderItem},
    styles::StyleRule,
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

pub struct RenderFrame<'a> {
    pub viewport: Rect,
    pub scroll_y: f32,
    pub render_array: Vec<RenderItem>,
    pub dom_tree: Vec<Rc<RefCell<DomElement>>>,
    pub parsed_css: Vec<StyleRule>,
    pub styles: Vec<StyleRule>,
    pub url: String,
    pub text_measurer: &'a mut dyn TextMeasurer,
    pub default_styles: Vec<StyleRule>,
}

pub trait TextMeasurer {
    /// Measure text, returning (width, height)
    fn measure(&mut self, text: &str, font_size: f32, font_family: &str) -> (f32, f32);

    /// Get the ascent (distance from baseline to top) for a font at given size
    /// Returns the ascent value that should be added to the top y to get baseline y
    fn ascent(&mut self, font_size: f32, font_family: &str) -> f32;
}

pub struct GlyphsTextMeasurer<'a> {
    pub glyphs_map: Rc<RefCell<HashMap<String, piston_window::Glyphs<'a>>>>,
}

impl TextMeasurer for GlyphsTextMeasurer<'_> {
    fn measure(&mut self, text: &str, font_size: f32, font_family: &str) -> (f32, f32) {
        let mut glyphs_map = self.glyphs_map.borrow_mut();
        let glyphs = glyphs_map.get_mut(font_family).unwrap();

        // Get width from glyph cache
        let width = 0.5 * glyphs.width(2 * (font_size) as u32, text).unwrap() as f32;

        // Get height from actual font metrics
        // We use 2x font size in rendering, then scale down by 0.5
        let scale = Scale::uniform(2.0 * font_size);
        let v_metrics = glyphs.font.v_metrics(scale);
        // height = ascent - descent (descent is negative, so this adds them)
        let height = (v_metrics.ascent - v_metrics.descent) * 0.5;

        (width, height)
    }

    fn ascent(&mut self, font_size: f32, font_family: &str) -> f32 {
        let mut glyphs_map = self.glyphs_map.borrow_mut();
        if let Some(glyphs) = glyphs_map.get_mut(font_family) {
            // Get actual font metrics from rusttype
            // We use 2x font size in rendering, then scale down by 0.5
            let scale = Scale::uniform(2.0 * font_size);
            let v_metrics = glyphs.font.v_metrics(scale);
            // Small adjustment: font ascent includes space for tall glyphs (Á),
            // but typical text sits slightly lower. Add 2px to baseline.
            v_metrics.ascent * 0.5 + 2.0
        } else {
            // Fallback
            font_size * 0.8
        }
    }
}

impl<'a> RenderFrame<'a> {
    pub fn new(viewport: Rect, text_measurer: &'a mut dyn TextMeasurer) -> RenderFrame<'a> {
        let default_css =
            fs::read_to_string("default_styles.css").expect("error while reading the file");
        let default_styles = parse_css(&default_css);

        RenderFrame {
            viewport,
            scroll_y: 0.0,
            render_array: vec![],
            dom_tree: vec![],
            parsed_css: vec![],
            styles: vec![],
            url: "".to_string(),
            text_measurer,
            default_styles,
        }
    }

    pub fn load_url(&mut self, url: &str) {
        self.url = url.to_string();
        let contents = fs::read_to_string(self.url.clone()).expect("error while reading the file");
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

        self.render();
    }

    pub fn refresh(&mut self) {
        let str = self.url.clone();
        self.load_url(&str);
    }

    pub fn fast_render(&mut self) {
        let s = Instant::now();
        let mut viewport = self.viewport.clone();
        viewport.y = self.scroll_y;
        let render_array: Vec<RenderItem> = get_render_array(&mut self.dom_tree, &viewport)
            .into_iter()
            .collect();
        println!(
            "Rerendering took: {:?}, items: {:?}",
            s.elapsed(),
            render_array.len()
        );

        self.render_array = render_array;
    }

    pub fn reflow(&mut self) {
        let s = Instant::now();

        reflow(&mut self.dom_tree, self.text_measurer, None, &self.viewport);
        println!("Reflow took: {:?}", s.elapsed());
    }

    pub fn compute_styles(&mut self) {
        let s = Instant::now();
        compute_styles(&mut self.dom_tree, &self.styles, &mut vec![], None);
        println!("Computing styles took: {:?}", s.elapsed());
        let s = Instant::now();
        propagate_styles(&mut self.dom_tree, None);
        println!("Propagating styles took: {:?}", s.elapsed());
    }

    pub fn render(&mut self) {
        let s = Instant::now();

        self.compute_styles();
        self.reflow();
        self.fast_render();

        println!("Refreshing took: {:?}", s.elapsed());
    }
}
