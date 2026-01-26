use std::cell::RefCell;
use std::env;
use std::fs;
use std::rc::Rc;

use graviton::colors::ColorTupleA;
use graviton::frame::Frame;
use graviton::layout::Rect;

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

    let mut frame = Frame::new(viewport);
    frame.load_url(input);

    if env::var("OFFSCREEN_DEBUG").is_ok() {
        dump_layout(&frame.dom_tree, 0);
    }

    let svg = render_to_svg(&frame);
    fs::write(output, svg).expect("failed to write SVG");
    println!("Wrote SVG to {}", output);
}

fn render_to_svg(frame: &Frame) -> String {
    let mut svg = String::new();
    let width = frame.viewport.width;
    let height = frame.viewport.height;

    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">",
        width, height, width, height
    ));
    svg.push_str("<rect x=\"0\" y=\"0\" width=\"100%\" height=\"100%\" fill=\"white\" />");

    for item in &frame.render_array {
        // Skip zero-dimension elements (Chromium doesn't render them)
        if item.width == 0.0 || item.height == 0.0 {
            continue;
        }

        // Render box shadow (before background)
        if item.box_shadow.is_visible() && !item.box_shadow.inset {
            render_box_shadow_svg(
                &mut svg,
                item.x + item.box_shadow.offset_x,
                item.y + item.box_shadow.offset_y,
                item.width + item.box_shadow.spread_radius * 2.0,
                item.height + item.box_shadow.spread_radius * 2.0,
                item.box_shadow.blur_radius,
                item.box_shadow.color,
            );
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

        // Render borders
        let border = &item.border;

        // Top border
        if border.top.is_visible() {
            render_border_line_svg(
                &mut svg, item.x, item.y, item.width, border.top.width,
                true, &border.top.style, border.top.color
            );
        }

        // Bottom border
        if border.bottom.is_visible() {
            render_border_line_svg(
                &mut svg, item.x, item.y + item.height - border.bottom.width,
                item.width, border.bottom.width,
                true, &border.bottom.style, border.bottom.color
            );
        }

        // Left border
        if border.left.is_visible() {
            render_border_line_svg(
                &mut svg, item.x, item.y, border.left.width, item.height,
                false, &border.left.style, border.left.color
            );
        }

        // Right border
        if border.right.is_visible() {
            render_border_line_svg(
                &mut svg, item.x + item.width - border.right.width, item.y,
                border.right.width, item.height,
                false, &border.right.style, border.right.color
            );
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

fn render_box_shadow_svg(
    svg: &mut String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    blur_radius: f32,
    color: ColorTupleA,
) {
    if blur_radius <= 0.0 {
        // No blur - just draw a solid shadow
        svg.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
            x, y, width, height, color_to_rgba(color)
        ));
        return;
    }

    // Approximate blur with multiple layers
    let layers = (blur_radius as i32).min(10).max(3);
    let step = blur_radius / layers as f32;

    for i in 0..layers {
        let offset = step * (layers - i) as f32;
        let alpha_factor = (i + 1) as f32 / (layers + 1) as f32;

        let layer_color = (
            color.0,
            color.1,
            color.2,
            color.3 * alpha_factor * 0.5,
        );

        svg.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
            x - offset,
            y - offset,
            width + offset * 2.0,
            height + offset * 2.0,
            color_to_rgba(layer_color)
        ));
    }
}

fn render_border_line_svg(
    svg: &mut String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    horizontal: bool,
    style: &str,
    color: ColorTupleA,
) {
    let fill = color_to_rgba(color);

    match style {
        "dashed" => {
            let border_width = if horizontal { height } else { width };
            let dash_len = (border_width * 3.0).max(3.0);
            let gap_len = dash_len;

            if horizontal {
                let mut cx = x;
                while cx < x + width {
                    let segment_width = dash_len.min(x + width - cx);
                    svg.push_str(&format!(
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
                        cx, y, segment_width, height, fill
                    ));
                    cx += dash_len + gap_len;
                }
            } else {
                let mut cy = y;
                while cy < y + height {
                    let segment_height = dash_len.min(y + height - cy);
                    svg.push_str(&format!(
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
                        x, cy, width, segment_height, fill
                    ));
                    cy += dash_len + gap_len;
                }
            }
        }
        "dotted" => {
            let border_width = if horizontal { height } else { width };
            let dot_size = border_width.max(1.0);
            let gap_len = dot_size;

            if horizontal {
                let mut cx = x;
                while cx < x + width {
                    svg.push_str(&format!(
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
                        cx, y, dot_size, height, fill
                    ));
                    cx += dot_size + gap_len;
                }
            } else {
                let mut cy = y;
                while cy < y + height {
                    svg.push_str(&format!(
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
                        x, cy, width, dot_size, fill
                    ));
                    cy += dot_size + gap_len;
                }
            }
        }
        _ => {
            // solid (default)
            svg.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
                x, y, width, height, fill
            ));
        }
    }
}

