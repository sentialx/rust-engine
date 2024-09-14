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

use piston_window::character::CharacterCache;
use piston_window::*;
use std::cell::RefCell;
use std::fs;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

pub fn create_browser_window(url: String) {
    let mut window: PistonWindow = WindowSettings::new("Graviton", [640, 480])
        .exit_on_esc(true)
        .build()
        .unwrap();

    let assets = find_folder::Search::ParentsThenKids(3, 3)
        .for_folder("assets")
        .unwrap();

    let mut glyphs_map: RefCell<HashMap<String, Glyphs>> = RefCell::new(HashMap::new());

    let mut add_font = |name: &str| {
        glyphs_map.borrow_mut().insert(
            name.to_string(),
            window.load_font(assets.join(name)).unwrap(),
        )
    };

    let mut scroll_y: f64 = 0.0;

    let get_window_rect = |window: &PistonWindow, scroll_y: f64| -> Rect {
        let window_size = window.size();
        return Rect {
            x: 0.0,
            y: scroll_y.clone(),
            width: window_size.width as f64,
            height: window_size.height as f64,
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
        [
            (c.0 / 255.0) as f32,
            (c.1 / 255.0) as f32,
            (c.2 / 255.0) as f32,
            (c.3 / 255.0) as f32,
        ]
    };

    let rerender = |window: &PistonWindow, scroll_y: f64| {
        let s = Instant::now();
        let window_rect = get_window_rect(&window, scroll_y);
        let render_array: Vec<RenderItem> = get_render_array(
            &mut dom_tree.borrow_mut(),
            &window_rect,
        )
        .into_iter()
        .collect();
        println!("Rerendering took: {:?}, items: {:?}", s.elapsed(), render_array.len());

        return render_array;
    };

    let reflow = || {
        let s = Instant::now();
        let closure_ref = RefCell::new(|text: String, font_size: f64, font_family: String| {
            let mut glyphs_map = glyphs_map.borrow_mut();
            let glyphs = glyphs_map.get_mut(font_family.as_str()).unwrap();
            return 0.5 * glyphs.width(2 * (font_size - 2.0) as u32, &text).unwrap();
        });
        
        reflow(
            &mut dom_tree.borrow_mut(),
            &move |text, font_size, font_family| {
                return (
                    (closure_ref.borrow_mut())(text, font_size, font_family),
                    font_size - 2.0 + 8.0,
                );
            },
            None,
        );
        println!("Reflow took: {:?}", s.elapsed());
    };

    let recompute_styles = |window: &PistonWindow, styles: &Vec<StyleRule>| {
        let s = Instant::now();
        compute_styles(&mut dom_tree.borrow_mut(), &styles, None);
        println!("Computing styles took: {:?}", s.elapsed());
    };

    let recalc_all = |window: &PistonWindow, styles: &Vec<StyleRule>, scroll_y: f64| {
        recompute_styles(&window, &styles);
        reflow();
        return rerender(&window, scroll_y);
    };

    let refresh = |window: &PistonWindow, u: String| {
        let contents = fs::read_to_string(u.clone()).expect("error while reading the file");
        *dom_tree.borrow_mut() = parse_html(&contents);

        let style = get_styles(dom_tree.borrow_mut().clone(), None);
        *parsed_css.borrow_mut() = parse_css(&style);

        println!("Styles: {:?}", parsed_css.borrow_mut());

        return [default_styles.clone(), parsed_css.borrow_mut().clone()].concat()
    };

    styles = refresh(&window, url.clone());
    render_array = recalc_all(&window, &styles, scroll_y);

    let mut pressed_up = false;
    let mut pressed_down = false;

    let mut mouse_x = 0.0;
    let mut mouse_y = 0.0;

    let mut el_txt = "".to_string();
    let mut element: Option<&DomElement> = None;
    
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

        if pressed_up {
            scroll_y -= 10.0;
        }

        if pressed_down {
            scroll_y += 10.0;
        }

        if pressed_down || pressed_up {
            render_array = rerender(&window, scroll_y);
            println!("items: {:?}", render_array);
        }

        // on resize
        if let Some(size) = event.resize_args() {
            reflow();
            render_array = rerender(&window, scroll_y);
        }



        if mouse.is_some() {
            mouse_x = mouse.unwrap()[0];
            mouse_y = mouse.unwrap()[1];

            // get dom element at mouse position
            

            // if should_rerender(
            //     mouse_x,
            //     mouse_y,
            //     &mut dom_tree.borrow_mut(),
            //     &*parsed_css.borrow_mut(),
            // ) {
            //     // render_array = rerender(&window);
            // }
        }

        let mut dom_tree = dom_tree.borrow_mut();
        let element = get_element_at(&dom_tree, mouse_x, mouse_y + scroll_y);
        if element.is_some() {
            let el = element.unwrap();
            el_txt = format!("{:?} {:#?} {:#?}", el.tag_name, el.attributes, el.computed_style);
        }

        let window_size = &window.size();

        window.draw_2d(&event, |context, graphics, device| {
            clear([1.0, 1.0, 1.0, 1.0], graphics);

            let mut font_path = "".to_string();

            for item in &render_array {
                let item_y = item.y - scroll_y;

                if item.background_color != (0.0, 0.0, 0.0, 0.0) {
                    rectangle(
                        color_conv(item.background_color),
                        [0.0, 0.0, item.width, item.height],
                        context.transform.trans(item.x, item_y),
                        graphics,
                    );
                }

                if item.text != "" {
                    let mut glyphs_map = glyphs_map.borrow_mut();
                    let glyphs = glyphs_map.get_mut(&item.font_path).unwrap();
                    font_path = item.font_path.clone();

                    let color = color_conv(item.color);

                    text::Text::new_color(color, 2 * ((item.font_size - 2.0) as u32))
                        .draw(
                            &item.text,
                            glyphs,
                            &context.draw_state,
                            context
                                .transform
                                .trans(item.x, item_y + item.height - 4.0)
                                .zoom(0.5),
                            graphics,
                        )
                        .unwrap();

                    if item.underline {
                        rectangle(
                            color,
                            [0.0, 0.0, item.width, 1.0],
                            context.transform.trans(item.x, item_y + item.height - 3.0),
                            graphics,
                        );
                    }
                    glyphs.factory.encoder.flush(device);
                }

                
            }

            if element.is_some() {
                let mut glyphs_map = glyphs_map.borrow_mut();
                let glyphs = glyphs_map.get_mut(&font_path).unwrap();

                let el = element.unwrap();
                let computed_flow = el.computed_flow.as_ref().unwrap();
                let el_y = computed_flow.y - scroll_y;

                rectangle(
                    [1.0, 0.0, 0.5, 0.1],
                    [0.0, 0.0, computed_flow.width, computed_flow.height],
                    context.transform.trans(computed_flow.x, el_y),
                    graphics,
                );

                let dev_tools_width = 256.0;    
                let dev_tools_x = window_size.width - dev_tools_width;
            
                rectangle(
                    [255.0, 255.0, 255.0, 255.0],
                    [0.0, 0.0, dev_tools_width, window_size.height as f64],
                    context.transform.trans(dev_tools_x, 0.0),
                    graphics,
                );

                // split newlines
                let mut lines = el_txt.split("\n");
                for (i, line) in lines.enumerate() {
                    text::Text::new_color([0.0, 0.0,0.0, 255.0], 2 * ((14.0 - 2.0) as u32))
                        .draw(
                            &line,
                            glyphs,
                            &context.draw_state,
                            context
                                .transform
                                    .trans(dev_tools_x,64.0 + 14.0 * i as f64)
                                    .zoom(0.5),
                            graphics,
                        )
                        .unwrap();
                }
            
            }
        });
    }
}
