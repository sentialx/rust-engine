use std::cell::RefCell;
use std::env;
use std::fs;
use std::rc::Rc;

use graviton::colors::ColorTupleA;
use graviton::layout::Rect;
use graviton::render_frame::{BoxTextMeasurer, RenderFrame};

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

    let mut text_measurer = BoxTextMeasurer::default();

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
        // Skip zero-dimension elements (Chromium doesn't render them)
        if item.width == 0.0 || item.height == 0.0 {
            continue;
        }
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

        // Note: Text segments are not rendered by default since text is not visible
        // in Chromium baselines. Enable with OFFSCREEN_DEBUG_TEXT=1 for debugging.
        if env::var("OFFSCREEN_DEBUG_TEXT").is_ok() && !item.text_segments.is_empty() {
            for seg in &item.text_segments {
                svg.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"rgba(255,0,0,0.5)\"/>",
                    seg.x, seg.y, seg.width, seg.height
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

