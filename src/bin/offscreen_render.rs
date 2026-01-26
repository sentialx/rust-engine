use std::cell::RefCell;
use std::env;
use std::fs;
use std::rc::Rc;

use find_folder::Search;
use piston_window::{PistonWindow, WindowSettings};

use graviton::colors::ColorTupleA;
use graviton::layout::Rect;
use graviton::render_frame::{GlyphsTextMeasurer, RenderFrame};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "Usage: offscreen_render <input.html> <output.svg> [width height]"
        );
        std::process::exit(1);
    }

    let input = &args[1];
    let output = &args[2];
    let width = args
        .get(3)
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(1366.0);
    let height = args
        .get(4)
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(768.0);

    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width,
        height,
    };

    // Initialize a minimal window so glyph cache can load
    let mut _window: PistonWindow = WindowSettings::new("Graviton Offscreen", [1, 1])
        .exit_on_esc(false)
        .build()
        .expect("failed to create offscreen window");

    let assets = Search::ParentsThenKids(3, 3)
        .for_folder("assets")
        .expect("assets folder not found");

    let glyphs_map: Rc<
        RefCell<std::collections::HashMap<String, piston_window::Glyphs<'static>>>,
    > = Rc::new(RefCell::new(
        std::collections::HashMap::<String, piston_window::Glyphs<'static>>::new(),
    ));

    let add_font = |name: &str,
                    map: &Rc<
        RefCell<std::collections::HashMap<String, piston_window::Glyphs<'static>>>,
    >| {
        let glyphs = _window
            .load_font(
                assets.join(name),
                piston_window::wgpu_graphics::TextureSettings::new(),
            )
            .unwrap();
        map.borrow_mut().insert(name.to_string(), glyphs);
    };

    add_font("Times New Roman 400.ttf", &glyphs_map);
    add_font("Times New Roman 700.ttf", &glyphs_map);
    add_font("Times New Roman Italique 400.ttf", &glyphs_map);
    add_font("Times New Roman Italique 700.ttf", &glyphs_map);

    let mut text_measurer = GlyphsTextMeasurer {
        glyphs_map: glyphs_map.clone(),
    };

    let mut render_frame = RenderFrame::new(viewport, &mut text_measurer);
    render_frame.load_url(input);

    if env::var("OFFSCREEN_DEBUG").is_ok() {
        dump_layout(&render_frame.dom_tree, 0);
    }

    let svg = render_to_svg(&render_frame);
    fs::write(output, svg).expect("failed to write SVG");
    println!("Wrote SVG to {}", output);
}

fn render_to_svg(render_frame: &RenderFrame) -> String {
    let mut svg = String::new();
    let width = render_frame.viewport.width;
    let height = render_frame.viewport.height;

    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">",
        width, height, width, height
    ));
    svg.push_str("<rect x=\"0\" y=\"0\" width=\"100%\" height=\"100%\" fill=\"white\" />");

    for item in &render_frame.render_array {
        if item.background_color != (0.0, 0.0, 0.0, 0.0) {
            // Extract class name from element if available
            let class_attr = item
                .element
                .as_ref()
                .map(|el| {
                    let el = el.borrow();
                    el.class_list.join(" ")
                })
                .filter(|s| !s.is_empty())
                .map(|c| format!(" data-class=\"{}\"", c))
                .unwrap_or_default();

            svg.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"{}/>",
                item.x,
                item.y,
                item.width,
                item.height,
                color_to_rgba(item.background_color),
                class_attr
            ));
        }

        if !item.text_segments.is_empty() {
            let font_family = font_family_from_path(&item.font_path);
            let font_weight = font_weight_from_path(&item.font_path);
            let text_color = color_to_rgba(item.color);
            let decoration = if item.underline { "underline" } else { "none" };

            for seg in &item.text_segments {
                if seg.text.is_empty() {
                    continue;
                }
                // SVG text y-coordinate is the baseline (top + ascent)
                let y = seg.y + seg.ascent;
                svg.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" font-family=\"{}\" font-size=\"{}\" font-weight=\"{}\" fill=\"{}\" text-decoration=\"{}\">{}</text>",
                    seg.x,
                    y,
                    font_family,
                    item.font_size,
                    font_weight,
                    text_color,
                    decoration,
                    escape_xml(&seg.text)
                ));
            }
        }
    }

    svg.push_str("</svg>");
    svg
}

fn dump_layout(tree: &Vec<Rc<RefCell<graviton::html::DomElement>>>, depth: usize) {
    for node in tree {
        let el = node.borrow();
        let class_name = el.attributes.get("class").cloned().unwrap_or_default();
        let is_text = el.node_type == graviton::html::NodeType::Text;
        if !class_name.is_empty() || el.tag_name == "BODY" || el.tag_name == "DIV" {
            if let Some(flow) = &el.computed_flow {
                let width_auto = el
                    .inherited_style
                    .as_ref()
                    .map(|style| !style.width.has_numeric_value())
                    .unwrap_or(false);
                let padding = el
                    .computed_style
                    .as_ref()
                    .map(|style| style.padding.clone());
                let padding_desc = padding
                    .as_ref()
                    .map(|p| format!("pad(l={},r={})", p.left, p.right))
                    .unwrap_or_else(|| "pad(l=?,r=?)".to_string());
                println!(
                    "{}{} class=\"{}\" x={} y={} w={} h={} width_auto={} {}",
                    "  ".repeat(depth),
                    el.tag_name,
                    class_name,
                    flow.x,
                    flow.y,
                    flow.width,
                    flow.height,
                    width_auto,
                    padding_desc
                );
            }
        }
        if is_text {
            if let Some(flow) = &el.computed_flow {
                let text_preview = el.node_value.replace('\n', "\\n");
                println!(
                    "{}#text \"{}\" x={} y={} w={} h={} segments={}",
                    "  ".repeat(depth),
                    text_preview,
                    flow.x,
                    flow.y,
                    flow.width,
                    flow.height,
                    el.text_segments.len()
                );
                for seg in &el.text_segments {
                    println!(
                        "{}  seg \"{}\" x={} y={} w={} h={}",
                        "  ".repeat(depth),
                        seg.text.replace('\n', "\\n"),
                        seg.x,
                        seg.y,
                        seg.width,
                        seg.height
                    );
                }
            }
        }
        if !el.children.is_empty() {
            dump_layout(&el.children, depth + 1);
        }
    }
}

fn color_to_rgba(color: ColorTupleA) -> String {
    let r = color.0.clamp(0.0, 255.0);
    let g = color.1.clamp(0.0, 255.0);
    let b = color.2.clamp(0.0, 255.0);
    let mut a = color.3;
    if a > 1.0 {
        a = (a / 255.0).clamp(0.0, 1.0);
    }
    format!("rgba({},{},{},{})", r as i32, g as i32, b as i32, a)
}

fn font_family_from_path(path: &str) -> &str {
    if path.contains("Times New Roman") {
        "Times New Roman"
    } else {
        "Times New Roman"
    }
}

fn font_weight_from_path(path: &str) -> &'static str {
    if path.contains("700") {
        "700"
    } else {
        "400"
    }
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
