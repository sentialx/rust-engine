use crate::dom::{DomElement, NodeType};
use crate::html::parse_html;
use crate::layout::Rect;
use crate::frame::Frame;
use crate::renderer::{RenderedBuffer, SkiaRenderer};
use crate::styles::ComputedStyle;
use crate::ui::browser_window::RenderFrameState;

use std::cell::RefCell;
use std::rc::Rc;

/// Overlay for highlighting elements - render only, no event handling
pub struct DevtoolsOverlay {
    render: RenderFrameState,
    has_content: bool,
}

impl DevtoolsOverlay {
    pub fn new(viewport: Rect) -> Self {
        Self {
            render: RenderFrameState::new_render_only(viewport),
            has_content: false,
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
        self.render.set_viewport(viewport.width, viewport.height);

        if hover_info.is_none() {
            if self.has_content {
                let mut frame = self.render.frame_mut();
                frame.dom_tree.clear();
                frame.render_array.clear();
                frame.page_height = 0.0;
                frame.cached_layout_tree = None;
                drop(frame);
                self.render.invalidate();
                self.has_content = false;
            }
            return;
        }

        self.has_content = true;
        {
            let mut frame = self.render.frame_mut();
            let ir_nodes = parse_html("<html><body></body></html>");
            frame.dom_tree = frame.build_dom_from_ir(&ir_nodes, None);
            frame.default_styles = vec![];
            frame.parsed_css = vec![];
            frame.styles = vec![];
        }

        // Find body element
        let dom_tree = self.render.frame().dom_tree.clone();
        let Some(body) = find_first_tag(&dom_tree, "BODY") else {
            return;
        };

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

            // Build overlay DOM elements (borrow frame for create_element calls)
            let popup = {
                let frame = self.render.frame();

                // Chromium-style overlay: margin (orange), padding (green), content (blue)
                add_inset_overlay(&frame, &body, margin_box, border_box.clone(), "rgba(255, 200, 0, 0.35)");
                add_inset_overlay(&frame, &body, border_box.clone(), content_box.clone(), "rgba(77, 200, 0, 0.35)");
                add_overlay_box(&frame, &body, content_box.x, content_box.y, content_box.width, content_box.height, "rgba(0, 128, 255, 0.35)");

                let mut text_segments = Vec::new();
                collect_text_segments(&element, &mut text_segments);
                for seg in &text_segments {
                    add_border_box(&frame, &body, seg.x, seg.y, seg.width, seg.height, 1.0, "dotted", "rgba(255,0,128,1.0)");
                }

                // Add info popup (position will be adjusted after layout)
                let popup_x = border_box.x.min(viewport.width - 300.0).max(8.0);
                add_info_popup(
                    &frame,
                    &body,
                    popup_x,
                    0.0, // temporary y position
                    &element,
                    flow.width,
                    flow.height,
                    style,
                )
            };

            // Drop borrow of element before mutably borrowing frame
            drop(element);

            // Do layout to get actual popup height
            self.render.frame_mut().full_layout();

            // Get computed popup height and reposition
            let popup_height = popup
                .borrow()
                .computed_flow
                .as_ref()
                .map(|f| f.height)
                .unwrap_or(100.0);

            let above_y = border_box.y - popup_height - 8.0;
            let below_y = border_box.y + border_box.height + 8.0;

            let popup_y = if above_y >= 8.0 {
                above_y
            } else if below_y + popup_height < page_height {
                below_y
            } else {
                8.0_f32.max(above_y)
            };

            // Update popup position
            {
                let mut popup_ref = popup.borrow_mut();
                let current_style = popup_ref.attributes.get("style").cloned().unwrap_or_default();
                let new_style = current_style.replace("top:0px;", &format!("top:{}px;", popup_y));
                popup_ref.set_attribute("style", &new_style);
            }
        }

        self.render.frame_mut().full_layout();
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
    frame: &Frame,
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
    let div = frame.create_element("div");
    let style = format!(
        "display:block; position:absolute; left:{}px; top:{}px; width:{}px; height:{}px; background:{};",
        left, top, width, height, color
    );
    div.borrow_mut().set_attribute("style", &style);
    parent.borrow_mut().append_child(div);
}

fn add_border_box(
    frame: &Frame,
    parent: &Rc<RefCell<DomElement>>,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    border_width: f32,
    border_style: &str,
    color: &str,
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let bw = border_width.max(1.0);
    let div = frame.create_element("div");
    let style = format!(
        "display:block; position:absolute; left:{}px; top:{}px; width:{}px; height:{}px; \
         border: {}px {} {};",
        left, top, width - bw * 2.0, height - bw * 2.0, bw, border_style, color
    );
    div.borrow_mut().set_attribute("style", &style);
    parent.borrow_mut().append_child(div);
}

fn add_inset_overlay(
    frame: &Frame,
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
        add_overlay_box(frame, parent, outer.x, outer.y, outer.width, inner.y - outer.y, color);
    }
    // Bottom band
    if inner_bottom < outer_bottom {
        add_overlay_box(
            frame,
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
        add_overlay_box(frame, parent, outer.x, inner.y, inner.x - outer.x, inner.height, color);
    }
    // Right band
    if inner_right < outer_right {
        add_overlay_box(
            frame,
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

fn add_info_popup(
    frame: &Frame,
    parent: &Rc<RefCell<DomElement>>,
    left: f32,
    top: f32,
    element: &DomElement,
    width: f32,
    height: f32,
    style: &ComputedStyle,
) -> Rc<RefCell<DomElement>> {
    let tag_name = &element.tag_name;
    let popup = frame.create_element("div");
    let popup_style = format!(
        "display:block; position:absolute; left:{}px; top:{}px; width:280px; \
         background:rgba(255,255,255,0.97); padding:10px; \
         box-shadow: 0 2px 4px rgba(0,0,0,0.08);",
        left, top
    );
    popup.borrow_mut().set_attribute("style", &popup_style);

    // Header: selector + dimensions (Chrome DevTools style)
    let header = frame.create_element("div");
    header.borrow_mut().set_attribute("style", "display:block; margin-bottom:8px;");

    // Tag name in purple/magenta
    let tag_span = frame.create_element("span");
    tag_span.borrow_mut().set_attribute(
        "style",
        "color:#881280; font-weight:700; font-size:13px;",
    );
    tag_span.borrow_mut().set_text_content(&tag_name.to_lowercase());
    header.borrow_mut().append_child(tag_span);

    // ID in blue (if present)
    if let Some(id) = element.attributes.get("id") {
        if !id.is_empty() {
            let id_span = frame.create_element("span");
            id_span.borrow_mut().set_attribute(
                "style",
                "color:#1a1aa6; font-weight:700; font-size:13px;",
            );
            id_span.borrow_mut().set_text_content(&format!("#{}", id));
            header.borrow_mut().append_child(id_span);
        }
    }

    // Classes in dark gray
    for class in &element.class_list {
        if !class.is_empty() {
            let class_span = frame.create_element("span");
            class_span.borrow_mut().set_attribute(
                "style",
                "color:#1a1a1a; font-weight:700; font-size:13px;",
            );
            class_span.borrow_mut().set_text_content(&format!(".{}", class));
            header.borrow_mut().append_child(class_span);
        }
    }

    // Dimensions
    let dims_span = frame.create_element("span");
    dims_span.borrow_mut().set_attribute(
        "style",
        "color:#666; font-size:12px; margin-left:8px;",
    );
    dims_span.borrow_mut().set_text_content(&format!("{:.0} \u{00D7} {:.0}", width, height));
    header.borrow_mut().append_child(dims_span);

    popup.borrow_mut().append_child(header);

    // Color row
    let (r, g, b, _a) = style.color;
    let color_hex = format!("#{:02x}{:02x}{:02x}", r as u8, g as u8, b as u8);
    add_property_row(frame, &popup, "Color", &color_hex, Some(style.color));

    // Background color row (if not transparent)
    let (bg_r, bg_g, bg_b, bg_a) = style.background_color;
    if bg_a > 0.0 {
        let bg_hex = format!("#{:02x}{:02x}{:02x}", bg_r as u8, bg_g as u8, bg_b as u8);
        add_property_row(frame, &popup, "Background", &bg_hex, Some(style.background_color));
    }

    // Font row
    let font_info = format!(
        "{}px {}",
        style.font_size as i32,
        style.font_family
    );
    add_property_row(frame, &popup, "Font", &font_info, None);

    // Margin row
    let margin = &style.margin;
    let margin_str = if margin.top == margin.right && margin.right == margin.bottom && margin.bottom == margin.left {
        format!("{}px", margin.top as i32)
    } else if margin.top == margin.bottom && margin.left == margin.right {
        format!("{}px {}px", margin.top as i32, margin.left as i32)
    } else {
        format!(
            "{}px {}px {}px {}px",
            margin.top as i32,
            margin.right as i32,
            margin.bottom as i32,
            margin.left as i32
        )
    };
    add_property_row(frame, &popup, "Margin", &margin_str, None);

    // Padding row (if non-zero)
    let padding = &style.padding;
    if padding.top != 0.0 || padding.right != 0.0 || padding.bottom != 0.0 || padding.left != 0.0 {
        let padding_str = if padding.top == padding.right && padding.right == padding.bottom && padding.bottom == padding.left {
            format!("{}px", padding.top as i32)
        } else if padding.top == padding.bottom && padding.left == padding.right {
            format!("{}px {}px", padding.top as i32, padding.left as i32)
        } else {
            format!(
                "{}px {}px {}px {}px",
                padding.top as i32,
                padding.right as i32,
                padding.bottom as i32,
                padding.left as i32
            )
        };
        add_property_row(frame, &popup, "Padding", &padding_str, None);
    }

    // Border row (if visible)
    let border = &style.border;
    if border.has_visible_border() {
        let border_str = if border.top.width == border.right.width
            && border.right.width == border.bottom.width
            && border.bottom.width == border.left.width
        {
            format!("{}px {}", border.top.width as i32, border.top.style)
        } else {
            format!(
                "{}px {}px {}px {}px",
                border.top.width as i32,
                border.right.width as i32,
                border.bottom.width as i32,
                border.left.width as i32
            )
        };
        add_property_row(frame, &popup, "Border", &border_str, Some(border.top.color));
    }

    parent.borrow_mut().append_child(popup.clone());
    popup
}

fn add_property_row(
    frame: &Frame,
    parent: &Rc<RefCell<DomElement>>,
    label: &str,
    value: &str,
    color_swatch: Option<(f32, f32, f32, f32)>,
) {
    let row = frame.create_element("div");
    row.borrow_mut().set_attribute(
        "style",
        "display:block; margin-bottom:4px; font-size:11px;",
    );

    let label_span = frame.create_element("span");
    label_span.borrow_mut().set_attribute(
        "style",
        "color:#888; width:60px; display:inline-block;",
    );
    label_span.borrow_mut().set_text_content(label);
    row.borrow_mut().append_child(label_span);

    if let Some((r, g, b, _a)) = color_swatch {
        let swatch = frame.create_element("span");
        swatch.borrow_mut().set_attribute(
            "style",
            &format!(
                "display:inline-block; width:12px; height:12px; \
                 background:rgb({},{},{}); margin-right:4px; border: 1px solid rgba(0, 0, 0, 0.15);",
                r as u8, g as u8, b as u8
            ),
        );
        row.borrow_mut().append_child(swatch);
    }

    let value_span = frame.create_element("span");
    value_span.borrow_mut().set_attribute("style", "color:#333;");
    value_span.borrow_mut().set_text_content(value);
    row.borrow_mut().append_child(value_span);

    parent.borrow_mut().append_child(row);
}
