use crate::colors::*;
use crate::html::*;
use crate::layout::*;
use crate::render_frame::GlyphsTextMeasurer;
use crate::render_frame::RenderFrame;
use std::collections::HashMap;
use std::rc::Rc;

extern crate find_folder;
extern crate piston_window;

use opengl_graphics::GlGraphics;
use piston_window::*;
use std::cell::RefCell;

fn css_color_to_piston(c: ColorTupleA) -> [f32; 4] {
    [
        c.0 as f32 / 255.0,
        c.1 as f32 / 255.0,
        c.2 as f32 / 255.0,
        c.3 as f32,
    ]
}

pub fn create_browser_window(url: String) {
    let mut window: PistonWindow = WindowSettings::new("Graviton", [1366, 768])
        .exit_on_esc(true)
        .build()
        .unwrap();

    let assets = find_folder::Search::ParentsThenKids(3, 3)
        .for_folder("assets")
        .unwrap();

    let glyphs_map: Rc<RefCell<HashMap<String, opengl_graphics::GlyphCache>>> =
        Rc::new(RefCell::new(HashMap::new()));

    let add_font = |name: &str| {
        let glyphs = opengl_graphics::GlyphCache::new(
            assets.join(name),
            (),
            opengl_graphics::TextureSettings::new(),
        )
        .unwrap();
        glyphs_map.borrow_mut().insert(name.to_string(), glyphs)
    };

    add_font("Times New Roman 400.ttf");
    add_font("Times New Roman 700.ttf");
    add_font("Times New Roman Italique 400.ttf");
    add_font("Times New Roman Italique 700.ttf");

    let mut pressed_up = false;
    let mut pressed_down = false;

    let mut mouse_x = 0.0;
    let mut mouse_y = 0.0;

    let mut el_txt = "".to_string();
    let mut element: Option<&DomElement> = None;

    let opengl = OpenGL::V3_2;
    let mut gl = GlGraphics::new(opengl);

    let devtools_width = 300.0;

    let window_size = window.size();
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: window_size.width as f32 - devtools_width,
        height: window_size.height as f32,
    };

    let mut text_measurer = GlyphsTextMeasurer {
        glyphs_map: glyphs_map.clone(),
    };
    let mut render_frame = RenderFrame::new(viewport, &mut text_measurer);

    render_frame.load_url(&url);

    let zoom = 1.0;
    let devtools_zoom = 0.65;

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
        let element = get_element_at(&dom_tree, mouse_x / zoom as f32, (mouse_y + render_frame.scroll_y) / (zoom as f32));
        if element.is_some() {
            let el = element.unwrap();
            el_txt = format!(
                "{:?}\n{:#?}\n{:#?}\n{:#?}",
                el.tag_name, el.attributes, el.matched_selectors, el.computed_style
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
                            css_color_to_piston(item.background_color),
                            [0.0, 0.0, item.width as f64, item.height as f64],
                            c.transform.trans(item.x as f64 * zoom, item_y * zoom).zoom(zoom),
                            g,
                        );
                    }

                    if item.text_lines.len() > 0 {
                        font_path = item.font_path.clone();

                        let color = css_color_to_piston(item.color);

                        for line in &item.text_lines {
                            let ly = line.y as f64 - render_frame.scroll_y as f64;
                            text::Text::new_color(color, (2 * (item.font_size) as u32))
                                .draw(
                                    &line.text,
                                    glyphs,
                                    &c.draw_state,
                                    c.transform
                                        .trans(line.x as f64 * zoom, (ly + (line.height as f64)) * zoom)
                                        .zoom(0.5)
                                        .zoom(zoom),
                                    g,
                                )
                                .unwrap();

                            if item.underline {
                                rectangle(
                                    color,
                                    [0.0, 0.0, item.width as f64, 1.0],
                                    c.transform
                                        .trans(line.x as f64 * zoom, (ly + line.height as f64 + 1.0) * zoom)
                                        .zoom(zoom),
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
                        [
                            0.0,
                            0.0,
                            computed_flow.width as f64,
                            computed_flow.height as f64,
                        ],
                        c.transform.trans(computed_flow.x as f64 * zoom, el_y * zoom).zoom(zoom),
                        g,
                    );

                    rectangle(
                        [1.0, 0.0, 0.5, 1.0],
                        [0.0, 0.0, 128.0, 18.0],
                        c.transform.trans(computed_flow.x as f64 * zoom, (el_y - 18.0) * zoom).zoom(zoom),
                        g,
                    );

                    // // split newlines
                    if glyphs_map.get_mut(&font_path).is_some() {
                        let glyphs = glyphs_map.get_mut(&font_path).unwrap();

                        text::Text::new_color(
                            [1.0, 1.0, 1.0, 1.0],
                            2 * 14,
                        )
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
                                .trans(computed_flow.x as f64 * zoom, el_y as f64 * zoom)
                                .zoom(0.5)
                                .zoom(zoom),
                            g,
                        )
                        .unwrap();

                        let mut lines = el_txt.split("\n");
                        for (i, line) in lines.enumerate() {
                            let font_size = 16.0;
                            text::Text::new_color([0.0, 0.0, 0.0, 1.0], 2 * font_size as u32)
                                .draw(
                                    &line,
                                    glyphs,
                                    &c.draw_state,
                                    c.transform
                                        .trans(dev_tools_x as f64, (font_size + 5.0) * i as f64 * devtools_zoom)
                                        .zoom(0.5)
                                        .zoom(devtools_zoom),
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
