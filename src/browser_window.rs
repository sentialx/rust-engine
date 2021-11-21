use crate::colors::*;
use crate::css::*;
use crate::debug::*;
use crate::html::*;
use crate::layout::*;
use crate::styles::*;
use std::collections::HashMap;

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

    add_font("Times New Roman 400.ttf");
    add_font("Times New Roman 700.ttf");
    add_font("Times New Roman Italique 400.ttf");
    add_font("Times New Roman Italique 700.ttf");

    let mut render_array: Vec<RenderItem> = vec![];
    let mut dom_tree: RefCell<Vec<DomElement>> = RefCell::new(vec![]);
    let mut parsed_css: RefCell<Vec<StyleRule>> = RefCell::new(vec![]);

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

    let mut rerender = || {
        let mut closure_ref = RefCell::new(|text: String, font_size: f64, font_family: String| {
            let mut glyphs_map = glyphs_map.borrow_mut();
            let mut glyphs = glyphs_map.get_mut(font_family.as_str()).unwrap();
            return 0.5 * glyphs.width(2 * (font_size - 2.0) as u32, &text).unwrap();
        });
        let render_array: Vec<RenderItem> = get_render_array(
            &mut dom_tree.borrow_mut(),
            [default_styles.clone(), parsed_css.borrow_mut().clone()].concat(),
            &move |text, font_size, font_family| {
                return (
                    (closure_ref.borrow_mut())(text, font_size, font_family),
                    font_size - 2.0 + 8.0,
                );
            },
            None,
        )
        .into_iter()
        .filter(|i| i.render)
        .collect();

        return render_array;
    };

    let mut refresh = |u: String| {
        let contents = fs::read_to_string(u.clone()).expect("error while reading the file");
        *dom_tree.borrow_mut() = parse_html(&contents);
        let style = get_styles(dom_tree.borrow_mut().clone(), None);
        println!("{}", style);
        *parsed_css.borrow_mut() = parse_css(&style);
        return rerender();
    };

    render_array = refresh(url.clone());

    while let Some(event) = window.next() {
        let mouse = event.mouse_cursor_args();

        if let Some(Button::Keyboard(key)) = event.press_args() {
            if key == Key::F5 {
                render_array = refresh(url.clone());
            }
        };

        if mouse.is_some() {
            if should_rerender(
                mouse.unwrap()[0],
                mouse.unwrap()[1],
                &mut dom_tree.borrow_mut(),
                parsed_css.borrow_mut().clone(),
            ) {
                render_array = rerender();
            }
        }

        window.draw_2d(&event, |context, graphics, device| {
            clear([1.0, 1.0, 1.0, 1.0], graphics);

            for item in &render_array {
                if item.background_color != (0.0, 0.0, 0.0, 0.0) {
                    rectangle(
                        color_conv(item.background_color),
                        [0.0, 0.0, item.width, item.height],
                        context.transform.trans(item.x, item.y),
                        graphics,
                    );
                }

                if item.text != "" {
                    let mut glyphs_map = glyphs_map.borrow_mut();
                    let mut glyphs = glyphs_map.get_mut(&item.font_path).unwrap();

                    let color = color_conv(item.color);

                    text::Text::new_color(color, 2 * ((item.font_size - 2.0) as u32))
                        .draw(
                            &item.text,
                            glyphs,
                            &context.draw_state,
                            context
                                .transform
                                .trans(item.x, item.y + item.height - 4.0)
                                .zoom(0.5),
                            graphics,
                        )
                        .unwrap();

                    if item.underline {
                        rectangle(
                            color,
                            [0.0, 0.0, item.width, 1.0],
                            context.transform.trans(item.x, item.y + item.height - 3.0),
                            graphics,
                        );
                    }
                    glyphs.factory.encoder.flush(device);
                }
            }
        });
    }
}
