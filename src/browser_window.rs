use crate::colors::*;
use crate::css::*;
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

#[derive(Clone, Debug)]
struct BrowserWindowInner {
  url: String,
}

pub struct BrowserWindow {
  inner: Arc<Mutex<BrowserWindowInner>>,
}

impl BrowserWindow {
  pub fn create() -> BrowserWindow {
    let browser_window = BrowserWindow {
      inner: Arc::from(Mutex::new(BrowserWindowInner {
        url: "".to_string(),
      })),
    };

    let inner = browser_window.inner.clone();

    thread::spawn(move || {
      let mut url = "".to_string();

      let mut window: PistonWindow = WindowSettings::new("Graviton", [1024, 1024])
        .exit_on_esc(true)
        .build()
        .unwrap();

      let assets = find_folder::Search::ParentsThenKids(3, 3)
        .for_folder("assets")
        .unwrap();

      let mut glyphs_map: HashMap<String, Glyphs> = HashMap::new();

      let mut add_font = |name: &str| {
        glyphs_map.insert(
          name.to_string(),
          window.load_font(assets.join(name)).unwrap(),
        )
      };

      add_font("Times New Roman 400.ttf");
      add_font("Times New Roman 700.ttf");
      add_font("Times New Roman Italique 400.ttf");
      add_font("Times New Roman Italique 700.ttf");

      let mut render_array: Vec<RenderItem> = vec![];

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

      while let Some(event) = window.next() {
        let new_url = (&inner.lock().unwrap()).url.clone();

        let mut refresh = |u: String| {
          render_array = vec![];
          let contents = fs::read_to_string(u.clone()).expect("error while reading the file");
          let dom_tree = parse_html(&contents);
          let style = get_styles(dom_tree.clone(), None);
          let parsed_css = parse_css(&style);
          let closure_ref = RefCell::new(|text: String, font_size: f64, font_family: String| {
            let mut glyphs = glyphs_map.remove(font_family.as_str()).unwrap();
            let res = glyphs.width(font_size as u32, &text).unwrap();
            glyphs_map.insert(font_family.clone(), glyphs);
            return res;
          });
          render_array = get_render_array(
            dom_tree.clone(),
            [default_styles.clone(), parsed_css].concat(),
            &move |text, font_size, font_family| {
              /*match glyphs_map.get(font_family) {
                  Some(g) => {}
                  None => {
                      glyphs_map.insert()
                  }
              }*/
              return (
                (&mut *closure_ref.borrow_mut())(text, font_size, font_family),
                font_size + 8.0,
              );
            },
            None,
          )
          .into_iter()
          .filter(|i| i.render)
          .collect();
        };
        if url != new_url {
          url = new_url;
          refresh(url.clone());
        }
        if let Some(Button::Keyboard(key)) = event.press_args() {
          if key == Key::F5 {
            refresh(url.clone());
          }
        };

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
              let mut glyphs = glyphs_map.remove(&item.font_path).unwrap();

              let color = color_conv(item.color);

              text::Text::new_color(color, 2 * item.font_size as u32)
                .draw(
                  &item.text,
                  &mut glyphs,
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
              glyphs_map.insert((&item).font_path.clone(), glyphs);
            }
          }
        });
      }
    });

    return browser_window;
  }

  pub fn load_file(&mut self, url: &str) {
    self.inner.lock().unwrap().url = url.to_string();
  }
}
