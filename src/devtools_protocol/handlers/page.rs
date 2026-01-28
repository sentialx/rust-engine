// Page domain handlers

use serde_json::{json, Value};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::devtools_protocol::{DevtoolsServer, types::*};
use crate::frame::Frame;
use crate::renderer::HybridRenderer;

/// Handle Page domain commands
pub fn handle(
    server: &mut DevtoolsServer,
    frame: &mut Frame,
    renderer: Option<&mut HybridRenderer>,
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
            let Some(renderer) = renderer else {
                return Response::error(id, ERROR_INTERNAL, "Renderer not available for screenshots");
            };

            let _format = params.get("format")
                .and_then(|v| v.as_str())
                .unwrap_or("png");

            // Render frame to texture
            const SCREENSHOT_FRAME_ID: usize = 999;
            renderer.render(frame, SCREENSHOT_FRAME_ID, 0.0);

            // Read pixels back from GPU
            let (pixels, width, height) = match renderer.read_pixels(SCREENSHOT_FRAME_ID) {
                Some(data) => data,
                None => return Response::error(id, ERROR_INTERNAL, "Failed to read pixels from GPU"),
            };

            // Create pixmap and encode to PNG
            let mut pixmap = match tiny_skia::Pixmap::new(width, height) {
                Some(p) => p,
                None => return Response::error(id, ERROR_INTERNAL, "Failed to create pixmap"),
            };

            // Copy RGBA pixels to pixmap
            for (i, chunk) in pixels.chunks(4).enumerate() {
                if chunk.len() == 4 {
                    pixmap.pixels_mut()[i] = tiny_skia::PremultipliedColorU8::from_rgba(
                        chunk[0], chunk[1], chunk[2], chunk[3]
                    ).unwrap();
                }
            }

            let png_data = match pixmap.encode_png() {
                Ok(data) => data,
                Err(_) => return Response::error(id, ERROR_INTERNAL, "Failed to encode PNG"),
            };

            let base64_data = BASE64.encode(&png_data);
            Response::success(id, json!({ "data": base64_data }))
        }

        "getFrameTree" => {
            // Return basic frame tree for Chrome DevTools
            let frame_tree = json!({
                "frameTree": {
                    "frame": {
                        "id": "main",
                        "loaderId": "1",
                        "url": if frame.url.is_empty() { "about:blank" } else { &frame.url },
                        "domainAndRegistry": "",
                        "securityOrigin": "://",
                        "mimeType": "text/html",
                        "secureContextType": "Secure",
                        "crossOriginIsolatedContextType": "NotIsolated",
                        "gatedAPIFeatures": []
                    },
                    "childFrames": []
                }
            });
            Response::success(id, frame_tree)
        }

        "getResourceTree" => {
            // Return basic resource tree for Chrome DevTools
            let url = if frame.url.is_empty() { "about:blank".to_string() } else { frame.url.clone() };
            let resource_tree = json!({
                "frameTree": {
                    "frame": {
                        "id": "main",
                        "loaderId": "1",
                        "url": url.clone(),
                        "domainAndRegistry": "",
                        "securityOrigin": "://",
                        "mimeType": "text/html",
                        "secureContextType": "Secure",
                        "crossOriginIsolatedContextType": "NotIsolated",
                        "gatedAPIFeatures": []
                    },
                    "childFrames": [],
                    "resources": [{
                        "url": url,
                        "type": "Document",
                        "mimeType": "text/html"
                    }]
                }
            });
            Response::success(id, resource_tree)
        }

        "getNavigationHistory" => {
            // Return basic navigation history
            let url = if frame.url.is_empty() { "about:blank".to_string() } else { frame.url.clone() };
            Response::success(id, json!({
                "currentIndex": 0,
                "entries": [{
                    "id": 0,
                    "url": url,
                    "userTypedURL": url,
                    "title": "Graviton",
                    "transitionType": "typed"
                }]
            }))
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
