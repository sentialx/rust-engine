use crate::colors::*;
use crate::html::*;
use crate::layout::*;
use crate::render_frame::GlyphsTextMeasurer;
use crate::render_frame::RenderFrame;
use crate::utils::Debouncer;
use std::collections::HashMap;
use std::rc::Rc;

extern crate find_folder;
extern crate piston_window;

use piston_window::*;
use piston_window::graphics::{clear, rectangle, Transformed};
use piston_window::graphics::text::Text;
use std::cell::RefCell;
use std::time::Duration;

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

    let glyphs_map: Rc<RefCell<HashMap<String, piston_window::Glyphs<'static>>>> =
        Rc::new(RefCell::new(HashMap::new()));

    let add_font = |name: &str| {
        let glyphs = window
            .load_font(
                assets.join(name),
                piston_window::wgpu_graphics::TextureSettings::new(),
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
    let mut resize_debouncer = Debouncer::new(Duration::from_millis(75));

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
            render_frame.scroll_y -= args[1] as f32 * 1.0;
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
        if event.resize_args().is_some() {
            let window_size = window.size();
            resize_debouncer.push((
                window_size.width as f32 - devtools_width,
                window_size.height as f32,
            ));
        }

        if let Some((width, height)) = resize_debouncer.poll() {
            render_frame.viewport.width = width;
            render_frame.viewport.height = height;
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

        let element = get_element_at(&render_frame.render_array, mouse_x / zoom as f32, (mouse_y + render_frame.scroll_y) / (zoom as f32));
        if element.is_some() {
            let el = element.unwrap().borrow();
            let mut rules = "".to_string();
            for rule in &el.matched_styles {
                rules += &(format!("{}", rule.to_string()) + "\n");
            }
            el_txt = rules;
            el_txt += &format!(
                "{:?}\n{:#?}\n{:#?}",
                el.tag_name, el.attributes, el.computed_style
            );
        }

        let window_size = &window.size();

        if resize_debouncer.is_pending() {
            continue;
        }
        window.draw_2d(&event, |c, g, _device| {
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
                            Text::new_color(color, 2 * (item.font_size) as u32)
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
                    let el = element.unwrap().borrow();
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

                        Text::new_color([1.0, 1.0, 1.0, 1.0], 2 * 12)
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
                                .trans(computed_flow.x as f64 * zoom, (el_y - 2.0) as f64 * zoom)
                                .zoom(0.5)
                                .zoom(zoom),
                            g,
                        )
                        .unwrap();

                        let mut lines = el_txt.split("\n");
                        for (i, line) in lines.enumerate() {
                            let font_size = 16.0;
                            Text::new_color([0.0, 0.0, 0.0, 1.0], 2 * font_size as u32)
                                .draw(
                                    &line,
                                    glyphs,
                                    &c.draw_state,
                                    c.transform
                                        .trans(dev_tools_x as f64 + 8.0, (font_size + 5.0) * i as f64 * devtools_zoom + 16.0)
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
