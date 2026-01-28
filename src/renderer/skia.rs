// Skia (tiny-skia) renderer implementation

use std::collections::HashMap;
use tiny_skia::{BlendMode, Color, FilterQuality, Paint, Pixmap, PixmapPaint, Rect as SkiaRect, Transform};

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

/// Fast rectangle fill - direct write for opaque, tiny-skia for semi-transparent
#[inline]
fn fast_fill_rect(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: ColorTupleA,
) {
    let alpha = color.3;

    if alpha < 0.001 {
        return;
    }

    if alpha >= 0.999 {
        // Opaque: fast direct write
        let px_width = pixmap.width() as i32;
        let px_height = pixmap.height() as i32;

        let x0 = (x as i32).max(0) as usize;
        let y0 = (y as i32).max(0) as usize;
        let x1 = ((x + width) as i32).min(px_width) as usize;
        let y1 = ((y + height) as i32).min(px_height) as usize;

        if x0 >= x1 || y0 >= y1 {
            return;
        }

        let row_width = x1 - x0;
        let pixels = pixmap.pixels_mut();
        let px_width = px_width as usize;

        let pixel = tiny_skia::PremultipliedColorU8::from_rgba(
            color.0 as u8, color.1 as u8, color.2 as u8, 255
        ).unwrap();

        for y in y0..y1 {
            let row_start = y * px_width + x0;
            pixels[row_start..row_start + row_width].fill(pixel);
        }
    } else {
        // Semi-transparent: use tiny-skia for proper CSS alpha blending within frame
        if let Some(rect) = SkiaRect::from_xywh(x, y, width, height) {
            let mut paint = Paint::default();
            paint.set_color(css_color_to_skia(color));
            paint.anti_alias = false;
            pixmap.fill_rect(rect, &paint, Transform::identity(), None);
        }
    }
}

/// Draw a border line (solid or dashed)
fn draw_border_line(
    pixmap: &mut Pixmap,
    _paint: &mut Paint,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    horizontal: bool,
    style: &str,
    color: ColorTupleA,
) {
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
                    fast_fill_rect(pixmap, cx, y, segment_width, height, color);
                    cx += dash_len + gap_len;
                }
            } else {
                let mut cy = y;
                while cy < y + height {
                    let segment_height = dash_len.min(y + height - cy);
                    fast_fill_rect(pixmap, x, cy, width, segment_height, color);
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
                    fast_fill_rect(pixmap, cx, y, dot_size, height, color);
                    cx += dot_size + gap_len;
                }
            } else {
                let mut cy = y;
                while cy < y + height {
                    fast_fill_rect(pixmap, x, cy, width, dot_size, color);
                    cy += dot_size + gap_len;
                }
            }
        }
        _ => {
            // solid (default)
            fast_fill_rect(pixmap, x, y, width, height, color);
        }
    }
}

/// Draw a box shadow with blur approximation
/// Uses layered semi-transparent rectangles to simulate blur
fn draw_box_shadow(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    blur_radius: f32,
    color: ColorTupleA,
) {
    if blur_radius <= 0.0 {
        // No blur - just draw a solid shadow
        fast_fill_rect(pixmap, x, y, width, height, color);
        return;
    }

    // Approximate blur with multiple layers
    // More layers = smoother blur but slower
    let layers = (blur_radius as i32).min(10).max(3);
    let step = blur_radius / layers as f32;

    for i in 0..layers {
        let offset = step * (layers - i) as f32;
        let alpha_factor = (i + 1) as f32 / (layers + 1) as f32;

        let layer_color = (
            color.0,
            color.1,
            color.2,
            color.3 * alpha_factor * 0.5,
        );

        fast_fill_rect(
            pixmap,
            x - offset,
            y - offset,
            width + offset * 2.0,
            height + offset * 2.0,
            layer_color,
        );
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

impl SkiaRenderer {
    /// Render text only (for hybrid GPU/Skia rendering)
    /// Returns a buffer with text on transparent background
    pub fn render_text_only(&mut self, frame: &Frame) -> RenderedBuffer {
        let page_width = (frame.viewport.width * self.scale_factor).max(1.0) as u32;
        let page_pixel_height = ((frame.page_height + 100.0) * self.scale_factor) as u32;
        let page_pixel_height = page_pixel_height.min(8192).max(1);
        let mut page_pm = Pixmap::new(page_width.max(1), page_pixel_height).unwrap();
        page_pm.fill(Color::TRANSPARENT);

        let scale = self.scale_factor;
        let items = frame.render_items();
        let fonts = frame.fonts();

        // Render text only
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
                                                // For text-only, write premultiplied color directly
                                                let premult = tiny_skia::ColorU8::from_rgba(
                                                    color_r, color_g, color_b, alpha
                                                ).premultiply();
                                                page_pm.pixels_mut()[dst_idx] = premult;
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

        // Render non-solid borders (dashed, dotted) - solid borders handled by GPU
        let mut paint = Paint::default();
        for item in items {
            let border = &item.border;

            // Top
            if border.top.is_visible() && !border.top.is_solid() {
                draw_border_line(
                    &mut page_pm, &mut paint,
                    item.x * scale, item.y * scale,
                    item.width * scale, border.top.width * scale,
                    true, &border.top.style, border.top.color
                );
            }

            // Bottom
            if border.bottom.is_visible() && !border.bottom.is_solid() {
                draw_border_line(
                    &mut page_pm, &mut paint,
                    item.x * scale, (item.y + item.height - border.bottom.width) * scale,
                    item.width * scale, border.bottom.width * scale,
                    true, &border.bottom.style, border.bottom.color
                );
            }

            // Left
            if border.left.is_visible() && !border.left.is_solid() {
                draw_border_line(
                    &mut page_pm, &mut paint,
                    item.x * scale, item.y * scale,
                    border.left.width * scale, item.height * scale,
                    false, &border.left.style, border.left.color
                );
            }

            // Right
            if border.right.is_visible() && !border.right.is_solid() {
                draw_border_line(
                    &mut page_pm, &mut paint,
                    (item.x + item.width - border.right.width) * scale, item.y * scale,
                    border.right.width * scale, item.height * scale,
                    false, &border.right.style, border.right.color
                );
            }
        }

        RenderedBuffer::new(page_pm)
    }

    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        self.scale_factor = scale_factor;
    }

    pub fn clear_caches(&mut self) {
        self.glyph_atlas.clear();
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
        let page_pixel_height = page_pixel_height.min(8192).max(1);
        let mut page_pm = Pixmap::new(page_width.max(1), page_pixel_height).unwrap();
        page_pm.fill(Color::TRANSPARENT);

        let scale = self.scale_factor;
        let items = frame.render_items();
        let fonts = frame.fonts();

        // Reuse paint object across items
        let mut paint = Paint::default();
        paint.anti_alias = false;

        // Render all items to page buffer
        for item in items {
            // Draw box shadow (before background)
            if item.box_shadow.is_visible() && !item.box_shadow.inset {
                draw_box_shadow(
                    &mut page_pm,
                    item.x * scale + item.box_shadow.offset_x * scale,
                    item.y * scale + item.box_shadow.offset_y * scale,
                    item.width * scale + item.box_shadow.spread_radius * scale * 2.0,
                    item.height * scale + item.box_shadow.spread_radius * scale * 2.0,
                    item.box_shadow.blur_radius * scale,
                    item.box_shadow.color,
                );
            }

            // Draw background
            if item.background_color != (0.0, 0.0, 0.0, 0.0) {
                fast_fill_rect(
                    &mut page_pm,
                    item.x * scale,
                    item.y * scale,
                    item.width * scale,
                    item.height * scale,
                    item.background_color,
                );
            }

            // Draw borders
            let border = &item.border;

            // Top border
            if border.top.is_visible() {
                draw_border_line(
                    &mut page_pm,
                    &mut paint,
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
                    &mut paint,
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
                    &mut paint,
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
                    &mut paint,
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

        RenderedBuffer::new(page_pm)
    }

    fn composite(&mut self, regions: &[CompositeRegion]) {
        use std::time::Instant;

        let Some(display_pm) = &mut self.display_pixmap else { return };

        let t0 = Instant::now();
        display_pm.fill(Color::WHITE);
        println!("    fill: {:?}", t0.elapsed());

        let t1 = Instant::now();
        let display_width = display_pm.width() as usize;
        let display_height = display_pm.height() as usize;

        for (i, region) in regions.iter().enumerate() {
            let rt = Instant::now();
            let buffer = &region.buffer.pixmap;
            let dest_x = (region.dest_x * self.scale_factor) as i32;
            let scroll_offset = (region.scroll_y * self.scale_factor) as i32;

            if region.opaque {
                // Fast path for opaque regions: direct row copy, skip transparent pixels
                let src_pixels = buffer.pixels();
                let dst_pixels = display_pm.pixels_mut();
                let src_width = buffer.width() as usize;
                let src_height = buffer.height() as usize;
                let dest_x_usize = dest_x.max(0) as usize;

                for y in 0..display_height {
                    let src_y = y as i32 + scroll_offset;
                    if src_y < 0 || src_y >= src_height as i32 {
                        continue;
                    }
                    let src_row_start = src_y as usize * src_width;
                    let dst_row_start = y * display_width + dest_x_usize;
                    let copy_width = src_width.min(display_width.saturating_sub(dest_x_usize));

                    for x in 0..copy_width {
                        let src = src_pixels[src_row_start + x];
                        if src.alpha() > 0 {
                            dst_pixels[dst_row_start + x] = src;
                        }
                    }
                }
            } else {
                // Alpha blending for transparent regions - skip fully transparent pixels
                // Note: tiny-skia uses premultiplied alpha, so RGB are already multiplied by alpha
                let src_pixels = buffer.pixels();
                let dst_pixels = display_pm.pixels_mut();
                let src_width = buffer.width() as usize;
                let src_height = buffer.height() as usize;
                let dest_x_usize = dest_x.max(0) as usize;

                for y in 0..display_height {
                    let src_y = y as i32 + scroll_offset;
                    if src_y < 0 || src_y >= src_height as i32 {
                        continue;
                    }
                    let src_row_start = src_y as usize * src_width;
                    let dst_row_start = y * display_width + dest_x_usize;
                    let row_width = src_width.min(display_width.saturating_sub(dest_x_usize));

                    for x in 0..row_width {
                        let src = src_pixels[src_row_start + x];
                        let alpha = src.alpha();
                        if alpha == 0 {
                            continue; // Skip fully transparent
                        }
                        let dst_idx = dst_row_start + x;
                        if alpha == 255 {
                            // Fully opaque - direct copy
                            dst_pixels[dst_idx] = src;
                        } else {
                            // Premultiplied alpha blend: dst = src + dst * (1 - src_alpha)
                            let dst = dst_pixels[dst_idx];
                            let inv_a = 255 - alpha as u16;
                            let r = src.red() as u16 + (dst.red() as u16 * inv_a) / 255;
                            let g = src.green() as u16 + (dst.green() as u16 * inv_a) / 255;
                            let b = src.blue() as u16 + (dst.blue() as u16 * inv_a) / 255;
                            dst_pixels[dst_idx] = tiny_skia::PremultipliedColorU8::from_rgba(
                                r.min(255) as u8, g.min(255) as u8, b.min(255) as u8, 255
                            ).unwrap();
                        }
                    }
                }
            }
            println!("      region {}: {:?} ({}x{})", i, rt.elapsed(), buffer.width(), buffer.height());
        }
        println!("    draw_pixmaps: {:?}", t1.elapsed());

        // Convert to display buffer format
        let t2 = Instant::now();
        let display_pm_ref = self.display_pixmap.as_ref().unwrap();
        for (i, pixel) in display_pm_ref.pixels().iter().enumerate() {
            let r = pixel.red() as u32;
            let g = pixel.green() as u32;
            let b = pixel.blue() as u32;
            self.display_buffer[i] = (r << 16) | (g << 8) | b;
        }
        println!("    convert: {:?}", t2.elapsed());
    }

    fn get_display_buffer(&self) -> &[u32] {
        &self.display_buffer
    }

    fn clear_caches(&mut self) {
        self.glyph_atlas.clear();
    }
}
