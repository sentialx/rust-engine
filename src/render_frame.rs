use std::{cell::RefCell, collections::HashMap, fs, rc::Rc, time::Instant};

use piston_window::CharacterCache;

use crate::{
    css::parse_css,
    html::{parse_html, DomElement},
    layout::{compute_styles, get_render_array, propagate_styles, reflow, Rect, RenderItem},
    styles::StyleRule,
};

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
    fn measure(&mut self, text: &str, font_size: f32, font_family: &str) -> (f32, f32);
}

pub struct GlyphsTextMeasurer<'a> {
    pub glyphs_map: Rc<RefCell<HashMap<String, opengl_graphics::GlyphCache<'a>>>>,
}

impl TextMeasurer for GlyphsTextMeasurer<'_> {
    fn measure(&mut self, text: &str, font_size: f32, font_family: &str) -> (f32, f32) {
        let mut glyphs_map = self.glyphs_map.borrow_mut();
        let glyphs = glyphs_map.get_mut(font_family).unwrap();
        return (
            0.5 * glyphs.width(2 * (font_size) as u32, text).unwrap() as f32,
            font_size + 8.0,
        );
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

        let style = fs::read_to_string("style.css").expect("error while reading the file");
        self.parsed_css = parse_css(&style);

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
