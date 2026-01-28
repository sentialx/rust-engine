use crate::colors::ColorTupleA;
use crate::css_value::CssValue;
use crate::styles::{PropertyImpl, ScalarEvaluationContext, Style, StyleScalar};
use crate::properties::color::Color;

/// Computed border for a single side
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedBorderSide {
    pub width: f32,
    pub style: String,
    pub color: ColorTupleA,
}

impl ComputedBorderSide {
    pub fn none() -> Self {
        Self {
            width: 0.0,
            style: "none".to_string(),
            color: (0.0, 0.0, 0.0, 1.0),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.width > 0.0 && self.style != "none" && self.style != "hidden"
    }

    pub fn is_solid(&self) -> bool {
        self.is_visible() && (self.style == "solid" || self.style.is_empty())
    }
}

/// Computed border for all four sides
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedBorder {
    pub top: ComputedBorderSide,
    pub right: ComputedBorderSide,
    pub bottom: ComputedBorderSide,
    pub left: ComputedBorderSide,
}

impl ComputedBorder {
    pub fn none() -> Self {
        Self {
            top: ComputedBorderSide::none(),
            right: ComputedBorderSide::none(),
            bottom: ComputedBorderSide::none(),
            left: ComputedBorderSide::none(),
        }
    }

    pub fn has_visible_border(&self) -> bool {
        self.top.is_visible() || self.right.is_visible() ||
        self.bottom.is_visible() || self.left.is_visible()
    }
}

/// Border side (width, style, color)
#[derive(Clone, Debug)]
pub struct BorderSide {
    pub width: StyleScalar,
    pub style: String,
    pub color: Color,
}

impl BorderSide {
    pub fn empty() -> Self {
        Self {
            width: StyleScalar::zero(),
            style: "none".to_string(),
            color: Color::empty(false, (0.0, 0.0, 0.0, 1.0)),
        }
    }

    pub fn evaluate(&mut self, ctx: &ScalarEvaluationContext) -> &Self {
        self.width.evaluate(ctx);
        self
    }

    pub fn to_computed(&self) -> ComputedBorderSide {
        ComputedBorderSide {
            width: if self.style == "none" || self.style == "hidden" {
                0.0
            } else {
                self.width.get().unwrap_or(0.0)
            },
            style: self.style.clone(),
            color: self.color.get(),
        }
    }

    pub fn from_width_value(value: CssValue) -> Self {
        let mut side = BorderSide::empty();
        match &value {
            CssValue::Size(_) | CssValue::Number(_) => {
                side.width = StyleScalar::new(value);
                side.style = "solid".to_string();
            }
            CssValue::Multiple(values) => {
                if let Some(v) = values.first() {
                    match v {
                        CssValue::Size(_) | CssValue::Number(_) => {
                            side.width = StyleScalar::new(v.clone());
                            side.style = "solid".to_string();
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        side
    }

    pub fn from_shorthand(value: CssValue) -> Self {
        let mut side = BorderSide::empty();

        match value {
            CssValue::Multiple(values) => {
                for v in values {
                    match &v {
                        CssValue::Size(_) | CssValue::Number(_) => {
                            side.width = StyleScalar::new(v.clone());
                        }
                        CssValue::String(s) => {
                            match s.as_str() {
                                "none" | "hidden" | "solid" | "dashed" | "dotted" |
                                "double" | "groove" | "ridge" | "inset" | "outset" => {
                                    side.style = s.clone();
                                }
                                _ => {
                                    // Try as color
                                    side.color.from_value(CssValue::Multiple(vec![v.clone()]));
                                }
                            }
                        }
                        CssValue::Function(_) => {
                            // Color function like rgb(), rgba()
                            side.color.from_value(CssValue::Multiple(vec![v.clone()]));
                        }
                        _ => {}
                    }
                }
                // If width was set but no style, default to solid
                if side.width.get().unwrap_or(0.0) > 0.0 && side.style == "none" {
                    side.style = "solid".to_string();
                }
            }
            _ => {}
        }

        side
    }

    pub fn create_inherited(&self, _inherit_style: &Style) -> BorderSide {
        // Borders don't inherit
        self.clone()
    }
}

/// Border for all four sides
#[derive(Clone, Debug)]
pub struct Border {
    pub top: BorderSide,
    pub right: BorderSide,
    pub bottom: BorderSide,
    pub left: BorderSide,
}

impl Border {
    pub fn empty() -> Self {
        Self {
            top: BorderSide::empty(),
            right: BorderSide::empty(),
            bottom: BorderSide::empty(),
            left: BorderSide::empty(),
        }
    }

    pub fn evaluate(&mut self, ctx: &ScalarEvaluationContext) -> &Self {
        self.top.evaluate(ctx);
        self.right.evaluate(ctx);
        self.bottom.evaluate(ctx);
        self.left.evaluate(ctx);
        self
    }

    pub fn to_computed(&self) -> ComputedBorder {
        ComputedBorder {
            top: self.top.to_computed(),
            right: self.right.to_computed(),
            bottom: self.bottom.to_computed(),
            left: self.left.to_computed(),
        }
    }

    pub fn from_shorthand(value: CssValue) -> Self {
        let side = BorderSide::from_shorthand(value);
        Self {
            top: side.clone(),
            right: side.clone(),
            bottom: side.clone(),
            left: side,
        }
    }
}

impl PropertyImpl for Border {
    fn create_inherited(&self, _inherit_style: &Style) -> Border {
        // Borders don't inherit
        self.clone()
    }

    fn from_value(value: CssValue) -> Border {
        Border::from_shorthand(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css_value::{CssSize, CssSizeUnit};

    #[test]
    fn test_border_side_from_shorthand_simple() {
        // Test "2px solid #333"
        let value = CssValue::Multiple(vec![
            CssValue::Size(CssSize { value: 2.0, unit: CssSizeUnit::Px }),
            CssValue::String("solid".to_string()),
            CssValue::String("#333333".to_string()),
        ]);
        let side = BorderSide::from_shorthand(value);

        assert_eq!(side.style, "solid");
        // Width needs to be evaluated to get the value
        let mut side = side;
        let ctx = ScalarEvaluationContext { percent_base: 100.0, em_base: 16.0, rem_base: 16.0 };
        side.evaluate(&ctx);
        assert_eq!(side.width.get(), Some(2.0));
    }

    #[test]
    fn test_border_side_from_width_value() {
        let value = CssValue::Size(CssSize { value: 5.0, unit: CssSizeUnit::Px });
        let mut side = BorderSide::from_width_value(value);

        assert_eq!(side.style, "solid");
        let ctx = ScalarEvaluationContext { percent_base: 100.0, em_base: 16.0, rem_base: 16.0 };
        side.evaluate(&ctx);
        assert_eq!(side.width.get(), Some(5.0));
    }

    #[test]
    fn test_border_side_from_width_value_multiple() {
        let value = CssValue::Multiple(vec![
            CssValue::Size(CssSize { value: 3.0, unit: CssSizeUnit::Px }),
        ]);
        let mut side = BorderSide::from_width_value(value);

        assert_eq!(side.style, "solid");
        let ctx = ScalarEvaluationContext { percent_base: 100.0, em_base: 16.0, rem_base: 16.0 };
        side.evaluate(&ctx);
        assert_eq!(side.width.get(), Some(3.0));
    }

    #[test]
    fn test_computed_border_is_visible() {
        let side = ComputedBorderSide {
            width: 2.0,
            style: "solid".to_string(),
            color: (0.0, 0.0, 0.0, 1.0),
        };
        assert!(side.is_visible());

        let side_none = ComputedBorderSide {
            width: 2.0,
            style: "none".to_string(),
            color: (0.0, 0.0, 0.0, 1.0),
        };
        assert!(!side_none.is_visible());

        let side_zero = ComputedBorderSide {
            width: 0.0,
            style: "solid".to_string(),
            color: (0.0, 0.0, 0.0, 1.0),
        };
        assert!(!side_zero.is_visible());
    }

    #[test]
    fn test_border_from_shorthand() {
        let value = CssValue::Multiple(vec![
            CssValue::Size(CssSize { value: 1.0, unit: CssSizeUnit::Px }),
            CssValue::String("solid".to_string()),
            CssValue::String("#000000".to_string()),
        ]);
        let border = Border::from_shorthand(value);

        assert_eq!(border.top.style, "solid");
        assert_eq!(border.right.style, "solid");
        assert_eq!(border.bottom.style, "solid");
        assert_eq!(border.left.style, "solid");
    }
}
