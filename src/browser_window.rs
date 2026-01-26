// Browser window - ties together Frame, Renderer, and window management

use crate::frame::{Frame, HoverInfo};
use crate::html::{parse_html, DomElement};
use crate::layout::Rect;
use crate::renderer::{CompositeRegion, RenderedBuffer, Renderer, SkiaRenderer};
use crate::utils::Debouncer;

use std::cell::RefCell;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Duration;

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use softbuffer::Surface;

struct RenderFrameState {
    frame: Frame,
    buffer: Option<RenderedBuffer>,
    dirty: bool,
}

impl RenderFrameState {
    fn new(viewport: Rect) -> Self {
        Self {
            frame: Frame::new(viewport),
            buffer: None,
            dirty: true,
        }
    }

    fn frame(&self) -> &Frame {
        &self.frame
    }

    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }

    fn invalidate(&mut self) {
        self.buffer = None;
        self.dirty = true;
    }

    fn set_viewport(&mut self, width: f32, height: f32) {
        self.frame.set_viewport(width, height);
        self.invalidate();
    }

    fn render_if_needed(&mut self, renderer: &mut SkiaRenderer) {
        if self.dirty || self.buffer.is_none() {
            self.buffer = Some(renderer.render(&self.frame));
            self.dirty = false;
        }
    }

    fn buffer(&self) -> Option<&RenderedBuffer> {
        self.buffer.as_ref()
    }
}

struct BrowserApp {
    url: String,
    window: Option<Rc<Window>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,

    // Document layout (owns text measurer and font loading)
    main: RenderFrameState,
    devtools: RenderFrameState,
    overlay: RenderFrameState,

    // Rendering
    renderer: SkiaRenderer,

    // Scroll state
    scroll_y: f32,

    // Input state
    pressed_up: bool,
    pressed_down: bool,
    mouse_x: f32,
    mouse_y: f32,

    // UI state
    devtools_visible: bool,
    devtools_panel_width: f32,
    scale_factor: f32,
    resize_debouncer: Debouncer<(f32, f32)>,
    initialized: bool,

    // Hover state
    hover_info: Option<HoverInfo>,
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
        "position:absolute; left:{}px; top:{}px; width:{}px; height:{}px; background:{};",
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

impl BrowserApp {
    fn new(url: String) -> Self {
        Self {
            url,
            window: None,
            surface: None,
            main: RenderFrameState::new(Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 }),
            devtools: RenderFrameState::new(Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 }),
            overlay: RenderFrameState::new(Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 }),
            renderer: SkiaRenderer::new(),
            scroll_y: 0.0,
            pressed_up: false,
            pressed_down: false,
            mouse_x: 0.0,
            mouse_y: 0.0,
            devtools_visible: true,
            devtools_panel_width: 300.0,
            scale_factor: 1.0,
            resize_debouncer: Debouncer::new(Duration::from_millis(1)),
            initialized: false,
            hover_info: None,
        }
    }

    fn devtools_width(&self) -> f32 {
        if self.devtools_visible {
            self.devtools_panel_width
        } else {
            0.0
        }
    }

    fn load_url(&mut self) {
        self.main.frame_mut().load_url(&self.url);
        self.main.invalidate();
    }

    fn do_reflow(&mut self) {
        self.main.frame_mut().full_layout();
        self.main.invalidate();
    }

    fn load_devtools(&mut self) {
        self.devtools.frame_mut().load_url("devtools.html");
        self.devtools.invalidate();
    }

    fn rebuild_overlay(&mut self) {
        let viewport = self.main.frame().viewport.clone();
        let page_height = self.main.frame().page_height.max(viewport.height);
        let overlay_frame = self.overlay.frame_mut();
        overlay_frame.set_viewport(viewport.width, viewport.height);
        overlay_frame.dom_tree = parse_html("<html><body></body></html>");
        overlay_frame.parsed_css = vec![];
        overlay_frame.styles = overlay_frame.default_styles.clone();

        if let Some(body) = find_first_tag(&overlay_frame.dom_tree, "BODY") {
            let body_style = format!(
                "margin:0px; position:relative; width:{}px; height:{}px; background: rgba(0,0,0,0);",
                viewport.width, page_height
            );
            body.borrow_mut().set_attribute("style", &body_style);

            if let Some(info) = &self.hover_info {
                add_overlay_box(&body, info.rect.x, info.rect.y, info.rect.width, info.rect.height, "rgba(0,128,255,0.3)");
                add_border_box(&body, info.rect.x, info.rect.y, info.rect.width, info.rect.height, 2.0, "rgba(0,128,255,1.0)");

                for seg in &info.text_segments {
                    add_border_box(&body, seg.x, seg.y, seg.width, seg.height, 1.0, "rgba(0,204,0,1.0)");
                }
            }
        }

        overlay_frame.full_layout();
        self.overlay.invalidate();
    }

    fn update_devtools_content(&mut self) {
        let (tag_text, dimension_text) = if let Some(info) = &self.hover_info {
            (
                format!("<{}>", info.tag_name.to_lowercase()),
                format!("{:.0}x{:.0}", info.rect.width, info.rect.height),
            )
        } else {
            ("--".to_string(), "--".to_string())
        };

        if let Some(el) = self.devtools.frame_mut().get_element_by_id("tag-name") {
            el.borrow_mut().set_text_content(&tag_text);
        }
        if let Some(el) = self.devtools.frame_mut().get_element_by_id("dimensions") {
            el.borrow_mut().set_text_content(&dimension_text);
        }

        self.devtools.frame_mut().full_layout();
        self.devtools.invalidate();
    }

    fn render_frame(&mut self) {
        self.main.render_if_needed(&mut self.renderer);
        if self.devtools_visible {
            self.devtools.render_if_needed(&mut self.renderer);
            self.overlay.render_if_needed(&mut self.renderer);
        }

        let mut regions = Vec::new();
        if let Some(main_buffer) = self.main.buffer() {
            regions.push(CompositeRegion {
                buffer: main_buffer,
                dest_x: 0.0,
                scroll_y: self.scroll_y,
            });
        }
        if self.devtools_visible {
            if let Some(devtools_buffer) = self.devtools.buffer() {
                regions.push(CompositeRegion {
                    buffer: devtools_buffer,
                    dest_x: self.main.frame().viewport.width,
                    scroll_y: 0.0,
                });
            }
            if let Some(overlay_buffer) = self.overlay.buffer() {
                regions.push(CompositeRegion {
                    buffer: overlay_buffer,
                    dest_x: 0.0,
                    scroll_y: self.scroll_y,
                });
            }
        }

        self.renderer.composite(&regions);
    }

    fn present(&mut self) {
        let Some(surface) = &mut self.surface else { return };
        let Some(window) = &self.window else { return };

        let size = window.inner_size();
        let width = size.width;
        let height = size.height;

        if width == 0 || height == 0 {
            return;
        }

        surface
            .resize(
                NonZeroU32::new(width).unwrap(),
                NonZeroU32::new(height).unwrap(),
            )
            .expect("Failed to resize surface");

        let mut buffer = surface.buffer_mut().expect("Failed to get buffer");
        let display_buffer = self.renderer.get_display_buffer();

        buffer.copy_from_slice(display_buffer);
        buffer.present().expect("Failed to present buffer");
    }

    fn handle_resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }

        self.renderer.resize(new_size.width, new_size.height, self.scale_factor);

        let logical_width = new_size.width as f32 / self.scale_factor;
        let logical_height = new_size.height as f32 / self.scale_factor;
        let devtools_width = self.devtools_width();
        self.main.set_viewport(logical_width - devtools_width, logical_height);
        self.devtools.set_viewport(devtools_width, logical_height);
        self.overlay.set_viewport(logical_width - devtools_width, logical_height);
    }

    fn handle_scale_factor_changed(&mut self, new_scale_factor: f64) {
        let new_scale = new_scale_factor as f32;
        if (new_scale - self.scale_factor).abs() > 0.001 {
            self.scale_factor = new_scale;
            self.renderer.clear_caches();
            self.main.invalidate();
            self.devtools.invalidate();
            self.overlay.invalidate();
        }
    }
}

impl ApplicationHandler for BrowserApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attrs = Window::default_attributes()
            .with_title("Graviton")
            .with_inner_size(LogicalSize::new(1366, 768));

        let window = Rc::new(event_loop.create_window(window_attrs).unwrap());
        let physical_size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;

        self.scale_factor = scale_factor;

        let logical_width = physical_size.width as f32 / scale_factor;
        let logical_height = physical_size.height as f32 / scale_factor;

        // Create surface
        let context = softbuffer::Context::new(window.clone()).unwrap();
        let surface = Surface::new(&context, window.clone()).unwrap();

        // Initialize renderer
        self.renderer.resize(physical_size.width, physical_size.height, scale_factor);

        // Set up viewport
        let devtools_width = self.devtools_width();
        self.main.set_viewport(logical_width - devtools_width, logical_height);
        self.devtools.set_viewport(devtools_width, logical_height);
        self.overlay.set_viewport(logical_width - devtools_width, logical_height);

        self.window = Some(window);
        self.surface = Some(surface);

        // Load URL only once
        if !self.initialized {
            self.initialized = true;
            self.load_url();
            if self.devtools_visible {
                self.load_devtools();
                self.update_devtools_content();
                self.rebuild_overlay();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::Resized(new_size) => {
                let logical_width = new_size.width as f32 / self.scale_factor;
                let logical_height = new_size.height as f32 / self.scale_factor;
                let devtools_width = self.devtools_width();
                self.resize_debouncer.push((logical_width - devtools_width, logical_height));
                self.handle_resize(new_size);
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.handle_scale_factor_changed(scale_factor);
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            WindowEvent::KeyboardInput {
                event: KeyEvent {
                    logical_key,
                    state,
                    ..
                },
                ..
            } => {
                let pressed = state == ElementState::Pressed;

                match logical_key {
                    Key::Named(NamedKey::Escape) => {
                        event_loop.exit();
                    }
                    Key::Named(NamedKey::F5) if pressed => {
                        self.load_url();
                        self.hover_info = None;
                        if self.devtools_visible {
                            self.update_devtools_content();
                            self.rebuild_overlay();
                        }
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        self.pressed_up = pressed;
                        if pressed {
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        }
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        self.pressed_down = pressed;
                        if pressed {
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        }
                    }
                    Key::Character(ref c) if c == "i" && pressed => {
                        self.devtools_visible = !self.devtools_visible;
                        let devtools_width = self.devtools_width();
                        let size_opt = self.window.as_ref().map(|w| w.inner_size());
                        if let Some(size) = size_opt {
                            let logical_width = size.width as f32 / self.scale_factor;
                            let logical_height = size.height as f32 / self.scale_factor;
                            self.main.set_viewport(logical_width - devtools_width, logical_height);
                            self.devtools.set_viewport(devtools_width, logical_height);
                            self.overlay.set_viewport(logical_width - devtools_width, logical_height);
                            self.do_reflow();
                            if self.devtools_visible {
                                self.load_devtools();
                            }
                            self.rebuild_overlay();
                        }
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    }
                    _ => {}
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let scroll_amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 1.0,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };

                self.scroll_y -= scroll_amount;
                self.scroll_y = self.scroll_y.max(0.0);
                let max_scroll = (self.main.frame().page_height - self.main.frame().viewport.height).max(0.0);
                self.scroll_y = self.scroll_y.min(max_scroll);

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_x = position.x as f32;
                self.mouse_y = position.y as f32;

                // Hit test to find hovered element (only when devtools visible)
                if self.devtools_visible {
                    // Convert screen coordinates to page coordinates
                    let page_x = self.mouse_x / self.scale_factor;
                    let page_y = self.mouse_y / self.scale_factor + self.scroll_y;

                    let new_hover = self.main.frame().hit_test(page_x, page_y);

                    // Only redraw if hover changed
                    if new_hover != self.hover_info {
                        self.hover_info = new_hover;
                        self.update_devtools_content();
                        self.rebuild_overlay();
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                // Handle continuous scroll
                if self.pressed_up || self.pressed_down {
                    if self.pressed_up {
                        self.scroll_y -= 4.0;
                    }
                    if self.pressed_down {
                        self.scroll_y += 4.0;
                    }
                    self.scroll_y = self.scroll_y.max(0.0);
                    let max_scroll = (self.main.frame().page_height - self.main.frame().viewport.height).max(0.0);
                    self.scroll_y = self.scroll_y.min(max_scroll);
                }

                // Handle debounced resize
                if let Some((width, height)) = self.resize_debouncer.poll() {
                    self.main.set_viewport(width, height);
                    self.devtools.set_viewport(self.devtools_width(), height);
                    self.overlay.set_viewport(width, height);
                    self.do_reflow();
                    if self.devtools_visible {
                        self.load_devtools();
                    }
                    self.rebuild_overlay();
                }

                // Render and present
                self.render_frame();
                self.present();
            }

            _ => {}
        }

        // Request redraw for continuous scroll
        if self.pressed_up || self.pressed_down {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Event-driven - don't continuously redraw
    }
}

pub fn create_browser_window(url: String) {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = BrowserApp::new(url);

    event_loop.run_app(&mut app).unwrap();
}
