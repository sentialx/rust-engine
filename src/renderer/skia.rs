// Skia (tiny-skia) renderer implementation

use std::collections::HashMap;
use tiny_skia::{Color, Paint, Pixmap, Rect as SkiaRect, Transform};

use crate::colors::ColorTupleA;
use crate::frame::Frame;
use crate::text::FontManager;
use super::{CompositeRegion, RenderedBuffer, Renderer};

fn css_color_to_skia(c: ColorTupleA) -> Color {
    Color::from_rgba(
        c.0 as f32 / 255.0,
        c.1 as f32 / 255.0,
        c.2 as f32 / 255.0,
        c.3 as f32,
    )
    .unwrap_or(Color::BLACK)
}

/// Draw a border line (solid or dashed)
fn draw_border_line(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    horizontal: bool,
    style: &str,
    color: ColorTupleA,
) {
    let mut paint = Paint::default();
    paint.set_color(css_color_to_skia(color));
    paint.anti_alias = false;

    match style {
        "dashed" => {
            // Dash length is typically 3x the border width
            let border_width = if horizontal { height } else { width };
            let dash_len = (border_width * 3.0).max(3.0);
            let gap_len = dash_len;

            if horizontal {
                let mut cx = x;
                while cx < x + width {
                    let segment_width = dash_len.min(x + width - cx);
                    if let Some(rect) = SkiaRect::from_xywh(cx, y, segment_width, height) {
                        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
                    }
                    cx += dash_len + gap_len;
                }
            } else {
                let mut cy = y;
                while cy < y + height {
                    let segment_height = dash_len.min(y + height - cy);
                    if let Some(rect) = SkiaRect::from_xywh(x, cy, width, segment_height) {
                        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
                    }
                    cy += dash_len + gap_len;
                }
            }
        }
        "dotted" => {
            // Dots are typically 1x the border width with 1x gap
            let border_width = if horizontal { height } else { width };
            let dot_size = border_width.max(1.0);
            let gap_len = dot_size;

            if horizontal {
                let mut cx = x;
                while cx < x + width {
                    if let Some(rect) = SkiaRect::from_xywh(cx, y, dot_size, height) {
                        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
                    }
                    cx += dot_size + gap_len;
                }
            } else {
                let mut cy = y;
                while cy < y + height {
                    if let Some(rect) = SkiaRect::from_xywh(x, cy, width, dot_size) {
                        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
                    }
                    cy += dot_size + gap_len;
                }
            }
        }
        _ => {
            // solid (default)
            if let Some(rect) = SkiaRect::from_xywh(x, y, width, height) {
                pixmap.fill_rect(rect, &paint, Transform::identity(), None);
            }
        }
    }
}

/// Glyph info stored in the atlas
struct GlyphInfo {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    xmin: i32,
    ymin: i32,
    advance_width: f32,
}

/// Key for glyph atlas
#[derive(Hash, Eq, PartialEq, Clone)]
struct GlyphAtlasKey {
    character: char,
    font_path: String,
    font_size_scaled: u32,
}

/// Glyph atlas - stores all glyphs in a single texture
struct GlyphAtlas {
    pixmap: Pixmap,
    glyphs: HashMap<GlyphAtlasKey, GlyphInfo>,
    next_x: u32,
    next_y: u32,
    row_height: u32,
}

impl GlyphAtlas {
    fn new() -> Self {
        Self {
            pixmap: Pixmap::new(2048, 2048).unwrap(),
            glyphs: HashMap::new(),
            next_x: 0,
            next_y: 0,
            row_height: 0,
        }
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

                    // Copy glyph to atlas
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
    }
}

/// Skia-based renderer using tiny-skia
pub struct SkiaRenderer {
    // Display buffer (window size)
    display_pixmap: Option<Pixmap>,
    display_buffer: Vec<u32>,

    // Glyph rendering
    glyph_atlas: GlyphAtlas,

    // State
    scale_factor: f32,
    width: u32,
    height: u32,
}

impl SkiaRenderer {
    pub fn new() -> Self {
        Self {
            display_pixmap: None,
            display_buffer: Vec::new(),
            glyph_atlas: GlyphAtlas::new(),
            scale_factor: 1.0,
            width: 0,
            height: 0,
        }
    }
}

impl Renderer for SkiaRenderer {
    fn resize(&mut self, width: u32, height: u32, scale_factor: f32) {
        if width == 0 || height == 0 {
            return;
        }

        self.width = width;
        self.height = height;
        self.scale_factor = scale_factor;

        // Recreate display pixmap
        self.display_pixmap = Pixmap::new(width, height);
        self.display_buffer.resize((width * height) as usize, 0);
    }

    fn render(&mut self, frame: &Frame) -> RenderedBuffer {
        let page_width = (frame.viewport.width * self.scale_factor).max(1.0) as u32;
        let page_pixel_height = ((frame.page_height + 100.0) * self.scale_factor) as u32;
        let page_pixel_height = page_pixel_height.min(16384).max(1);
        let mut page_pm = Pixmap::new(page_width.max(1), page_pixel_height).unwrap();
        page_pm.fill(Color::TRANSPARENT);

        let scale = self.scale_factor;
        let items = frame.render_items();
        let fonts = frame.fonts();

        // Render all items to page buffer
        for item in items {
            // Draw background
            if item.background_color != (0.0, 0.0, 0.0, 0.0) {
                let mut paint = Paint::default();
                paint.set_color(css_color_to_skia(item.background_color));
                paint.anti_alias = false;

                if let Some(rect) = SkiaRect::from_xywh(
                    item.x * scale,
                    item.y * scale,
                    item.width * scale,
                    item.height * scale,
                ) {
                    page_pm.fill_rect(rect, &paint, Transform::identity(), None);
                }
            }

            // Draw borders
            let border = &item.border;

            // Top border
            if border.top.is_visible() {
                draw_border_line(
                    &mut page_pm,
                    item.x * scale,
                    item.y * scale,
                    item.width * scale,
                    border.top.width * scale,
                    true, // horizontal
                    &border.top.style,
                    border.top.color,
                );
            }

            // Bottom border
            if border.bottom.is_visible() {
                draw_border_line(
                    &mut page_pm,
                    item.x * scale,
                    (item.y + item.height) * scale - border.bottom.width * scale,
                    item.width * scale,
                    border.bottom.width * scale,
                    true, // horizontal
                    &border.bottom.style,
                    border.bottom.color,
                );
            }

            // Left border
            if border.left.is_visible() {
                draw_border_line(
                    &mut page_pm,
                    item.x * scale,
                    item.y * scale,
                    border.left.width * scale,
                    item.height * scale,
                    false, // vertical
                    &border.left.style,
                    border.left.color,
                );
            }

            // Right border
            if border.right.is_visible() {
                draw_border_line(
                    &mut page_pm,
                    (item.x + item.width) * scale - border.right.width * scale,
                    item.y * scale,
                    border.right.width * scale,
                    item.height * scale,
                    false, // vertical
                    &border.right.style,
                    border.right.color,
                );
            }
        }

        // Second pass for text - inline to avoid borrow issues
        let items_for_text: Vec<_> = items.iter()
            .filter(|item| !item.text_segments.is_empty())
            .cloned()
            .collect();

        for item in &items_for_text {
            let color_r = item.color.0 as u8;
            let color_g = item.color.1 as u8;
            let color_b = item.color.2 as u8;
            let scaled_font_size = item.font_size * scale;
            let font_size_key = (scaled_font_size * 100.0) as u32;

            for seg in &item.text_segments {
                let baseline_y = seg.y + seg.ascent;
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
                            let (g_x, g_y, g_w, g_h, g_xmin, g_ymin, g_advance) = (
                                glyph_info.x,
                                glyph_info.y,
                                glyph_info.width,
                                glyph_info.height,
                                glyph_info.xmin,
                                glyph_info.ymin,
                                glyph_info.advance_width,
                            );

                            if g_w > 0 && g_h > 0 {
                                let glyph_x = (x_offset + g_xmin as f32) as i32;
                                let glyph_y = (baseline_y * scale - g_ymin as f32 - g_h as f32) as i32;

                                let page_width = page_pm.width();
                                let page_height = page_pm.height();
                                let atlas_width = self.glyph_atlas.pixmap.width();
                                let atlas_pixels = self.glyph_atlas.pixmap.pixels();

                                for gy in 0..g_h {
                                    for gx in 0..g_w {
                                        let atlas_idx = ((g_y + gy) * atlas_width + g_x + gx) as usize;
                                        let alpha = atlas_pixels[atlas_idx].alpha();

                                        if alpha > 0 {
                                            let px = glyph_x + gx as i32;
                                            let py = glyph_y + gy as i32;

                                            if px >= 0 && py >= 0 && (px as u32) < page_width && (py as u32) < page_height {
                                                let dst_idx = (py as u32 * page_width + px as u32) as usize;
                                                let existing = page_pm.pixels()[dst_idx];
                                                let a = alpha as f32 / 255.0;
                                                let blended = tiny_skia::ColorU8::from_rgba(
                                                    (color_r as f32 * a + existing.red() as f32 * (1.0 - a)) as u8,
                                                    (color_g as f32 * a + existing.green() as f32 * (1.0 - a)) as u8,
                                                    (color_b as f32 * a + existing.blue() as f32 * (1.0 - a)) as u8,
                                                    255,
                                                ).premultiply();
                                                page_pm.pixels_mut()[dst_idx] = blended;
                                            }
                                        }
                                    }
                                }
                            }

                            x_offset += g_advance;
                        } else {
                            x_offset += scaled_font_size * 0.6;
                        }
                    } else {
                        x_offset += scaled_font_size * 0.6;
                    }
                }

                // Draw underline if needed
                if item.underline {
                    let color = css_color_to_skia(item.color);
                    let mut paint = Paint::default();
                    paint.set_color(color);
                    if let Some(rect) = SkiaRect::from_xywh(
                        seg.x * scale,
                        (baseline_y + 2.0) * scale,
                        seg.width * scale,
                        1.0 * scale,
                    ) {
                        page_pm.fill_rect(rect, &paint, Transform::identity(), None);
                    }
                }
            }
        }

        RenderedBuffer { pixmap: page_pm }
    }

    fn composite(&mut self, regions: &[CompositeRegion]) {
        let Some(display_pm) = &mut self.display_pixmap else { return };
        display_pm.fill(Color::WHITE);

        let display_width = display_pm.width();
        let display_height = display_pm.height();
        let display_width_i = display_width as i32;
        let display_height_i = display_height as i32;

        for region in regions {
            let buffer = &region.buffer.pixmap;
            let dest_x = (region.dest_x * self.scale_factor) as i32;
            let scroll_offset = (region.scroll_y * self.scale_factor) as i32;

            let buffer_width = buffer.width() as i32;
            let buffer_height = buffer.height() as i32;

            for y in 0..display_height_i {
                let src_y = y + scroll_offset;
                if src_y < 0 || src_y >= buffer_height {
                    continue;
                }

                let src_row_start = (src_y as u32 * buffer.width()) as usize;
                let dst_row_start = (y as u32 * display_width) as usize;

                for x in 0..buffer_width {
                    let dst_x = dest_x + x;
                    if dst_x < 0 || dst_x >= display_width_i {
                        continue;
                    }

                    let src_idx = src_row_start + x as usize;
                    let dst_idx = dst_row_start + dst_x as usize;
                    let src = buffer.pixels()[src_idx];
                    let alpha = src.alpha();
                    if alpha == 0 {
                        continue;
                    }

                    let dst = display_pm.pixels()[dst_idx];
                    let a = alpha as f32 / 255.0;
                    let inv = 1.0 - a;
                    let blended = tiny_skia::ColorU8::from_rgba(
                        (src.red() as f32 + dst.red() as f32 * inv) as u8,
                        (src.green() as f32 + dst.green() as f32 * inv) as u8,
                        (src.blue() as f32 + dst.blue() as f32 * inv) as u8,
                        255,
                    ).premultiply();
                    display_pm.pixels_mut()[dst_idx] = blended;
                }
            }
        }

        // Convert to display buffer format
        let display_pm_ref = self.display_pixmap.as_ref().unwrap();
        for (i, pixel) in display_pm_ref.pixels().iter().enumerate() {
            let r = pixel.red() as u32;
            let g = pixel.green() as u32;
            let b = pixel.blue() as u32;
            self.display_buffer[i] = (r << 16) | (g << 8) | b;
        }
    }

    fn get_display_buffer(&self) -> &[u32] {
        &self.display_buffer
    }

    fn clear_caches(&mut self) {
        self.glyph_atlas.clear();
    }
}
