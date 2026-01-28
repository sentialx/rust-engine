// GPU-accelerated compositor using wgpu

use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

use super::CompositeFrame;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    // Screen position (in pixels)
    dest_x: f32,
    dest_y: f32,
    dest_w: f32,
    dest_h: f32,
    // Texture source (normalized 0-1)
    src_y: f32,
    src_h: f32,
    // Screen size for normalization
    screen_w: f32,
    screen_h: f32,
}

pub struct WgpuCompositor {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    config: wgpu::SurfaceConfiguration,
    // Texture compositing pipeline
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    // Dimensions
    width: u32,
    height: u32,
}

impl WgpuCompositor {
    pub fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        // Configure Metal layer after surface creation to prevent resize glitch
        #[cfg(target_os = "macos")]
        {
            Self::configure_metal_layer_for_window(&window);
        }

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .unwrap();

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let surface_caps = surface.get_capabilities(&adapter);
        // Prefer non-sRGB format so blending happens in sRGB space (matching tiny-skia)
        let surface_format = surface_caps.formats.iter()
            .find(|f| !f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        // Use Fifo (vsync) for synchronized presentation with presentsWithTransaction
        // Use Mailbox if available for faster presentation during resize
        // Falls back to Fifo if Mailbox is not supported
        let present_mode = if surface_caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::Fifo
        };

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compositor Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("compositor.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Simple quad vertices (0,0) to (1,1)
        let vertices: &[Vertex] = &[
            Vertex { position: [0.0, 0.0], tex_coords: [0.0, 0.0] },
            Vertex { position: [1.0, 0.0], tex_coords: [1.0, 0.0] },
            Vertex { position: [1.0, 1.0], tex_coords: [1.0, 1.0] },
            Vertex { position: [0.0, 1.0], tex_coords: [0.0, 1.0] },
        ];
        let indices: &[u16] = &[0, 1, 2, 0, 2, 3];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            window,
            surface,
            device,
            queue,
            config,
            pipeline,
            vertex_buffer,
            index_buffer,
            bind_group_layout,
            sampler,
            width: size.width,
            height: size.height,
        }
    }

    /// Configure Metal layer to prevent content stretching during resize (macOS only)
    /// Sets contentsGravity to bottomLeft so old content stays anchored during resize
    #[cfg(target_os = "macos")]
    fn configure_metal_layer_for_window(window: &Window) {
        use objc::runtime::Object;
        use objc::{class, msg_send, sel, sel_impl};

        unsafe {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            let Ok(handle) = window.window_handle() else { return };
            let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() else { return };

            let ns_view = appkit_handle.ns_view.as_ptr() as *mut Object;
            if ns_view.is_null() { return }

            // Check if view is layer-backed
            let wants_layer: bool = msg_send![ns_view, wantsLayer];
            if !wants_layer {
                return;
            }

            // Get the layer from the view
            let layer: *mut Object = msg_send![ns_view, layer];
            if layer.is_null() {
                return;
            }

            // Set contentsGravity on sublayers to prevent stretching during resize
            // Use bottomLeft since macOS has origin at bottom-left
            let sublayers: *mut Object = msg_send![layer, sublayers];
            if !sublayers.is_null() {
                let count: usize = msg_send![sublayers, count];
                let gravity: *mut Object = msg_send![class!(NSString), stringWithUTF8String: b"bottomLeft\0".as_ptr()];
                for i in 0..count {
                    let sublayer: *mut Object = msg_send![sublayers, objectAtIndex: i];
                    if !sublayer.is_null() {
                        let _: () = msg_send![sublayer, setContentsGravity: gravity];
                    }
                }
            }
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.width = width;
        self.height = height;
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);

        // Reconfigure Metal layer after surface reconfiguration
        #[cfg(target_os = "macos")]
        {
            Self::configure_metal_layer_for_window(&self.window);
        }
    }

    /// Get device for creating HybridRenderer
    pub fn device(&self) -> Arc<wgpu::Device> {
        Arc::clone(&self.device)
    }

    /// Get queue for creating HybridRenderer
    pub fn queue(&self) -> Arc<wgpu::Queue> {
        Arc::clone(&self.queue)
    }

    /// Composite frame textures to the screen
    pub fn compose_frames(&mut self, frames: &[CompositeFrame], scale_factor: f32) {
        let output = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                self.surface.configure(&self.device, &self.config);
                match self.surface.get_current_texture() {
                    Ok(t) => t,
                    Err(_) => return,
                }
            }
            Err(_) => return,
        };

        if output.texture.width() != self.width || output.texture.height() != self.height {
            return;
        }

        // Create bind groups for frame textures
        let mut bind_groups = Vec::new();
        for (i, frame) in frames.iter().enumerate() {
            let tex = frame.texture;
            let dest_x = frame.dest_x * scale_factor;
            let scroll_y = frame.scroll_y * scale_factor;
            let tex_w = tex.width as f32;
            let tex_h = tex.height as f32;

            let visible_h = (self.height as f32).min(tex_h - scroll_y);
            let src_y = scroll_y / tex_h;
            let src_h = visible_h / tex_h;

            let uniforms = Uniforms {
                dest_x,
                dest_y: 0.0,
                dest_w: tex_w.min(self.width as f32 - dest_x),
                dest_h: visible_h,
                src_y,
                src_h,
                screen_w: self.width as f32,
                screen_h: self.height as f32,
            };

            let uniform_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Frame Uniform Buffer {}", i)),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Frame Bind Group {}", i)),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&tex.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });

            bind_groups.push(bind_group);
        }

        // Render
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Frame Compose Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Frame Compose Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

            for bind_group in &bind_groups {
                render_pass.set_bind_group(0, bind_group, &[]);
                render_pass.draw_indexed(0..6, 0, 0..1);
            }
        }

        // Submit and wait for GPU to finish before presenting
        // This is required when using presentsWithTransaction
        self.queue.submit(std::iter::once(encoder.finish()));
        self.device.poll(wgpu::Maintain::Wait);

        output.present();
    }
}
