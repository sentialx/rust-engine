use crate::colors::ColorTupleA;
use crate::css_value::CssValue;
use crate::styles::{Style, StyleScalar, ScalarEvaluationContext};

/// A single box shadow
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedBoxShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
    pub spread_radius: f32,
    pub color: ColorTupleA,
    pub inset: bool,
}

impl ComputedBoxShadow {
    pub fn none() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            blur_radius: 0.0,
            spread_radius: 0.0,
            color: (0.0, 0.0, 0.0, 0.0),
            inset: false,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.color.3 > 0.0 && (self.blur_radius > 0.0 || self.spread_radius > 0.0 || self.offset_x != 0.0 || self.offset_y != 0.0)
    }
}

/// Box shadow property (can have multiple shadows, but we'll support one for now)
#[derive(Clone, Debug)]
pub struct BoxShadow {
    pub offset_x: StyleScalar,
    pub offset_y: StyleScalar,
    pub blur_radius: StyleScalar,
    pub spread_radius: StyleScalar,
    pub color: ColorTupleA,
    pub inset: bool,
    pub has_value: bool,
}

impl BoxShadow {
    pub fn empty() -> Self {
        Self {
            offset_x: StyleScalar::zero(),
            offset_y: StyleScalar::zero(),
            blur_radius: StyleScalar::zero(),
            spread_radius: StyleScalar::zero(),
            color: (0.0, 0.0, 0.0, 0.5), // default shadow color
            inset: false,
            has_value: false,
        }
    }

    pub fn evaluate(&mut self, ctx: &ScalarEvaluationContext) -> &Self {
        self.offset_x.evaluate(ctx);
        self.offset_y.evaluate(ctx);
        self.blur_radius.evaluate(ctx);
        self.spread_radius.evaluate(ctx);
        self
    }

    pub fn to_computed(&self) -> ComputedBoxShadow {
        if !self.has_value {
            return ComputedBoxShadow::none();
        }
        ComputedBoxShadow {
            offset_x: self.offset_x.get().unwrap_or(0.0),
            offset_y: self.offset_y.get().unwrap_or(0.0),
            blur_radius: self.blur_radius.get().unwrap_or(0.0).max(0.0),
            spread_radius: self.spread_radius.get().unwrap_or(0.0),
            color: self.color,
            inset: self.inset,
        }
    }

    pub fn create_inherited(&self, _inherit_style: &Style) -> BoxShadow {
        // box-shadow doesn't inherit
        self.clone()
    }

    /// Parse box-shadow from CSS value
    /// Syntax: offset-x offset-y [blur-radius [spread-radius]] [color] [inset]
    /// Example: 0 2px 8px rgba(0,0,0,0.15)
    pub fn from_value(&mut self, value: CssValue) {
        use crate::colors::hex_to_rgb;
        use crate::lisia_colors::match_named_color;

        let values = match value {
            CssValue::Multiple(v) => v,
            other => vec![other],
        };

        let mut sizes: Vec<CssValue> = Vec::new();
        let mut color: Option<ColorTupleA> = None;
        let mut inset = false;

        for v in values {
            match &v {
                CssValue::Size(_) | CssValue::Number(_) => {
                    sizes.push(v);
                }
                CssValue::String(s) => {
                    match s.as_str() {
                        "inset" => inset = true,
                        "none" => {
                            self.has_value = false;
                            return;
                        }
                        _ if s.starts_with("#") => {
                            if let Ok(c) = hex_to_rgb(s) {
                                color = Some((c.0, c.1, c.2, 1.0));
                            }
                        }
                        _ => {
                            if let Some(c) = match_named_color(s) {
                                color = Some((c[0], c[1], c[2], 1.0));
                            }
                        }
                    }
                }
                CssValue::Function(func) => {
                    match func.name.as_str() {
                        "rgba(" | "rgb(" => {
                            let mut comps: Vec<f32> = Vec::new();
                            for arg in &func.args {
                                if let CssValue::Number(n) = arg {
                                    comps.push(*n);
                                }
                            }
                            if comps.len() >= 3 {
                                let a = if comps.len() >= 4 { comps[3] } else { 1.0 };
                                color = Some((comps[0], comps[1], comps[2], a));
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        // Parse sizes: offset-x offset-y [blur-radius [spread-radius]]
        if sizes.len() >= 2 {
            self.offset_x = StyleScalar::new(sizes[0].clone());
            self.offset_y = StyleScalar::new(sizes[1].clone());
            self.has_value = true;
        }
        if sizes.len() >= 3 {
            self.blur_radius = StyleScalar::new(sizes[2].clone());
        }
        if sizes.len() >= 4 {
            self.spread_radius = StyleScalar::new(sizes[3].clone());
        }

        if let Some(c) = color {
            self.color = c;
        }
        self.inset = inset;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css_value::{CssSize, CssSizeUnit};

    #[test]
    fn test_box_shadow_simple() {
        let mut shadow = BoxShadow::empty();
        shadow.from_value(CssValue::Multiple(vec![
            CssValue::Size(CssSize { value: 0.0, unit: CssSizeUnit::Px }),
            CssValue::Size(CssSize { value: 2.0, unit: CssSizeUnit::Px }),
            CssValue::Size(CssSize { value: 8.0, unit: CssSizeUnit::Px }),
        ]));

        assert!(shadow.has_value);
        let ctx = ScalarEvaluationContext { percent_base: 100.0, em_base: 16.0, rem_base: 16.0 };
        shadow.evaluate(&ctx);

        let computed = shadow.to_computed();
        assert_eq!(computed.offset_x, 0.0);
        assert_eq!(computed.offset_y, 2.0);
        assert_eq!(computed.blur_radius, 8.0);
    }

    #[test]
    fn test_box_shadow_with_color() {
        let mut shadow = BoxShadow::empty();
        shadow.from_value(CssValue::Multiple(vec![
            CssValue::Size(CssSize { value: 2.0, unit: CssSizeUnit::Px }),
            CssValue::Size(CssSize { value: 4.0, unit: CssSizeUnit::Px }),
            CssValue::Size(CssSize { value: 6.0, unit: CssSizeUnit::Px }),
            CssValue::String("#000000".to_string()),
        ]));

        assert!(shadow.has_value);
        let ctx = ScalarEvaluationContext { percent_base: 100.0, em_base: 16.0, rem_base: 16.0 };
        shadow.evaluate(&ctx);

        let computed = shadow.to_computed();
        assert_eq!(computed.offset_x, 2.0);
        assert_eq!(computed.offset_y, 4.0);
        assert_eq!(computed.blur_radius, 6.0);
        assert_eq!(computed.color, (0.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn test_box_shadow_none() {
        let mut shadow = BoxShadow::empty();
        shadow.has_value = true; // pretend it had a value
        shadow.from_value(CssValue::Multiple(vec![
            CssValue::String("none".to_string()),
        ]));

        assert!(!shadow.has_value);
    }
}
