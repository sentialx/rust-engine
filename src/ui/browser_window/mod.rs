mod render_frame_state;

pub(crate) use render_frame_state::RenderFrameState;
use crate::events::{EventRouter, FrameRegion, InputEventKind};
use crate::frame::RenderDelegate;
use crate::layout::Rect;
use crate::renderer::{CompositeFrame, HybridRenderer, WgpuCompositor};
use crate::ui::devtools_manager::DevtoolsManager;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

/// Wrapper to allow Rc<Arc<Window>> for the delegate
struct ArcWindow(Arc<Window>);

impl std::ops::Deref for ArcWindow {
    type Target = Window;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Delegate that forwards render requests to the window
struct WindowRenderDelegate {
    window: Rc<ArcWindow>,
}

impl RenderDelegate for WindowRenderDelegate {
    fn request_redraw(&self) {
        self.window.request_redraw();
    }
}

struct BrowserApp {
    url: String,
    window: Option<Arc<Window>>,
    compositor: Option<WgpuCompositor>,
    renderer: Option<HybridRenderer>,
    render_delegate: Option<Rc<WindowRenderDelegate>>,

    // Document layout
    main: RenderFrameState,

    // Devtools (panel, overlay, selection state)
    devtools: DevtoolsManager,

    // Event routing
    router: EventRouter,

    // Input state
    pressed_up: bool,
    pressed_down: bool,
    mouse_x: f32,
    mouse_y: f32,

    // UI state
    scale_factor: f32,
    initialized: bool,
}

impl BrowserApp {
    fn new(url: String) -> Self {
        let empty_rect = Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };
        let mut main = RenderFrameState::new(empty_rect);

        // Create devtools and attach to main frame for inspection
        let devtools = DevtoolsManager::create_for_frame(&mut main);

        Self {
            url,
            window: None,
            compositor: None,
            renderer: None,
            render_delegate: None,
            main,
            devtools,
            router: EventRouter::new(),
            pressed_up: false,
            pressed_down: false,
            mouse_x: 0.0,
            mouse_y: 0.0,
            scale_factor: 1.0,
            initialized: false,
        }
    }

    fn load_url(&mut self) {
        self.main.frame_mut().load_url(&self.url);
        // load_url calls full_layout which sets render_needed
        self.devtools.clear_selection();
    }

    fn update_devtools(&mut self) {
        let viewport = self.main.frame().viewport.clone();
        let scroll_y = self.main.scroll_y();
        self.devtools.update(&self.main.frame().dom_tree, viewport, scroll_y);
    }

    fn setup_event_routing(&mut self) {
        self.router.clear();

        let main_viewport = self.main.frame().viewport.clone();

        // Main frame region
        self.router.add_region(FrameRegion::new(
            Rect {
                x: 0.0,
                y: 0.0,
                width: main_viewport.width,
                height: main_viewport.height,
            },
            self.main.sink_rc(),
        ));

        // Devtools panel region (if visible)
        if self.devtools.is_visible() {
            let panel_viewport = self.devtools.panel_viewport().clone();
            self.router.add_region(FrameRegion::new(
                Rect {
                    x: main_viewport.width,
                    y: 0.0,
                    width: panel_viewport.width,
                    height: panel_viewport.height,
                },
                self.devtools.panel_sink_rc(),
            ));
        }
    }

    /// Dispatch an input event through the router
    fn dispatch_event(&mut self, kind: InputEventKind, logical_x: f32, logical_y: f32) -> bool {
        let result = self.router.dispatch(kind, logical_x, logical_y);

        // Just update devtools if needed for non-scroll events
        if result.handled && !matches!(kind, InputEventKind::Scroll { .. }) {
            if self.devtools.is_visible() {
                self.update_devtools();
            }
        }

        result.handled
    }

    fn render_and_present(&mut self) {
        let Some(renderer) = &mut self.renderer else { return };

        let scale = self.scale_factor;
        renderer.set_scale_factor(scale);

        // Frame IDs for caching
        const MAIN_FRAME_ID: usize = 0;
        const PANEL_FRAME_ID: usize = 1;
        const OVERLAY_FRAME_ID: usize = 2;

        // Update styles/layout for all frames
        self.main.frame_mut().update_styles_if_needed();
        let devtools_visible = self.devtools.is_visible();
        if devtools_visible {
            self.devtools.panel_mut().render_state_mut().frame_mut().update_styles_if_needed();
            self.devtools.overlay_mut().render_state_mut().frame_mut().update_styles_if_needed();
        }

        // Render frames to textures
        let t0 = Instant::now();
        renderer.render(&self.main.frame(), MAIN_FRAME_ID);
        println!("  main render: {:?}", t0.elapsed());

        if devtools_visible {
            let t1 = Instant::now();
            renderer.render(&self.devtools.panel_mut().render_state_mut().frame(), PANEL_FRAME_ID);
            println!("  panel render: {:?}", t1.elapsed());

            let t2 = Instant::now();
            renderer.render(&self.devtools.overlay_mut().render_state_mut().frame(), OVERLAY_FRAME_ID);
            println!("  overlay render: {:?}", t2.elapsed());
        }

        // Collect frame data for compositing
        let main_scroll_y = self.main.scroll_y();
        let main_viewport_width = self.main.frame().viewport.width;
        let panel_scroll_y = self.devtools.panel_scroll_y();

        // Build composite frames
        let mut frames: Vec<CompositeFrame> = Vec::new();

        if let Some(tex) = renderer.get_texture(MAIN_FRAME_ID) {
            frames.push(CompositeFrame {
                texture: tex,
                dest_x: 0.0,
                scroll_y: main_scroll_y,
            });
        }

        if devtools_visible {
            if let Some(tex) = renderer.get_texture(PANEL_FRAME_ID) {
                frames.push(CompositeFrame {
                    texture: tex,
                    dest_x: main_viewport_width,
                    scroll_y: panel_scroll_y,
                });
            }
            if let Some(tex) = renderer.get_texture(OVERLAY_FRAME_ID) {
                frames.push(CompositeFrame {
                    texture: tex,
                    dest_x: 0.0,
                    scroll_y: 0.0, // Overlay is viewport-relative
                });
            }
        }

        // Notify winit we're about to present, then composite to screen
        if let Some(window) = &self.window {
            window.pre_present_notify();
        }
        if let Some(compositor) = &mut self.compositor {
            compositor.compose_frames(&frames, scale);
        }
    }

    fn handle_resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }

        // Resize compositor immediately for correct surface dimensions
        if let Some(compositor) = &mut self.compositor {
            compositor.resize(new_size.width, new_size.height);
        }

        // Clear cached frame textures to prevent stale content
        if let Some(renderer) = &mut self.renderer {
            renderer.clear_caches();
        }

        // Update viewport and relayout immediately
        let logical_width = new_size.width as f32 / self.scale_factor;
        let logical_height = new_size.height as f32 / self.scale_factor;
        let devtools_width = self.devtools.reserved_width();
        let main_width = logical_width - devtools_width;

        self.main.set_viewport(main_width, logical_height);
        self.devtools.set_viewport(devtools_width, logical_height, main_width);
        self.setup_event_routing();

        // Render and present immediately to avoid showing stretched old frame
        self.render_and_present();
    }

    fn handle_scale_factor_changed(&mut self, new_scale_factor: f64) {
        let new_scale = new_scale_factor as f32;
        if (new_scale - self.scale_factor).abs() > 0.001 {
            self.scale_factor = new_scale;
            // HybridRenderer handles scale factor changes internally via set_scale_factor
            if let Some(ref window) = self.window {
                window.request_redraw();
            }
        }
    }
}

impl ApplicationHandler for BrowserApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attrs = Window::default_attributes()
            .with_title("Graviton")
            .with_inner_size(LogicalSize::new(1366, 768));

        let window = Arc::new(event_loop.create_window(window_attrs).unwrap());
        let physical_size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;

        self.scale_factor = scale_factor;

        let logical_width = physical_size.width as f32 / scale_factor;
        let logical_height = physical_size.height as f32 / scale_factor;

        // Create GPU compositor
        let compositor = WgpuCompositor::new(window.clone());

        // Create hybrid renderer using compositor's device/queue
        let renderer = HybridRenderer::new(
            compositor.device(),
            compositor.queue(),
        );
        self.renderer = Some(renderer);

        // Set up viewport
        let devtools_width = self.devtools.reserved_width();
        let main_width = logical_width - devtools_width;
        self.main.set_viewport(main_width, logical_height);
        self.devtools.set_viewport(devtools_width, logical_height, main_width);

        self.window = Some(window.clone());
        self.compositor = Some(compositor);

        // Set up render delegate so frames can request redraws
        let window_rc = Rc::new(ArcWindow(window));
        let delegate = Rc::new(WindowRenderDelegate { window: window_rc });
        self.render_delegate = Some(delegate.clone());
        self.main.set_render_delegate(Rc::downgrade(&delegate) as _);
        self.devtools.set_render_delegate(Rc::downgrade(&delegate) as _);

        // Set up event routing
        self.setup_event_routing();

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
                self.handle_resize(new_size);
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.handle_scale_factor_changed(scale_factor);
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
                        // Redraw triggered automatically via DOM changes
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
                            self.setup_event_routing();
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
                let scroll_delta = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 20.0,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };

                let logical_x = self.mouse_x / self.scale_factor;
                let logical_y = self.mouse_y / self.scale_factor;

                if self.dispatch_event(InputEventKind::Scroll { delta: scroll_delta }, logical_x, logical_y) {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                if state == ElementState::Pressed && button == MouseButton::Left {
                    let logical_x = self.mouse_x / self.scale_factor;
                    let logical_y = self.mouse_y / self.scale_factor;

                    if self.dispatch_event(InputEventKind::Click, logical_x, logical_y) {
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_x = position.x as f32;
                self.mouse_y = position.y as f32;

                let logical_x = self.mouse_x / self.scale_factor;
                let logical_y = self.mouse_y / self.scale_factor;

                if self.dispatch_event(InputEventKind::MouseMove, logical_x, logical_y) {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                // Handle continuous keyboard scroll
                if self.pressed_up || self.pressed_down {
                    let delta = if self.pressed_up { 4.0 } else { -4.0 };
                    // Dispatch to main frame (use center of viewport)
                    let main_viewport = self.main.frame().viewport.clone();
                    self.dispatch_event(
                        InputEventKind::Scroll { delta },
                        main_viewport.width / 2.0,
                        main_viewport.height / 2.0,
                    );
                }

                // Render and present
                self.render_and_present();
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
