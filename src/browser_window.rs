use crate::colors::*;
use crate::css::*;
use crate::debug::*;
use crate::html::*;
use crate::layout::*;
use crate::styles::*;
use std::cell::RefMut;
use std::collections::HashMap;
use std::fmt::format;
use std::rc::Rc;
use std::time::Instant;

extern crate find_folder;
extern crate piston_window;

use opengl_graphics::GlGraphics;
use piston_window::character::CharacterCache;
use piston_window::*;
use std::cell::RefCell;
use std::fs;

pub struct RenderFrame<'a> {
    pub viewport: Rect,
    pub scroll_y: f32,
    pub render_array: Vec<RenderItem>,
    pub dom_tree: Vec<DomElement>,
    pub parsed_css: Vec<StyleRule>,
    pub styles: Vec<StyleRule>,
    pub url: String,
    pub text_measurer: &'a mut dyn TextMeasurer,
    pub default_styles: Vec<StyleRule>,
}

pub trait TextMeasurer {
    fn measure(&mut self, text: &str, font_size: f32, font_family: &str) -> (f32, f32);
}

struct GlyphsTextMeasurer<'a> {
    glyphs_map: Rc<RefCell<HashMap<String, opengl_graphics::GlyphCache<'a>>>>,
}

impl TextMeasurer for GlyphsTextMeasurer<'_> {
    fn measure(&mut self, text: &str, font_size: f32, font_family: &str) -> (f32, f32) {
        let mut glyphs_map = self.glyphs_map.borrow_mut();
        let glyphs = glyphs_map.get_mut(font_family).unwrap();
        return (0.5 * glyphs.width(2 * (font_size) as u32, text).unwrap() as f32, font_size - 2.0 + 8.0);
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

        let style = get_styles(self.dom_tree.clone(), None);
        self.parsed_css = parse_css(&style);

        self.styles = [
            self.default_styles.clone(),
            self.parsed_css.clone(),
        ].concat();

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
        let render_array: Vec<RenderItem> =
            get_render_array(&mut self.dom_tree, &viewport)
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

        reflow(
            &mut self.dom_tree,
            self.text_measurer,
            None,
            &self.viewport,
        );
        println!("Reflow took: {:?}", s.elapsed());
    }

    pub fn compute_styles(&mut self) {
        let s = Instant::now();
        compute_styles(&mut self.dom_tree, &self.styles, None);
        println!("Computing styles took: {:?}", s.elapsed());
    }

    pub fn render(&mut self) {
        let s = Instant::now();
        
        self.compute_styles();
        self.reflow();
        self.fast_render();

        println!("Refreshing took: {:?}", s.elapsed());
    }
}

pub fn create_browser_window(url: String) {
    let mut window: PistonWindow = WindowSettings::new("Graviton", [1366, 768])
        .exit_on_esc(true)
        .build()
        .unwrap();

    let assets = find_folder::Search::ParentsThenKids(3, 3)
        .for_folder("assets")
        .unwrap();

    let mut glyphs_map: Rc<RefCell<HashMap<String, opengl_graphics::GlyphCache>>> = Rc::new(RefCell::new(HashMap::new()));

    let mut add_font = |name: &str| {
        let mut glyphs = opengl_graphics::GlyphCache::new(
            assets.join(name),
            (),
            opengl_graphics::TextureSettings::new(),
        )
        .unwrap();
        glyphs_map.borrow_mut().insert(
            name.to_string(),
            // window.load_font(assets.join(name)).unwrap(),
            glyphs,
        )
    };


    let get_window_rect = |window: &PistonWindow, scroll_y: f32| -> Rect {
        let window_size = window.size();
        return Rect {
            x: 0.0,
            y: scroll_y.clone(),
            width: window_size.width as f32,
            height: window_size.height as f32,
        };
    };

    add_font("Times New Roman 400.ttf");
    add_font("Times New Roman 700.ttf");
    add_font("Times New Roman Italique 400.ttf");
    add_font("Times New Roman Italique 700.ttf");


    let color_conv = |c: ColorTupleA| {
        // [
        //     (c.0 / 255.0) as f32,
        //     (c.1 / 255.0) as f32,
        //     (c.2 / 255.0) as f32,
        //     (c.3 / 255.0) as f32,
        // ]
        [c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0, c.3 as f32]
    };

    let mut pressed_up = false;
    let mut pressed_down = false;

    let mut mouse_x = 0.0;
    let mut mouse_y = 0.0;

    let mut el_txt = "".to_string();
    let mut element: Option<&DomElement> = None;

    let opengl = OpenGL::V3_2;
    let mut gl = GlGraphics::new(opengl);

    let devtools_width = 300.0;

    let window_rect = get_window_rect(&window, 0.0);
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: window_rect.width - devtools_width,
        height: window_rect.height,
    };

    let mut text_measurer = GlyphsTextMeasurer { glyphs_map: glyphs_map.clone() };
    let mut render_frame = RenderFrame::new(viewport, &mut text_measurer);

    render_frame.load_url(&url);

    while let Some(event) = window.next() {
        let mouse = event.mouse_cursor_args();

        // key down

        if let Some(Button::Keyboard(key)) = event.press_args() {
            if key == Key::F5 {
                render_frame.refresh();
            }

            if key == Key::Up {
                pressed_up = true;
            } else if key == Key::Down {
                pressed_down = true;
            }
        };

        if let Some(Button::Keyboard(key)) = event.release_args() {
            if key == Key::Up {
                pressed_up = false;
            } else if key == Key::Down {
                pressed_down = false;
            }
        };

        // scroll event
        if let Some(args) = event.mouse_scroll_args() {
            render_frame.scroll_y -= args[1] as f32 * 24.0;
            render_frame.fast_render();
        }

        if pressed_up {
            render_frame.scroll_y -= 4.0;
        }

        if pressed_down {
            render_frame.scroll_y += 4.0;
        }

        render_frame.scroll_y = f32::max(0.0, render_frame.scroll_y);

        if pressed_down || pressed_up {
            render_frame.fast_render();
            // println!("items: {:?}", render_array);
        }

        // on resize
        if let Some(size) = event.resize_args() {
            let window_size = window.size();
            render_frame.viewport.width = window_size.width as f32 - devtools_width;
            render_frame.viewport.height = window_size.height as f32;
            render_frame.reflow();
            render_frame.fast_render();
        }

        if mouse.is_some() {
            mouse_x = mouse.unwrap()[0] as f32;
            mouse_y = mouse.unwrap()[1] as f32;

            // get dom element at mouse position

            // if should_rerender(
            //     mouse_x,
            //     mouse_y,
            //     &mut dom_tree.borrow_mut(),
            //     &*parsed_css.borrow_mut(),
            // ) {
                
            //     recompute_styles(&window, &styles);
            //     reflow();
            //     render_array = rerender(&window, scroll_y);
            // }
        }

        let dom_tree = &render_frame.dom_tree;
        let element = get_element_at(&dom_tree, mouse_x, mouse_y + render_frame.scroll_y);
        if element.is_some() {
            let el = element.unwrap();
            el_txt = format!(
                "{:?} {:#?} {:#?}",
                el.tag_name, el.attributes, el.computed_style
            );
        }

        let window_size = &window.size();

        if let Some(args) = event.render_args() {
            gl.draw(args.viewport(), |c, g| {
                clear([1.0, 1.0, 1.0, 1.0], g);
                // let device = &mut c.de

                // window.draw_2d(&event, |context, graphics, device| {

                let mut font_path = "".to_string();
                let mut glyphs_map = glyphs_map.borrow_mut();
                

                for item in &render_frame.render_array {
                    let item_y = item.y as f64 - render_frame.scroll_y as f64;
                    let glyphs = glyphs_map.get_mut(&item.font_path).unwrap();

                    if item.background_color != (0.0, 0.0, 0.0, 0.0) {
                        rectangle(
                            color_conv(item.background_color),
                            [0.0, 0.0, item.width as f64, item.height as f64],
                            c.transform.trans(item.x as f64, item_y),
                            g,
                        );
                    }

                    if item.text_lines.len() > 0 {
                        font_path = item.font_path.clone();

                        let color = color_conv(item.color);

                        for line in &item.text_lines {
                            let ly = line.y as f64 - render_frame.scroll_y as f64;
                            text::Text::new_color(color, 2 * ((item.font_size) as u32))
                            .draw(
                                &line.text,
                                glyphs,
                                &c.draw_state,
                                c.transform
                                    .trans(line.x as f64, ly + (line.height as f64) - 4.0 )
                                    .zoom(0.5),
                                g,
                            )
                            .unwrap();

                            if item.underline {
                                rectangle(
                                    color,
                                    [0.0, 0.0, item.width as f64, 1.0],
                                    c.transform.trans(line.x as f64, ly + line.height as f64 - 3.0),
                                    g,
                                );
                            }
                        }
                       

                       
                    }
                }

                let dev_tools_x = window_size.width as f32 - devtools_width;

                // separator
                rectangle(
                    [0.0, 0.0, 0.0, 0.12],
                    [0.0, 0.0, 1 as f64, window_size.height as f64],
                    c.transform.trans(dev_tools_x as f64, 0.0),
                    g,
                );

                rectangle(
                    [1.0, 1.0, 1.0, 1.0],
                    [0.0, 0.0, devtools_width as f64, window_size.height as f64],
                    c.transform.trans(dev_tools_x as f64, 0.0),
                    g,
                );

                if element.is_some() {
                    let el = element.unwrap();
                    let computed_flow = el.computed_flow.as_ref().unwrap();
                    let el_y = computed_flow.y as f64 - render_frame.scroll_y as f64;

                    rectangle(
                        [1.0, 0.0, 0.5, 0.1],
                        [0.0, 0.0, computed_flow.width as f64, computed_flow.height as f64],
                        c.transform.trans(computed_flow.x as f64, el_y),
                        g,
                    );

                    rectangle(
                        [1.0, 0.0, 0.5, 1.0],
                        [0.0, 0.0, 128.0, 18.0],
                        c.transform.trans(computed_flow.x as f64, el_y - 18.0),
                        g,
                    );

                    // // split newlines
                    if glyphs_map.get_mut(&font_path).is_some() {
                        
                
                    let glyphs = glyphs_map.get_mut(&font_path).unwrap();

                    text::Text::new_color([255.0, 255.0, 255.0, 255.0], 2 * ((14.0 - 2.0) as u32))
                        .draw(
                            format!(
                                "{:?} {:?}x{:?}",
                                el.tag_name,
                                f64::trunc(computed_flow.width as f64 * 10.0) / 10.0,
                                f64::trunc(computed_flow.height as f64 * 10.0) / 10.0
                            )
                            .as_str(),
                            glyphs,
                            &c.draw_state,
                            c.transform
                                .trans(computed_flow.x as f64, el_y - 4.0 as f64)
                                .zoom(0.5),
                            g,
                        )
                        .unwrap();

                    let mut lines = el_txt.split("\n");
                    for (i, line) in lines.enumerate() {
                        text::Text::new_color([0.0, 0.0, 0.0, 255.0], 2 * (12.0 as u32))
                            .draw(
                                &line,
                                glyphs,
                                &c.draw_state,
                                c.transform
                                    .trans(dev_tools_x as f64, 8.0 + 12.0 * i as f64)
                                    .zoom(0.5),
                                g,
                            )
                            .unwrap();
                    }
                }
                }

                // glyphs_map.iter_mut().for_each(|(k, mut v)| {
                //     v.factory.encoder.flush(device);
                // });
            });
        }
    }
}
