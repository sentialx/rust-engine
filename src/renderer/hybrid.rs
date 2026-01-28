// HybridRenderer: combines GPU quad rendering with GPU text rendering

use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;

use crate::frame::Frame;
use super::skia::{SkiaRenderer, TextQuad};
use super::wgpu_renderer::WgpuRenderer;
use super::GpuQuad;

/// Output texture from HybridRenderer
pub struct FrameTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct TextUniforms {
    screen_w: f32,
    screen_h: f32,
    _padding: [f32; 2],
}

/// Cached frame data
struct CachedFrame {
    texture: FrameTexture,
}

/// Combines WgpuRenderer (quads) + GPU text rendering into FrameTextures
pub struct HybridRenderer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,

    // Sub-renderers
    quad_renderer: WgpuRenderer,
    skia_renderer: SkiaRenderer,

    // Text rendering pipeline
    text_pipeline: wgpu::RenderPipeline,
    text_bind_group_layout: wgpu::BindGroupLayout,
    text_sampler: wgpu::Sampler,
    text_uniform_buffer: wgpu::Buffer,

    // Glyph atlas on GPU
    glyph_atlas_texture: Option<(wgpu::Texture, wgpu::TextureView)>,
    glyph_atlas_version: u64,

    // Text instance buffer
    text_instance_buffer: wgpu::Buffer,
    text_instance_capacity: usize,

    // Cached frames by ID
    frames: HashMap<usize, CachedFrame>,

    scale_factor: f32,
}

impl HybridRenderer {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let quad_renderer = WgpuRenderer::new(Arc::clone(&device), Arc::clone(&queue));
        let skia_renderer = SkiaRenderer::new();

        // Text rendering shader
        let text_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Text Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("text.wgsl").into()),
        });

        let text_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Text Bind Group Layout"),
            entries: &[
                // Uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Glyph atlas texture
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
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let text_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Text Pipeline Layout"),
            bind_group_layouts: &[&text_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Instance buffer layout for text quads
        let text_instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TextQuad>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // pos: vec4<f32>
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // uv: vec4<f32>
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // color: vec4<f32>
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        };

        let text_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text Pipeline"),
            layout: Some(&text_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &text_shader,
                entry_point: Some("vs_main"),
                buffers: &[text_instance_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &text_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let text_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let text_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Text Uniform Buffer"),
            size: std::mem::size_of::<TextUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let initial_capacity = 4096;
        let text_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Text Instance Buffer"),
            size: (initial_capacity * std::mem::size_of::<TextQuad>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            device,
            queue,
            quad_renderer,
            skia_renderer,
            text_pipeline,
            text_bind_group_layout,
            text_sampler,
            text_uniform_buffer,
            glyph_atlas_texture: None,
            glyph_atlas_version: 0,
            text_instance_buffer,
            text_instance_capacity: initial_capacity,
            frames: HashMap::new(),
            scale_factor: 1.0,
        }
    }

    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        if (self.scale_factor - scale_factor).abs() > 0.001 {
            self.scale_factor = scale_factor;
            self.skia_renderer.set_scale_factor(scale_factor);
            // Clear cached frames when scale changes
            self.frames.clear();
        }
    }

    /// Render a frame to a cached texture using GPU text rendering
    pub fn render(&mut self, frame: &Frame, frame_id: usize, scroll_y: f32) {
        use std::time::Instant;
        let scale = self.scale_factor;

        // Get render items and convert to quads
        let items = frame.render_items();
        let quads = GpuQuad::from_render_items_scaled(&items, scale);

        // Calculate dimensions - use viewport size for output
        let viewport_width = (frame.viewport.width * scale).max(1.0) as u32;
        let viewport_height = (frame.viewport.height * scale).max(1.0) as u32;
        let page_height = ((frame.page_height + 100.0) * scale).min(8192.0).max(1.0) as u32;
        let scroll_y_scaled = (scroll_y * scale) as u32;

        // 1. Render quads to texture (full page height for scroll support)
        let t_quads = Instant::now();
        let quad_texture = self.quad_renderer.render(&quads, viewport_width, page_height);
        let quad_time = t_quads.elapsed();

        // 2. Generate text quads for GPU rendering
        let t_text = Instant::now();
        let text_quads = self.skia_renderer.generate_text_quads(frame, scroll_y);
        let text_time = t_text.elapsed();

        // 3. Update glyph atlas texture if needed
        let t_atlas = Instant::now();
        let atlas = self.skia_renderer.glyph_atlas();
        let atlas_version = atlas.version;
        let (atlas_w, atlas_h) = atlas.dimensions();

        if self.glyph_atlas_version != atlas_version || self.glyph_atlas_texture.is_none() {
            // Create or recreate atlas texture
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Glyph Atlas"),
                size: wgpu::Extent3d { width: atlas_w, height: atlas_h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            // Upload atlas data
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                atlas.pixmap_data(),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(atlas_w * 4),
                    rows_per_image: Some(atlas_h),
                },
                wgpu::Extent3d { width: atlas_w, height: atlas_h, depth_or_array_layers: 1 },
            );

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.glyph_atlas_texture = Some((texture, view));
            self.glyph_atlas_version = atlas_version;
        }
        let atlas_time = t_atlas.elapsed();

        // 4. Upload text instance data
        let t_upload = Instant::now();
        if !text_quads.is_empty() {
            // Grow buffer if needed
            if text_quads.len() > self.text_instance_capacity {
                self.text_instance_capacity = text_quads.len().next_power_of_two();
                self.text_instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Text Instance Buffer"),
                    size: (self.text_instance_capacity * std::mem::size_of::<TextQuad>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            self.queue.write_buffer(&self.text_instance_buffer, 0, bytemuck::cast_slice(&text_quads));
        }
        let upload_time = t_upload.elapsed();

        // 5. Ensure output texture exists with correct size (viewport-sized)
        let out_w = viewport_width;
        let out_h = viewport_height;

        let needs_new_output = self.frames.get(&frame_id)
            .map(|f| f.texture.width != out_w || f.texture.height != out_h)
            .unwrap_or(true);

        if needs_new_output {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("Frame {} Output", frame_id)),
                size: wgpu::Extent3d { width: out_w, height: out_h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                     | wgpu::TextureUsages::TEXTURE_BINDING
                     | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.frames.insert(frame_id, CachedFrame {
                texture: FrameTexture { texture, view, width: out_w, height: out_h },
            });
        }

        let output = &self.frames.get(&frame_id).unwrap().texture;

        // 6. Composite: copy quads, then render text on top
        let t_composite = Instant::now();
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Hybrid Composite Encoder"),
        });

        // Clear output texture first (prevents garbage when scrolled past content)
        // Use transparent so overlay frames composite correctly
        {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Output Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            // Pass ends immediately, clearing the texture
        }

        // Copy visible portion of quad texture to output (at scroll offset)
        let copy_height = out_h.min(quad_texture.height.saturating_sub(scroll_y_scaled));
        if copy_height > 0 {
            encoder.copy_texture_to_texture(
                wgpu::ImageCopyTexture {
                    texture: &quad_texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: scroll_y_scaled, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::ImageCopyTexture {
                    texture: &output.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: out_w.min(quad_texture.width),
                    height: copy_height,
                    depth_or_array_layers: 1,
                },
            );
        }

        // Render text on top using GPU
        if !text_quads.is_empty() {
            // Update uniforms
            let uniforms = TextUniforms {
                screen_w: out_w as f32,
                screen_h: out_h as f32,
                _padding: [0.0; 2],
            };
            self.queue.write_buffer(&self.text_uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

            let (_, atlas_view) = self.glyph_atlas_texture.as_ref().unwrap();

            let text_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Text Bind Group"),
                layout: &self.text_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.text_uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(atlas_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.text_sampler),
                    },
                ],
            });

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Text Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &output.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load, // Keep quads, render text on top
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                render_pass.set_pipeline(&self.text_pipeline);
                render_pass.set_bind_group(0, &text_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.text_instance_buffer.slice(..));
                render_pass.draw(0..6, 0..text_quads.len() as u32);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        let composite_time = t_composite.elapsed();

        println!("    quads:{:?} text_gen:{:?} atlas:{:?} upload:{:?} comp:{:?} ({} glyphs)",
            quad_time, text_time, atlas_time, upload_time, composite_time, text_quads.len());
    }

    /// Get a cached frame texture by ID
    pub fn get_texture(&self, frame_id: usize) -> Option<&FrameTexture> {
        self.frames.get(&frame_id).map(|f| &f.texture)
    }

    pub fn clear_caches(&mut self) {
        self.frames.clear();
        self.skia_renderer.clear_caches();
        self.glyph_atlas_texture = None;
        self.glyph_atlas_version = 0;
    }

    /// Read back pixels from a rendered frame texture as RGBA bytes
    pub fn read_pixels(&self, frame_id: usize) -> Option<(Vec<u8>, u32, u32)> {
        let frame = self.frames.get(&frame_id)?;
        let texture = &frame.texture;
        let width = texture.width;
        let height = texture.height;

        // Calculate buffer size with alignment (wgpu requires 256-byte row alignment)
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;
        let buffer_size = (padded_bytes_per_row * height) as u64;

        // Create staging buffer
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Screenshot Staging Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy texture to buffer
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Screenshot Encoder"),
        });

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &staging_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        // Map buffer and read data
        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });

        self.device.poll(wgpu::Maintain::Wait);

        if rx.recv().ok()?.is_err() {
            return None;
        }

        let data = buffer_slice.get_mapped_range();

        // Remove row padding if present
        let mut pixels = Vec::with_capacity((width * height * bytes_per_pixel) as usize);
        for row in 0..height {
            let start = (row * padded_bytes_per_row) as usize;
            let end = start + (width * bytes_per_pixel) as usize;
            pixels.extend_from_slice(&data[start..end]);
        }

        drop(data);
        staging_buffer.unmap();

        Some((pixels, width, height))
    }
}
