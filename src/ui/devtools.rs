use crate::html::{parse_html, DomElement, NodeType};
use crate::layout::Rect;
use crate::renderer::{RenderedBuffer, SkiaRenderer};
use crate::ui::browser_window::RenderFrameState;

use std::cell::RefCell;
use std::rc::Rc;

pub struct DevtoolsOverlay {
    render: RenderFrameState,
}

impl DevtoolsOverlay {
    pub fn new(viewport: Rect) -> Self {
        Self {
            render: RenderFrameState::new(viewport),
        }
    }

    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.render.set_viewport(width, height);
    }

    pub fn render_if_needed(&mut self, renderer: &mut SkiaRenderer) {
        self.render.render_if_needed(renderer);
    }

    pub fn buffer(&self) -> Option<&RenderedBuffer> {
        self.render.buffer()
    }

    pub fn invalidate(&mut self) {
        self.render.invalidate();
    }

    pub fn rebuild(&mut self, hover_info: Option<&Rc<RefCell<DomElement>>>, viewport: Rect, page_height: f32) {
        let overlay_frame = self.render.frame_mut();
        overlay_frame.set_viewport(viewport.width, viewport.height);
        overlay_frame.dom_tree = parse_html("<html><body></body></html>");
        overlay_frame.default_styles = vec![];
        overlay_frame.parsed_css = vec![];
        overlay_frame.styles = vec![];

        if let Some(body) = find_first_tag(&overlay_frame.dom_tree, "BODY") {
            let body_style = format!(
                "display:block; margin:0px; position:relative; width:{}px; height:{}px; background: rgba(0,0,0,0);",
                viewport.width, page_height
            );
            body.borrow_mut().set_attribute("style", &body_style);

            if let Some(info) = hover_info {
                let element = info.borrow();
                let Some(flow) = element.computed_flow.as_ref() else {
                    return;
                };
                let Some(style) = element.computed_style.as_ref() else {
                    return;
                };

                let margin_box = clamp_rect(flow.hover_rect.clone());
                let border_box = clamp_rect(Rect {
                    x: flow.x,
                    y: flow.y,
                    width: flow.width,
                    height: flow.height,
                });
                let content_box = clamp_rect(Rect {
                    x: flow.x + style.padding.left,
                    y: flow.y + style.padding.top,
                    width: flow.width - style.padding.left - style.padding.right,
                    height: flow.height - style.padding.top - style.padding.bottom,
                });

                // Chromium-style overlay: margin (orange), padding (green), content (blue)
                add_inset_overlay(&body, margin_box, border_box.clone(), "rgba(255, 200, 0, 0.35)");
                add_inset_overlay(&body, border_box.clone(), content_box.clone(), "rgba(77, 200, 0, 0.35)");
                add_overlay_box(&body, content_box.x, content_box.y, content_box.width, content_box.height, "rgba(0, 128, 255, 0.35)");

                let mut text_segments = Vec::new();
                collect_text_segments(&element, &mut text_segments);
                for seg in &text_segments {
                    add_border_box(&body, seg.x, seg.y, seg.width, seg.height, 1.0, "rgba(0,204,0,1.0)");
                }
            }
        }

        overlay_frame.full_layout();
        self.render.invalidate();
    }
}

fn find_first_tag(
    tree: &Vec<Rc<RefCell<DomElement>>>,
    tag_name: &str,
) -> Option<Rc<RefCell<DomElement>>> {
    for node in tree {
        let node_ref = node.borrow();
        if node_ref.tag_name == tag_name {
            return Some(node.clone());
        }
        if !node_ref.children.is_empty() {
            if let Some(found) = find_first_tag(&node_ref.children, tag_name) {
                return Some(found);
            }
        }
    }
    None
}

fn add_overlay_box(
    parent: &Rc<RefCell<DomElement>>,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    color: &str,
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let div = DomElement::create("div");
    let style = format!(
        "display:block; position:absolute; left:{}px; top:{}px; width:{}px; height:{}px; background:{};",
        left, top, width, height, color
    );
    div.borrow_mut().set_attribute("style", &style);
    parent.borrow_mut().append_child(div);
}

fn add_border_box(
    parent: &Rc<RefCell<DomElement>>,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    border: f32,
    color: &str,
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let bw = border.max(1.0);
    let bw_x = bw.min(width);
    let bw_y = bw.min(height);

    add_overlay_box(parent, left, top, width, bw_y, color);
    add_overlay_box(parent, left, top + height - bw_y, width, bw_y, color);
    add_overlay_box(parent, left, top, bw_x, height, color);
    add_overlay_box(parent, left + width - bw_x, top, bw_x, height, color);
}

fn add_inset_overlay(
    parent: &Rc<RefCell<DomElement>>,
    outer: Rect,
    inner: Rect,
    color: &str,
) {
    let outer_right = outer.x + outer.width;
    let outer_bottom = outer.y + outer.height;
    let inner_right = inner.x + inner.width;
    let inner_bottom = inner.y + inner.height;

    // Top band
    if inner.y > outer.y {
        add_overlay_box(parent, outer.x, outer.y, outer.width, inner.y - outer.y, color);
    }
    // Bottom band
    if inner_bottom < outer_bottom {
        add_overlay_box(
            parent,
            outer.x,
            inner_bottom,
            outer.width,
            outer_bottom - inner_bottom,
            color,
        );
    }
    // Left band
    if inner.x > outer.x {
        add_overlay_box(parent, outer.x, inner.y, inner.x - outer.x, inner.height, color);
    }
    // Right band
    if inner_right < outer_right {
        add_overlay_box(
            parent,
            inner_right,
            inner.y,
            outer_right - inner_right,
            inner.height,
            color,
        );
    }
}

fn clamp_rect(rect: Rect) -> Rect {
    Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width.max(0.0),
        height: rect.height.max(0.0),
    }
}

fn collect_text_segments(element: &DomElement, out: &mut Vec<Rect>) {
    for child_rc in &element.children {
        let child = child_rc.borrow();
        if child.node_type == NodeType::Text {
            for seg in &child.text_segments {
                if seg.width > 0.0 && seg.height > 0.0 {
                    out.push(Rect {
                        x: seg.x,
                        y: seg.y,
                        width: seg.width,
                        height: seg.height,
                    });
                }
            }
        } else {
            collect_text_segments(&child, out);
        }
    }
}
