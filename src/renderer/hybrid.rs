// HybridRenderer: combines GPU quad rendering with Skia text rendering

use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;

use crate::frame::Frame;
use super::skia::SkiaRenderer;
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
struct CompositeUniforms {
    tex_w: f32,
    tex_h: f32,
    _padding: [f32; 2],
}

/// Cached frame data
struct CachedFrame {
    texture: FrameTexture,
}

/// Combines WgpuRenderer (quads) + SkiaRenderer (text) into FrameTextures
pub struct HybridRenderer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,

    // Sub-renderers
    quad_renderer: WgpuRenderer,
    skia_renderer: SkiaRenderer,

    // Compositing pipeline (quads + text → output)
    composite_pipeline: wgpu::RenderPipeline,
    composite_bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,

    // Cached text texture
    text_texture: Option<(wgpu::Texture, wgpu::TextureView, u32, u32)>,

    // Cached frames by ID
    frames: HashMap<usize, CachedFrame>,

    scale_factor: f32,
}

impl HybridRenderer {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let quad_renderer = WgpuRenderer::new(Arc::clone(&device), Arc::clone(&queue));
        let skia_renderer = SkiaRenderer::new();

        // Compositing shader - blends text over quads
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Hybrid Composite Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("text_blend.wgsl").into()),
        });

        let composite_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Hybrid Bind Group Layout"),
            entries: &[
                // Uniforms
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
                // Text texture
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Hybrid Pipeline Layout"),
            bind_group_layouts: &[&composite_bind_group_layout],
            push_constant_ranges: &[],
        });

        let composite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Hybrid Composite Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
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

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            device,
            queue,
            quad_renderer,
            skia_renderer,
            composite_pipeline,
            composite_bind_group_layout,
            sampler,
            text_texture: None,
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

    /// Render a frame to a cached texture
    pub fn render(&mut self, frame: &Frame, frame_id: usize) {
        let scale = self.scale_factor;

        // Get render items and convert to quads
        let items = frame.render_items();
        let quads = GpuQuad::from_render_items_scaled(&items, scale);

        // Calculate dimensions
        let page_width = (frame.viewport.width * scale).max(1.0) as u32;
        let page_height = ((frame.page_height + 100.0) * scale).min(8192.0).max(1.0) as u32;

        // 1. Render quads to texture
        let quad_texture = self.quad_renderer.render(&quads, page_width, page_height);

        // 2. Render text with Skia
        let text_buffer = self.skia_renderer.render_text_only(frame);
        let text_pixmap = &text_buffer.pixmap;

        // 3. Upload text pixmap to GPU texture
        let text_w = text_pixmap.width();
        let text_h = text_pixmap.height();

        let needs_new_text = self.text_texture.as_ref()
            .map(|(_, _, w, h)| *w != text_w || *h != text_h)
            .unwrap_or(true);

        if needs_new_text {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Text Texture"),
                size: wgpu::Extent3d { width: text_w, height: text_h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.text_texture = Some((texture, view, text_w, text_h));
        }

        // Upload text pixels
        let (text_tex, text_view, _, _) = self.text_texture.as_ref().unwrap();
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: text_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            text_pixmap.data(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(text_w * 4),
                rows_per_image: Some(text_h),
            },
            wgpu::Extent3d { width: text_w, height: text_h, depth_or_array_layers: 1 },
        );

        // 4. Ensure output texture exists with correct size
        let out_w = page_width;
        let out_h = page_height;

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

        // 5. Composite: copy quads, then blend text on top
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Hybrid Composite Encoder"),
        });

        // Copy quad texture to output
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture {
                texture: &quad_texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
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
                height: out_h.min(quad_texture.height),
                depth_or_array_layers: 1,
            },
        );

        // Blend text on top
        let uniforms = CompositeUniforms {
            tex_w: out_w as f32,
            tex_h: out_h as f32,
            _padding: [0.0; 2],
        };
        let uniform_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Hybrid Uniforms"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Hybrid Bind Group"),
            layout: &self.composite_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(text_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Text Blend Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Keep quads, blend text on top
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.composite_pipeline);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.draw(0..6, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Get a cached frame texture by ID
    pub fn get_texture(&self, frame_id: usize) -> Option<&FrameTexture> {
        self.frames.get(&frame_id).map(|f| &f.texture)
    }

    pub fn clear_caches(&mut self) {
        self.frames.clear();
        self.skia_renderer.clear_caches();
    }
}
