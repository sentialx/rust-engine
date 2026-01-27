mod render_frame_state;

pub(crate) use render_frame_state::RenderFrameState;
use crate::html::DomElement;
use crate::layout::Rect;
use crate::renderer::{CompositeRegion, Renderer, SkiaRenderer};
use crate::ui::devtools::DevtoolsOverlay;
use crate::utils::Debouncer;

use std::cell::RefCell;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Duration;

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use softbuffer::Surface;

struct BrowserApp {
    url: String,
    window: Option<Rc<Window>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,

    // Document layout (owns text measurer and font loading)
    main: RenderFrameState,
    devtools: RenderFrameState,
    overlay: DevtoolsOverlay,

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
    hover_info: Option<Rc<RefCell<DomElement>>>,
}

impl BrowserApp {
    fn new(url: String) -> Self {
        Self {
            url,
            window: None,
            surface: None,
            main: RenderFrameState::new(Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 }),
            devtools: RenderFrameState::new(Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 }),
            overlay: DevtoolsOverlay::new(Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 }),
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

    fn load_devtools(&mut self) {
        self.devtools.frame_mut().load_url("devtools.html");
        self.devtools.invalidate();
    }

    fn rebuild_overlay(&mut self) {
        let viewport = self.main.frame().viewport.clone();
        let page_height = self.main.frame().page_height.max(viewport.height);
        self.overlay.rebuild(self.hover_info.as_ref(), viewport, page_height);
    }

    fn update_devtools_content(&mut self) {
        let (tag_text, dimension_text) = if let Some(node) = &self.hover_info {
            let element = node.borrow();
            let tag = element.tag_name.to_lowercase();
            let dims = element
                .computed_flow
                .as_ref()
                .map(|flow| format!("{:.0}x{:.0}", flow.width, flow.height))
                .unwrap_or_else(|| "--".to_string());
            (format!("<{}>", tag), dims)
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

            WindowEvent::MouseInput { state, button, .. } => {
                if state == ElementState::Pressed && button == MouseButton::Left {
                    let logical_x = self.mouse_x / self.scale_factor;
                    let logical_y = self.mouse_y / self.scale_factor;
                    let mut handled = false;

                    let main_width = self.main.frame().viewport.width;
                    let main_height = self.main.frame().viewport.height;
                    let devtools_width = self.devtools.frame().viewport.width;
                    let devtools_height = self.devtools.frame().viewport.height;

                    let mut frames: Vec<(Rect, &mut RenderFrameState, bool)> = Vec::new();
                    let main_rect = Rect {
                        x: 0.0,
                        y: 0.0,
                        width: main_width,
                        height: main_height,
                    };
                    frames.push((main_rect, &mut self.main, true));

                    if self.devtools_visible {
                        let devtools_rect = Rect {
                            x: main_width,
                            y: 0.0,
                            width: devtools_width,
                            height: devtools_height,
                        };
                        frames.push((devtools_rect, &mut self.devtools, false));
                    }

                    for (rect, frame, uses_scroll) in frames {
                        if logical_x >= rect.x
                            && logical_x < rect.x + rect.width
                            && logical_y >= rect.y
                            && logical_y < rect.y + rect.height
                        {
                            let local_x = logical_x - rect.x;
                            let local_y = logical_y - rect.y;
                            let click_y = if uses_scroll { local_y + self.scroll_y } else { local_y };
                            handled = frame.frame_mut().dispatch_click_at(local_x, click_y);
                            if handled {
                                frame.invalidate();
                            }
                            break;
                        }
                    }

                    if handled {
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_x = position.x as f32;
                self.mouse_y = position.y as f32;

                // Hit test to find hovered element
                // Convert screen coordinates to page coordinates
                let page_x = self.mouse_x / self.scale_factor;
                let page_y = self.mouse_y / self.scale_factor + self.scroll_y;

                let new_hover = self.main.frame().hit_test(page_x, page_y);

                // Check if hover changed
                let hover_changed = match (&self.hover_info, &new_hover) {
                    (Some(current), Some(next)) => !Rc::ptr_eq(current, next),
                    (None, None) => false,
                    _ => true,
                };

                if hover_changed {
                    // Update CSS :hover state (this triggers restyle)
                    self.main.frame_mut().set_hover(new_hover.clone());
                    self.main.invalidate();

                    // Update devtools overlay if visible
                    if self.devtools_visible {
                        self.hover_info = new_hover;
                        self.update_devtools_content();
                        self.rebuild_overlay();
                    } else {
                        self.hover_info = new_hover;
                    }

                    if let Some(window) = &self.window {
                        window.request_redraw();
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
