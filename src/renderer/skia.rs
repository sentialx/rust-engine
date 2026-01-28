// Skia (tiny-skia) glyph atlas and text quad generation
// Used by HybridRenderer for GPU text rendering

use std::collections::HashMap;
use tiny_skia::{Color, Pixmap};

use crate::frame::Frame;
use crate::text::FontManager;

/// Glyph info stored in the atlas
#[derive(Clone, Copy)]
pub struct GlyphInfo {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub xmin: i32,
    pub ymin: i32,
    pub advance_width: f32,
}

/// Key for glyph atlas lookup
#[derive(Hash, Eq, PartialEq, Clone)]
pub struct GlyphAtlasKey {
    pub character: char,
    pub font_path: String,
    pub font_size_scaled: u32,
}

/// A single text glyph quad for GPU rendering
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextQuad {
    /// Screen position (x, y, width, height) in pixels
    pub pos: [f32; 4],
    /// UV coordinates in atlas (u, v, u_size, v_size) normalized 0-1
    pub uv: [f32; 4],
    /// Color (r, g, b, a)
    pub color: [f32; 4],
}

/// Glyph atlas - stores rasterized glyphs in a single texture
pub struct GlyphAtlas {
    pixmap: Pixmap,
    glyphs: HashMap<GlyphAtlasKey, GlyphInfo>,
    next_x: u32,
    next_y: u32,
    row_height: u32,
    /// Version counter - incremented when atlas is modified
    pub version: u64,
}

impl GlyphAtlas {
    fn new() -> Self {
        Self {
            pixmap: Pixmap::new(2048, 2048).unwrap(),
            glyphs: HashMap::new(),
            next_x: 0,
            next_y: 0,
            row_height: 0,
            version: 0,
        }
    }

    /// Get the atlas pixmap data for GPU upload
    pub fn pixmap_data(&self) -> &[u8] {
        self.pixmap.data()
    }

    /// Get atlas dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.pixmap.width(), self.pixmap.height())
    }

    fn get_or_insert(
        &mut self,
        key: GlyphAtlasKey,
        fonts: &FontManager,
        font_size: f32,
    ) -> Option<&GlyphInfo> {
        if !self.glyphs.contains_key(&key) {
            if let Some(glyph) = fonts.rasterize(key.character, &key.font_path, font_size) {
                if glyph.width > 0 && glyph.height > 0 {
                    // Check if we need to wrap to next row
                    if self.next_x + glyph.width as u32 > self.pixmap.width() {
                        self.next_x = 0;
                        self.next_y += self.row_height + 1;
                        self.row_height = 0;
                    }

                    let atlas_x = self.next_x;
                    let atlas_y = self.next_y;
                    let pixmap_width = self.pixmap.width();
                    let pixmap_height = self.pixmap.height();

                    // Copy glyph to atlas (white with alpha)
                    for gy in 0..glyph.height {
                        for gx in 0..glyph.width {
                            let alpha = glyph.bitmap[gy * glyph.width + gx];
                            if alpha > 0 {
                                let px = atlas_x + gx as u32;
                                let py = atlas_y + gy as u32;
                                if px < pixmap_width && py < pixmap_height {
                                    let pixel = tiny_skia::ColorU8::from_rgba(255, 255, 255, alpha).premultiply();
                                    self.pixmap.pixels_mut()[(py * pixmap_width + px) as usize] = pixel;
                                }
                            }
                        }
                    }

                    let info = GlyphInfo {
                        x: atlas_x,
                        y: atlas_y,
                        width: glyph.width as u32,
                        height: glyph.height as u32,
                        xmin: glyph.xmin,
                        ymin: glyph.ymin,
                        advance_width: glyph.advance_width,
                    };

                    self.next_x += glyph.width as u32 + 1;
                    self.row_height = self.row_height.max(glyph.height as u32);

                    self.glyphs.insert(key.clone(), info);
                    self.version += 1;
                } else {
                    // Empty glyph (space, etc.)
                    let info = GlyphInfo {
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                        xmin: glyph.xmin,
                        ymin: glyph.ymin,
                        advance_width: glyph.advance_width,
                    };
                    self.glyphs.insert(key.clone(), info);
                }
            }
        }

        self.glyphs.get(&key)
    }

    fn clear(&mut self) {
        self.pixmap.fill(Color::TRANSPARENT);
        self.glyphs.clear();
        self.next_x = 0;
        self.next_y = 0;
        self.row_height = 0;
        self.version += 1;
    }
}

/// Manages glyph atlas and generates text quads for GPU rendering
pub struct SkiaRenderer {
    glyph_atlas: GlyphAtlas,
    scale_factor: f32,
}

impl SkiaRenderer {
    pub fn new() -> Self {
        Self {
            glyph_atlas: GlyphAtlas::new(),
            scale_factor: 1.0,
        }
    }

    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        self.scale_factor = scale_factor;
    }

    /// Access the glyph atlas for GPU upload
    pub fn glyph_atlas(&self) -> &GlyphAtlas {
        &self.glyph_atlas
    }

    pub fn clear_caches(&mut self) {
        self.glyph_atlas.clear();
    }

    /// Generate text quads for GPU rendering (viewport-relative)
    /// Returns quads for all visible text, with atlas UVs
    pub fn generate_text_quads(&mut self, frame: &Frame, scroll_y: f32) -> Vec<TextQuad> {
        let scale = self.scale_factor;
        let items = frame.render_items();
        let fonts = frame.fonts();

        let visible_top = scroll_y;
        let visible_bottom = scroll_y + frame.viewport.height;
        let atlas_w = self.glyph_atlas.pixmap.width() as f32;
        let atlas_h = self.glyph_atlas.pixmap.height() as f32;

        let mut quads = Vec::new();

        for item in items.iter().filter(|item| !item.text_segments.is_empty()) {
            let color = [
                item.color.0 / 255.0,
                item.color.1 / 255.0,
                item.color.2 / 255.0,
                item.color.3,
            ];
            let scaled_font_size = item.font_size * scale;
            let font_size_key = (scaled_font_size * 100.0) as u32;

            for seg in &item.text_segments {
                // Skip segments outside visible range
                let seg_top = seg.y - seg.ascent;
                let seg_bottom = seg.y + seg.height;
                if seg_bottom < visible_top - 50.0 || seg_top > visible_bottom + 50.0 {
                    continue;
                }

                let baseline_y = seg.y + seg.ascent - scroll_y;
                let mut x_offset = seg.x * scale;

                for c in seg.text.chars() {
                    let atlas_key = GlyphAtlasKey {
                        character: c,
                        font_path: item.font_path.clone(),
                        font_size_scaled: font_size_key,
                    };

                    if fonts.has_font(&item.font_path) {
                        if let Some(glyph_info) = self.glyph_atlas.get_or_insert(
                            atlas_key,
                            fonts,
                            scaled_font_size,
                        ) {
                            let glyph_info = *glyph_info; // Copy to avoid borrow issues

                            if glyph_info.width > 0 && glyph_info.height > 0 {
                                let glyph_x = x_offset + glyph_info.xmin as f32;
                                let glyph_y = baseline_y * scale - glyph_info.ymin as f32 - glyph_info.height as f32;

                                quads.push(TextQuad {
                                    pos: [glyph_x, glyph_y, glyph_info.width as f32, glyph_info.height as f32],
                                    uv: [
                                        glyph_info.x as f32 / atlas_w,
                                        glyph_info.y as f32 / atlas_h,
                                        glyph_info.width as f32 / atlas_w,
                                        glyph_info.height as f32 / atlas_h,
                                    ],
                                    color,
                                });
                            }

                            x_offset += glyph_info.advance_width;
                        } else {
                            x_offset += scaled_font_size * 0.6;
                        }
                    } else {
                        x_offset += scaled_font_size * 0.6;
                    }
                }
            }
        }

        quads
    }
}
