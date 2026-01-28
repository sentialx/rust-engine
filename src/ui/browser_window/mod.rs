mod render_frame_state;

pub(crate) use render_frame_state::RenderFrameState;
use crate::events::{EventRouter, FrameRegion, InputEventKind};
use crate::frame::RenderDelegate;
use crate::layout::{Rect, Size};
use crate::renderer::{HybridRenderer, WgpuCompositor};
use crate::ui::devtools_panel::DevtoolsPanel;
use crate::ui::web_contents::WebContents;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

struct ArcWindow(Arc<Window>);

impl std::ops::Deref for ArcWindow {
    type Target = Window;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

struct WindowRenderDelegate {
    window: Rc<ArcWindow>,
}

impl RenderDelegate for WindowRenderDelegate {
    fn request_redraw(&self) {
        self.window.request_redraw();
    }
}

const PANEL_WIDTH: f32 = 300.0;
const MAIN_FRAME_ID: usize = 0;
const MAIN_OVERLAY_FRAME_ID: usize = 1;
const PANEL_FRAME_ID: usize = 2;
const PANEL_OVERLAY_FRAME_ID: usize = 3;

struct BrowserApp {
    url: String,
    window: Option<Arc<Window>>,
    compositor: Option<WgpuCompositor>,
    renderer: Option<HybridRenderer>,
    render_delegate: Option<Rc<dyn RenderDelegate>>,

    web_contents: WebContents,
    devtools_panel: Option<DevtoolsPanel>,
    router: EventRouter,

    pressed_up: bool,
    pressed_down: bool,
    mouse_x: f32,
    mouse_y: f32,
    scale_factor: f32,
    initialized: bool,
    panel_visible: bool,
}

impl BrowserApp {
    fn new(url: String) -> Self {
        let empty_size = Size { width: 0.0, height: 0.0 };
        let web_contents = WebContents::new(empty_size, MAIN_FRAME_ID, MAIN_OVERLAY_FRAME_ID);

        Self {
            url,
            window: None,
            compositor: None,
            renderer: None,
            render_delegate: None,
            web_contents,
            devtools_panel: None,
            router: EventRouter::new(),
            pressed_up: false,
            pressed_down: false,
            mouse_x: 0.0,
            mouse_y: 0.0,
            scale_factor: 1.0,
            initialized: false,
            panel_visible: false,
        }
    }

    fn load_url(&mut self) {
        self.web_contents.frame_mut().load_url(&self.url);
        self.web_contents.inspector().borrow_mut().clear_selection();
    }

    fn reserved_width(&self) -> f32 {
        if self.panel_visible { PANEL_WIDTH } else { 0.0 }
    }

    fn update_panel_content(&mut self) {
        if let Some(ref mut panel) = self.devtools_panel {
            let inspector = self.web_contents.inspector().borrow();
            let selected = inspector.selected_element();
            let is_pinned = inspector.is_pinned();
            panel.update(selected.as_ref(), &self.web_contents.frame().dom_tree, is_pinned);
        }
    }

    fn setup_event_routing(&mut self) {
        self.router.clear();
        let main_viewport = self.web_contents.frame().viewport.clone();

        self.router.add_region(FrameRegion::new(
            Rect { x: 0.0, y: 0.0, width: main_viewport.width, height: main_viewport.height },
            self.web_contents.sink_rc(),
        ));

        if let Some(ref panel) = self.devtools_panel {
            let panel_viewport = panel.viewport().clone();
            self.router.add_region(FrameRegion::new(
                Rect { x: main_viewport.width, y: 0.0, width: panel_viewport.width, height: panel_viewport.height },
                panel.sink_rc(),
            ));
        }
    }

    fn dispatch_event(&mut self, kind: InputEventKind, logical_x: f32, logical_y: f32) -> bool {
        self.router.dispatch(kind, logical_x, logical_y).handled
    }

    fn render_and_present(&mut self) {
        let scale = self.scale_factor;

        // Phase 1: Render all content
        {
            let Some(renderer) = &mut self.renderer else { return };
            renderer.set_scale_factor(scale);
            self.web_contents.render(renderer);
            if let Some(ref mut panel) = self.devtools_panel {
                panel.render(renderer);
            }
        }

        // Phase 2: Update panel content if main selection changed
        if self.web_contents.take_selection_dirty() {
            self.update_panel_content();
        }

        // Phase 3: Composite
        let Some(renderer) = &mut self.renderer else { return };

        // Composite (order: main, panel on side, main overlay on top)
        let mut frames = self.web_contents.get_composite_frames(renderer, 0.0);
        if let Some(ref panel) = self.devtools_panel {
            // Insert panel frames before main overlay (which is last in web_contents frames)
            let main_overlay = frames.pop();
            frames.extend(panel.get_composite_frames(renderer, self.web_contents.viewport_width()));
            if let Some(f) = main_overlay {
                frames.push(f);
            }
        }

        if let Some(window) = &self.window {
            window.pre_present_notify();
        }
        if let Some(compositor) = &mut self.compositor {
            compositor.compose_frames(&frames, scale);
        }
    }

    fn handle_resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 { return }

        if let Some(compositor) = &mut self.compositor {
            compositor.resize(new_size.width, new_size.height);
        }
        if let Some(renderer) = &mut self.renderer {
            renderer.clear_caches();
        }

        let logical_width = new_size.width as f32 / self.scale_factor;
        let logical_height = new_size.height as f32 / self.scale_factor;
        let devtools_width = self.reserved_width();
        let main_width = logical_width - devtools_width;

        self.web_contents.set_viewport(main_width, logical_height);

        if let Some(ref mut panel) = self.devtools_panel {
            panel.set_viewport(devtools_width, logical_height);
        }

        self.setup_event_routing();
        self.render_and_present();
    }

    fn handle_scale_factor_changed(&mut self, new_scale_factor: f64) {
        let new_scale = new_scale_factor as f32;
        if (new_scale - self.scale_factor).abs() > 0.001 {
            self.scale_factor = new_scale;
            if let Some(ref window) = self.window {
                window.request_redraw();
            }
        }
    }

    fn toggle_panel(&mut self) {
        self.panel_visible = !self.panel_visible;
        self.web_contents.selection().borrow_mut().visible = self.panel_visible;

        if let Some(window) = &self.window {
            let size = window.inner_size();
            let logical_width = size.width as f32 / self.scale_factor;
            let logical_height = size.height as f32 / self.scale_factor;
            let devtools_width = self.reserved_width();
            let main_width = logical_width - devtools_width;

            self.web_contents.set_viewport(main_width, logical_height);

            if self.panel_visible {
                let panel_size = Size { width: devtools_width, height: logical_height };
                let mut panel = DevtoolsPanel::new(panel_size, PANEL_FRAME_ID, PANEL_OVERLAY_FRAME_ID);

                if let Some(ref delegate) = self.render_delegate {
                    panel.set_render_delegate(Rc::downgrade(delegate));
                }

                panel.load();

                let inspector = self.web_contents.inspector().borrow();
                let selected = inspector.selected_element();
                let is_pinned = inspector.is_pinned();
                panel.update(selected.as_ref(), &self.web_contents.frame().dom_tree, is_pinned);

                self.devtools_panel = Some(panel);
            } else {
                self.devtools_panel = None;
            }

            self.setup_event_routing();
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

        let compositor = WgpuCompositor::new(window.clone());
        let renderer = HybridRenderer::new(compositor.device(), compositor.queue());
        self.renderer = Some(renderer);

        let devtools_width = self.reserved_width();
        let main_width = logical_width - devtools_width;
        self.web_contents.set_viewport(main_width, logical_height);

        self.window = Some(window.clone());
        self.compositor = Some(compositor);

        // Set up render delegate - inspector wraps it for auto-updates
        let window_rc = Rc::new(ArcWindow(window));
        let delegate: Rc<dyn RenderDelegate> = Rc::new(WindowRenderDelegate { window: window_rc });
        self.render_delegate = Some(delegate.clone());

        self.web_contents.set_render_delegate(Rc::downgrade(&delegate) as _);

        self.setup_event_routing();

        if !self.initialized {
            self.initialized = true;
            self.load_url();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => self.handle_resize(new_size),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => self.handle_scale_factor_changed(scale_factor),

            WindowEvent::KeyboardInput { event: KeyEvent { logical_key, state, .. }, .. } => {
                let pressed = state == ElementState::Pressed;
                match logical_key {
                    Key::Named(NamedKey::Escape) => event_loop.exit(),
                    Key::Named(NamedKey::F5) if pressed => {
                        self.load_url();
                        self.update_panel_content();
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        self.pressed_up = pressed;
                        if pressed { if let Some(w) = &self.window { w.request_redraw(); } }
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        self.pressed_down = pressed;
                        if pressed { if let Some(w) = &self.window { w.request_redraw(); } }
                    }
                    Key::Character(ref c) if c == "i" && pressed => {
                        self.toggle_panel();
                        if let Some(w) = &self.window { w.request_redraw(); }
                    }
                    _ => {}
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let scroll_delta = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 20.0,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };
                let (lx, ly) = (self.mouse_x / self.scale_factor, self.mouse_y / self.scale_factor);
                if self.dispatch_event(InputEventKind::Scroll { delta: scroll_delta }, lx, ly) {
                    if let Some(w) = &self.window { w.request_redraw(); }
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                if state == ElementState::Pressed && button == MouseButton::Left {
                    let (lx, ly) = (self.mouse_x / self.scale_factor, self.mouse_y / self.scale_factor);
                    if self.dispatch_event(InputEventKind::Click, lx, ly) {
                        if let Some(w) = &self.window { w.request_redraw(); }
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_x = position.x as f32;
                self.mouse_y = position.y as f32;
                let (lx, ly) = (self.mouse_x / self.scale_factor, self.mouse_y / self.scale_factor);
                if self.dispatch_event(InputEventKind::MouseMove, lx, ly) {
                    if let Some(w) = &self.window { w.request_redraw(); }
                }
            }

            WindowEvent::RedrawRequested => {
                if self.pressed_up || self.pressed_down {
                    let delta = if self.pressed_up { 4.0 } else { -4.0 };
                    let vp = self.web_contents.frame().viewport.clone();
                    self.dispatch_event(InputEventKind::Scroll { delta }, vp.width / 2.0, vp.height / 2.0);
                }
                self.render_and_present();
            }

            _ => {}
        }

        if self.pressed_up || self.pressed_down {
            if let Some(w) = &self.window { w.request_redraw(); }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {}
}

pub fn create_browser_window(url: String) {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = BrowserApp::new(url);
    event_loop.run_app(&mut app).unwrap();
}
