use crate::colors::*;
use crate::css::*;
use crate::debug::*;
use crate::html::*;
use crate::layout::*;
use crate::styles::*;
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::fmt::format;
use std::time::Instant;

extern crate find_folder;
extern crate piston_window;

use opengl_graphics::GlGraphics;
use piston_window::character::CharacterCache;
use piston_window::*;
use std::cell::RefCell;
use std::fs;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

pub fn create_browser_window(url: String) {
    let mut window: PistonWindow = WindowSettings::new("Graviton", [1366, 768])
        .exit_on_esc(true)
        .build()
        .unwrap();

    let assets = find_folder::Search::ParentsThenKids(3, 3)
        .for_folder("assets")
        .unwrap();

    let mut glyphs_map: RefCell<HashMap<String, opengl_graphics::GlyphCache>> =
        RefCell::new(HashMap::new());

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

    let mut scroll_y: f32 = 0.0;



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

    let mut render_array: Vec<RenderItem> = vec![];
    let mut dom_tree: RefCell<Vec<DomElement>> = RefCell::new(vec![]);
    let mut parsed_css: RefCell<Vec<StyleRule>> = RefCell::new(vec![]);
    let mut styles: Vec<StyleRule> = vec![];

    let default_css =
        fs::read_to_string("default_styles.css").expect("error while reading the file");
    let default_styles = parse_css(&default_css);

    let color_conv = |c: ColorTupleA| {
        // [
        //     (c.0 / 255.0) as f32,
        //     (c.1 / 255.0) as f32,
        //     (c.2 / 255.0) as f32,
        //     (c.3 / 255.0) as f32,
        // ]
        [c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0, c.3 as f32]
    };

    let rerender = |window: &PistonWindow, scroll_y: f32| {
        let s = Instant::now();
        let window_rect = get_window_rect(&window, scroll_y);
        let render_array: Vec<RenderItem> =
            get_render_array(&mut dom_tree.borrow_mut(), &window_rect)
                .into_iter()
                .collect();
        println!(
            "Rerendering took: {:?}, items: {:?}",
            s.elapsed(),
            render_array.len()
        );

        return render_array;
    };

    let reflow = |window: &PistonWindow, scroll_y: f32| {
        let s = Instant::now();
        let closure_ref = RefCell::new(|text: String, font_size: f32, font_family: String| {
            let mut glyphs_map = glyphs_map.borrow_mut();
            let glyphs = glyphs_map.get_mut(font_family.as_str()).unwrap();
            return 0.5 * glyphs.width(2 * (font_size) as u32, &text).unwrap();
        });

        reflow(
            &mut dom_tree.borrow_mut(),
            &move |text, font_size, font_family| {
                return (
                    (closure_ref.borrow_mut())(text, font_size, font_family) as f32,
                    font_size - 2.0 + 8.0,
                );
            },
            None,
            &get_window_rect(&window, scroll_y),
        );
        println!("Reflow took: {:?}", s.elapsed());
    };

    let recompute_styles = |window: &PistonWindow, styles: &Vec<StyleRule>| {
        let s = Instant::now();
        compute_styles(&mut dom_tree.borrow_mut(), &styles, None);
        println!("Computing styles took: {:?}", s.elapsed());
    };

    let recalc_all = |window: &PistonWindow, styles: &Vec<StyleRule>, scroll_y: f32| {
        recompute_styles(&window, &styles);
        reflow(&window, scroll_y);
        return rerender(&window, scroll_y);
    };

    let refresh = |window: &PistonWindow, u: String| {
        let contents = fs::read_to_string(u.clone()).expect("error while reading the file");
        *dom_tree.borrow_mut() = parse_html(&contents);

        let style = get_styles(dom_tree.borrow_mut().clone(), None);
        *parsed_css.borrow_mut() = parse_css(&style);

        // println!("Styles: {:#?}", parsed_css.borrow_mut());

        return [default_styles.clone(), parsed_css.borrow_mut().clone()].concat();
    };

    styles = refresh(&window, url.clone());
    render_array = recalc_all(&window, &styles, scroll_y);

    let mut pressed_up = false;
    let mut pressed_down = false;

    let mut mouse_x = 0.0;
    let mut mouse_y = 0.0;

    let mut el_txt = "".to_string();
    let mut element: Option<&DomElement> = None;

    let opengl = OpenGL::V3_2;
    let mut gl = GlGraphics::new(opengl);

    while let Some(event) = window.next() {
        let mouse = event.mouse_cursor_args();

        // key down

        if let Some(Button::Keyboard(key)) = event.press_args() {
            if key == Key::F5 {
                styles = refresh(&window, url.clone());
                render_array = recalc_all(&window, &styles, scroll_y);
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
            scroll_y -= args[1] as f32 * 1.0;
            render_array = rerender(&window, scroll_y);
        }

        if pressed_up {
            scroll_y -= 4.0;
        }

        if pressed_down {
            scroll_y += 4.0;
        }

        scroll_y = f32::max(0.0, scroll_y);

        if pressed_down || pressed_up {
            render_array = rerender(&window, scroll_y);
            // println!("items: {:?}", render_array);
        }

        // on resize
        if let Some(size) = event.resize_args() {
            reflow(&window, scroll_y);
            render_array = rerender(&window, scroll_y);
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

        let mut dom_tree = dom_tree.borrow_mut();
        let element = get_element_at(&dom_tree, mouse_x, mouse_y + scroll_y);
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

                for item in &render_array {
                    let item_y = item.y as f64 - scroll_y as f64;
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
                            let ly = line.y as f64 - scroll_y as f64;
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

                if element.is_some() {
                    let el = element.unwrap();
                    let computed_flow = el.computed_flow.as_ref().unwrap();
                    let el_y = computed_flow.y as f64 - scroll_y as f64;

                    rectangle(
                        [1.0, 0.0, 0.5, 0.1],
                        [0.0, 0.0, computed_flow.width as f64, computed_flow.height as f64],
                        c.transform.trans(computed_flow.x as f64, el_y),
                        g,
                    );

                    let dev_tools_width = 256.0;
                    let dev_tools_x = window_size.width - dev_tools_width;

                    // rectangle(
                    //     [255.0, 255.0, 255.0, 255.0],
                    //     [0.0, 0.0, dev_tools_width, window_size.height as f64],
                    //     c.transform.trans(dev_tools_x, 0.0),
                    //     g,
                    // );
                    
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

                    // let mut lines = el_txt.split("\n");
                    // for (i, line) in lines.enumerate() {
                    //     text::Text::new_color([0.0, 0.0, 0.0, 255.0], 2 * (12.0 as u32))
                    //         .draw(
                    //             &line,
                    //             glyphs,
                    //             &c.draw_state,
                    //             c.transform
                    //                 .trans(dev_tools_x, 8.0 + 12.0 * i as f64)
                    //                 .zoom(0.5),
                    //             g,
                    //         )
                    //         .unwrap();
                    // }
                }
                }

                // glyphs_map.iter_mut().for_each(|(k, mut v)| {
                //     v.factory.encoder.flush(device);
                // });
            });
        }
    }
}
