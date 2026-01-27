// Page domain handlers

use serde_json::{json, Value};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::devtools_protocol::{DevtoolsServer, types::*};
use crate::frame::Frame;
use crate::layout::Rect;
use crate::renderer::{Renderer, SkiaRenderer, CompositeRegion};
use crate::ui::devtools::DevtoolsOverlay;

/// Handle Page domain commands
pub fn handle(
    server: &mut DevtoolsServer,
    frame: &mut Frame,
    id: u64,
    command: &str,
    params: &Value,
) -> Response {
    match command {
        "enable" => {
            Response::success(id, json!({}))
        }

        "disable" => {
            Response::success(id, json!({}))
        }

        "navigate" => {
            let url = match params.get("url").and_then(|v| v.as_str()) {
                Some(u) => u,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Missing url parameter"),
            };

            // Clear node registry on navigation
            server.clear_registry();

            // Load the new URL
            frame.load_url(url);

            Response::success(id, json!({
                "frameId": "main",
                "loaderId": "1"
            }))
        }

        "reload" => {
            // Clear node registry on reload
            server.clear_registry();

            frame.refresh();

            Response::success(id, json!({}))
        }

        "captureScreenshot" => {
            let format = params.get("format")
                .and_then(|v| v.as_str())
                .unwrap_or("png");

            let _quality = params.get("quality")
                .and_then(|v| v.as_i64())
                .unwrap_or(100) as u8;

            // Create a renderer and render the frame
            let mut renderer = SkiaRenderer::new();
            let width = frame.viewport.width as u32;
            let height = frame.viewport.height as u32;
            let page_height = frame.page_height;

            renderer.resize(width, height, 1.0);
            let main_buffer = renderer.render(frame);

            // Check if we need to render an overlay
            let highlighted = server.get_highlighted_node();

            let final_pixmap = if highlighted.is_some() {
                // Create overlay with highlighted element
                let viewport = Rect {
                    x: 0.0,
                    y: 0.0,
                    width: frame.viewport.width,
                    height: frame.viewport.height,
                };
                let mut overlay = DevtoolsOverlay::new(viewport.clone());
                overlay.set_viewport(viewport.width, viewport.height);
                overlay.rebuild(highlighted.as_ref(), viewport, page_height);
                overlay.render(&mut renderer);

                // Composite main + overlay
                let mut regions = vec![
                    CompositeRegion {
                        buffer: &main_buffer,
                        dest_x: 0.0,
                        scroll_y: 0.0,
                    },
                ];

                if let Some(overlay_buffer) = overlay.buffer() {
                    regions.push(CompositeRegion {
                        buffer: overlay_buffer,
                        dest_x: 0.0,
                        scroll_y: 0.0,
                    });
                }

                renderer.composite(&regions);

                // Get the composited result - we need to create a pixmap from display buffer
                let display = renderer.get_display_buffer();
                let mut pixmap = tiny_skia::Pixmap::new(width, height).unwrap();
                for (i, &pixel) in display.iter().enumerate() {
                    let r = ((pixel >> 16) & 0xFF) as u8;
                    let g = ((pixel >> 8) & 0xFF) as u8;
                    let b = (pixel & 0xFF) as u8;
                    pixmap.pixels_mut()[i] = tiny_skia::ColorU8::from_rgba(r, g, b, 255).premultiply();
                }
                pixmap
            } else {
                main_buffer.pixmap
            };

            // Encode to PNG
            let png_data = match format {
                "png" => encode_pixmap_to_png(&final_pixmap),
                "jpeg" | "jpg" => encode_pixmap_to_png(&final_pixmap),
                _ => encode_pixmap_to_png(&final_pixmap),
            };

            let base64_data = match png_data {
                Some(data) => BASE64.encode(&data),
                None => return Response::error(id, ERROR_INTERNAL, "Failed to encode screenshot"),
            };

            Response::success(id, json!({ "data": base64_data }))
        }

        "getLayoutMetrics" => {
            let viewport_width = frame.viewport.width as f64;
            let viewport_height = frame.viewport.height as f64;
            let page_height = frame.page_height as f64;

            let metrics = LayoutMetrics {
                layout_viewport: LayoutViewport {
                    page_x: 0,
                    page_y: 0,
                    client_width: viewport_width as i32,
                    client_height: viewport_height as i32,
                },
                visual_viewport: VisualViewport {
                    offset_x: 0.0,
                    offset_y: 0.0,
                    page_x: 0.0,
                    page_y: 0.0,
                    client_width: viewport_width,
                    client_height: viewport_height,
                    scale: 1.0,
                    zoom: 1.0,
                },
                content_size: ContentSize {
                    width: viewport_width,
                    height: page_height,
                },
            };

            Response::success(id, json!(metrics))
        }

        _ => Response::error(id, ERROR_METHOD_NOT_FOUND, &format!("Unknown Page method: {}", command)),
    }
}

/// Encode a tiny-skia Pixmap to PNG bytes
fn encode_pixmap_to_png(pixmap: &tiny_skia::Pixmap) -> Option<Vec<u8>> {
    pixmap.encode_png().ok()
}
