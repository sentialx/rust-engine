mod render_frame_state;

pub(crate) use render_frame_state::RenderFrameState;
use crate::layout::Rect;
use crate::renderer::{CompositeRegion, Renderer, SkiaRenderer};
use crate::ui::devtools_manager::DevtoolsManager;
use crate::utils::Debouncer;

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

    // Document layout
    main: RenderFrameState,

    // Devtools (panel, overlay, selection state)
    devtools: DevtoolsManager,

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
    scale_factor: f32,
    resize_debouncer: Debouncer<(f32, f32)>,
    initialized: bool,
}

impl BrowserApp {
    fn new(url: String) -> Self {
        Self {
            url,
            window: None,
            surface: None,
            main: RenderFrameState::new(Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 }),
            devtools: DevtoolsManager::new(),
            renderer: SkiaRenderer::new(),
            scroll_y: 0.0,
            pressed_up: false,
            pressed_down: false,
            mouse_x: 0.0,
            mouse_y: 0.0,
            scale_factor: 1.0,
            resize_debouncer: Debouncer::new(Duration::from_millis(1)),
            initialized: false,
        }
    }

    fn load_url(&mut self) {
        self.main.frame_mut().load_url(&self.url);
        self.main.invalidate();
    }

    fn update_devtools(&mut self) {
        let viewport = self.main.frame().viewport.clone();
        let page_height = self.main.frame().page_height.max(viewport.height);
        self.devtools.update(&self.main.frame().dom_tree, viewport, page_height);
    }

    /// Dispatch a click to the appropriate frame using generic hit testing
    fn dispatch_click(&mut self, logical_x: f32, logical_y: f32) -> bool {
        // Build frame regions: (x_offset, uses_scroll, is_main_frame)
        let main_viewport = self.main.frame().viewport.clone();

        struct FrameRegion {
            x_offset: f32,
            width: f32,
            height: f32,
            uses_scroll: bool,
            is_main: bool,
        }

        let mut regions = vec![
            FrameRegion {
                x_offset: 0.0,
                width: main_viewport.width,
                height: main_viewport.height,
                uses_scroll: true,
                is_main: true,
            },
        ];

        if self.devtools.is_visible() {
            let panel_viewport = self.devtools.panel_viewport();
            regions.push(FrameRegion {
                x_offset: main_viewport.width,
                width: panel_viewport.width,
                height: panel_viewport.height,
                uses_scroll: false,
                is_main: false,
            });
        }

        // Find which frame was clicked
        for region in &regions {
            if logical_x >= region.x_offset
                && logical_x < region.x_offset + region.width
                && logical_y >= 0.0
                && logical_y < region.height
            {
                let local_x = logical_x - region.x_offset;
                let local_y = if region.uses_scroll {
                    logical_y + self.scroll_y
                } else {
                    logical_y
                };

                if region.is_main {
                    // Special handling for main frame when devtools visible (element pinning)
                    if self.devtools.is_visible() {
                        let clicked_element = self.main.frame().hit_test(local_x, local_y);
                        self.devtools.handle_element_click(clicked_element);
                        self.update_devtools();
                        return true;
                    } else {
                        let handled = self.main.frame_mut().dispatch_click_at(local_x, local_y);
                        if handled {
                            self.main.invalidate();
                        }
                        return handled;
                    }
                } else {
                    // Devtools panel - dispatch click to frame
                    let handled = self.devtools.panel_frame_mut().frame_mut().dispatch_click_at(local_x, local_y);
                    if handled {
                        self.devtools.panel_frame_mut().invalidate();
                    }
                    return handled;
                }
            }
        }

        false
    }

    fn render_frame(&mut self) {
        self.main.render_if_needed(&mut self.renderer);
        self.devtools.render_if_needed(&mut self.renderer);

        let mut regions = Vec::new();
        if let Some(main_buffer) = self.main.buffer() {
            regions.push(CompositeRegion {
                buffer: main_buffer,
                dest_x: 0.0,
                scroll_y: self.scroll_y,
            });
        }

        self.devtools.add_composite_regions(
            &mut regions,
            self.main.frame().viewport.width,
            self.scroll_y,
        );

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
        let devtools_width = self.devtools.reserved_width();
        let main_width = logical_width - devtools_width;

        self.main.set_viewport(main_width, logical_height);
        self.devtools.set_viewport(devtools_width, logical_height, main_width);
    }

    fn handle_scale_factor_changed(&mut self, new_scale_factor: f64) {
        let new_scale = new_scale_factor as f32;
        if (new_scale - self.scale_factor).abs() > 0.001 {
            self.scale_factor = new_scale;
            self.renderer.clear_caches();
            self.main.invalidate();
            self.devtools.invalidate();
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
        let devtools_width = self.devtools.reserved_width();
        let main_width = logical_width - devtools_width;
        self.main.set_viewport(main_width, logical_height);
        self.devtools.set_viewport(devtools_width, logical_height, main_width);

        self.window = Some(window);
        self.surface = Some(surface);

        // Load URL only once
        if !self.initialized {
            self.initialized = true;
            self.load_url();
            if self.devtools.is_visible() {
                self.devtools.load();
                self.update_devtools();
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
                let devtools_width = self.devtools.reserved_width();
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
                        self.devtools.clear_selection();
                        if self.devtools.is_visible() {
                            self.update_devtools();
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
                        self.devtools.toggle_visible();
                        let size_opt = self.window.as_ref().map(|w| w.inner_size());
                        if let Some(size) = size_opt {
                            let logical_width = size.width as f32 / self.scale_factor;
                            let logical_height = size.height as f32 / self.scale_factor;
                            let devtools_width = self.devtools.reserved_width();
                            let main_width = logical_width - devtools_width;
                            self.main.set_viewport(main_width, logical_height);
                            self.devtools.set_viewport(devtools_width, logical_height, main_width);
                            if self.devtools.is_visible() {
                                self.devtools.load();
                                self.update_devtools();
                            }
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

                    let handled = self.dispatch_click(logical_x, logical_y);

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
                let page_x = self.mouse_x / self.scale_factor;
                let page_y = self.mouse_y / self.scale_factor + self.scroll_y;

                let new_hover = self.main.frame().hit_test(page_x, page_y);

                // Check if hover changed by comparing with devtools tracked hover
                let current_hover = if self.devtools.is_pinned() {
                    None // Don't compare when pinned
                } else {
                    self.devtools.selected_element()
                };

                let hover_changed = match (&current_hover, &new_hover) {
                    (Some(current), Some(next)) => !Rc::ptr_eq(current, next),
                    (None, None) => false,
                    _ => true,
                };

                if hover_changed || self.devtools.is_pinned() {
                    // Update CSS :hover state (this triggers restyle)
                    self.main.frame_mut().set_hover(new_hover.clone());
                    self.main.invalidate();

                    // Update devtools if visible and not pinned
                    if self.devtools.is_visible() && !self.devtools.is_pinned() {
                        self.devtools.set_hover(new_hover);
                        self.update_devtools();
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
                    self.devtools.set_viewport(self.devtools.reserved_width(), height, width);
                    let viewport = self.main.frame().viewport.clone();
                    let page_height = self.main.frame().page_height.max(viewport.height);
                    self.devtools.rebuild_overlay(viewport, page_height);
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
