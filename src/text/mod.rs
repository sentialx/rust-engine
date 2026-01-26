// Text measurement and font handling - backend agnostic

use std::collections::HashMap;
use std::fs;

/// Trait for measuring text dimensions - used by layout
pub trait TextMeasurer {
    /// Measure text, returning (width, height)
    fn measure(&mut self, text: &str, font_size: f32, font_family: &str) -> (f32, f32);

    /// Get the ascent (distance from baseline to top) for a font at given size
    fn ascent(&mut self, font_size: f32, font_family: &str) -> f32;
}

/// Simple box-based text measurer for testing
pub struct BoxTextMeasurer {
    pub char_width_ratio: f32,
    pub ascent_ratio: f32,
}

impl Default for BoxTextMeasurer {
    fn default() -> Self {
        Self {
            char_width_ratio: 0.6,
            ascent_ratio: 0.8,
        }
    }
}

impl TextMeasurer for BoxTextMeasurer {
    fn measure(&mut self, text: &str, font_size: f32, _font_family: &str) -> (f32, f32) {
        let width = text.chars().count() as f32 * font_size * self.char_width_ratio;
        let height = font_size;
        (width, height)
    }

    fn ascent(&mut self, font_size: f32, _font_family: &str) -> f32 {
        font_size * self.ascent_ratio
    }
}

/// Font manager - handles font loading, measurement, and rasterization
pub struct FontManager {
    fonts: HashMap<String, fontdue::Font>,
}

impl FontManager {
    pub fn new() -> Self {
        Self {
            fonts: HashMap::new(),
        }
    }

    pub fn load_font(&mut self, name: &str, path: &str) {
        let font_data = fs::read(path).expect(&format!("Failed to load font: {}", path));
        let font = fontdue::Font::from_bytes(font_data, fontdue::FontSettings::default())
            .expect(&format!("Failed to parse font: {}", path));
        self.fonts.insert(name.to_string(), font);
    }

    pub fn get_font(&self, font_family: &str) -> Option<&fontdue::Font> {
        self.fonts.get(font_family)
    }

    pub fn has_font(&self, font_family: &str) -> bool {
        self.fonts.contains_key(font_family)
    }

    /// Rasterize a character glyph
    pub fn rasterize(&self, character: char, font_family: &str, font_size: f32) -> Option<RasterizedGlyph> {
        let font = self.fonts.get(font_family)?;
        let (metrics, bitmap) = font.rasterize(character, font_size);

        Some(RasterizedGlyph {
            bitmap,
            width: metrics.width,
            height: metrics.height,
            xmin: metrics.xmin,
            ymin: metrics.ymin,
            advance_width: metrics.advance_width,
        })
    }
}

impl TextMeasurer for FontManager {
    fn measure(&mut self, text: &str, font_size: f32, font_family: &str) -> (f32, f32) {
        let Some(font) = self.get_font(font_family) else {
            // Fallback to box measurement if font not loaded
            let width = text.chars().count() as f32 * font_size * 0.6;
            return (width, font_size);
        };

        let mut width = 0.0;
        for c in text.chars() {
            let (metrics, _) = font.rasterize(c, font_size);
            width += metrics.advance_width;
        }

        let height = if let Some(metrics) = font.horizontal_line_metrics(font_size) {
            metrics.ascent - metrics.descent
        } else {
            font_size
        };

        (width, height)
    }

    fn ascent(&mut self, font_size: f32, font_family: &str) -> f32 {
        let Some(font) = self.get_font(font_family) else {
            return font_size * 0.8;
        };

        if let Some(metrics) = font.horizontal_line_metrics(font_size) {
            metrics.ascent
        } else {
            font_size * 0.8
        }
    }
}

/// Rasterized glyph data - backend agnostic
pub struct RasterizedGlyph {
    pub bitmap: Vec<u8>,      // Alpha values
    pub width: usize,
    pub height: usize,
    pub xmin: i32,
    pub ymin: i32,
    pub advance_width: f32,
}
