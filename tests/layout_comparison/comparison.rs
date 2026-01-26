use roxmltree::Document;

/// SVG rect element representation for layout comparison
#[derive(Debug, Clone)]
pub struct SvgRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub fill: String,
    pub class: String,
}

/// Parse SVG content and extract rect elements using roxmltree
pub fn parse_svg(svg_content: &str) -> Vec<SvgRect> {
    let doc = match Document::parse(svg_content) {
        Ok(doc) => doc,
        Err(_) => return Vec::new(),
    };

    doc.descendants()
        .filter(|node| node.has_tag_name("rect"))
        .filter_map(|node| {
            let fill = node.attribute("fill").unwrap_or("");

            // Skip white background and transparent rects
            if fill == "white" || fill.contains("rgba(0,0,0,0)") {
                return None;
            }

            // Parse numeric attributes, skip if percentage or missing
            let x = parse_numeric_attr(node.attribute("x"))?;
            let y = parse_numeric_attr(node.attribute("y"))?;
            let width = parse_numeric_attr(node.attribute("width"))?;
            let height = parse_numeric_attr(node.attribute("height"))?;

            Some(SvgRect {
                x,
                y,
                width,
                height,
                fill: fill.to_string(),
                class: node.attribute("data-class").unwrap_or("").to_string(),
            })
        })
        .collect()
}

fn parse_numeric_attr(value: Option<&str>) -> Option<f32> {
    let value = value?;
    // Skip percentage values
    if value.ends_with('%') {
        return None;
    }
    value.parse::<f32>().ok()
}

/// Comparison result for a single rect element
#[derive(Debug)]
pub struct RectDifference {
    pub index: usize,
    pub class: String,
    pub field: String,
    pub expected: f32,
    pub actual: f32,
    pub diff: f32,
}

/// Compare two sets of SVG rects with a given tolerance
/// Returns Ok(()) if they match within tolerance, or Err with list of differences
pub fn compare_svgs(
    baseline: &[SvgRect],
    actual: &[SvgRect],
    tolerance: f32,
) -> Result<(), Vec<RectDifference>> {
    let mut differences = Vec::new();

    // Check if counts match
    if baseline.len() != actual.len() {
        differences.push(RectDifference {
            index: 0,
            class: String::new(),
            field: "element_count".to_string(),
            expected: baseline.len() as f32,
            actual: actual.len() as f32,
            diff: (baseline.len() as f32 - actual.len() as f32).abs(),
        });
        return Err(differences);
    }

    for (i, (base, act)) in baseline.iter().zip(actual.iter()).enumerate() {
        let class = if !base.class.is_empty() {
            base.class.clone()
        } else {
            format!("element_{}", i)
        };

        // Compare x
        let x_diff = (base.x - act.x).abs();
        if x_diff > tolerance {
            differences.push(RectDifference {
                index: i,
                class: class.clone(),
                field: "x".to_string(),
                expected: base.x,
                actual: act.x,
                diff: x_diff,
            });
        }

        // Compare y
        let y_diff = (base.y - act.y).abs();
        if y_diff > tolerance {
            differences.push(RectDifference {
                index: i,
                class: class.clone(),
                field: "y".to_string(),
                expected: base.y,
                actual: act.y,
                diff: y_diff,
            });
        }

        // Compare width
        let width_diff = (base.width - act.width).abs();
        if width_diff > tolerance {
            differences.push(RectDifference {
                index: i,
                class: class.clone(),
                field: "width".to_string(),
                expected: base.width,
                actual: act.width,
                diff: width_diff,
            });
        }

        // Compare height
        let height_diff = (base.height - act.height).abs();
        if height_diff > tolerance {
            differences.push(RectDifference {
                index: i,
                class: class.clone(),
                field: "height".to_string(),
                expected: base.height,
                actual: act.height,
                diff: height_diff,
            });
        }
    }

    if differences.is_empty() {
        Ok(())
    } else {
        Err(differences)
    }
}

/// Format differences for display
pub fn format_differences(differences: &[RectDifference]) -> String {
    let mut output = String::new();

    for diff in differences {
        if diff.field == "element_count" {
            output.push_str(&format!(
                "Element count mismatch: expected {}, got {}\n",
                diff.expected as i32, diff.actual as i32
            ));
        } else {
            output.push_str(&format!(
                "[{}] {}: {} differs (expected {}, got {}, diff: {})\n",
                diff.index, diff.class, diff.field, diff.expected, diff.actual, diff.diff
            ));
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_svg() {
        let svg = r#"
<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600" viewBox="0 0 800 600">
  <rect x="0" y="0" width="800" height="600" fill="white"/>
  <rect x="18" y="18" width="240" height="140" fill="rgba(204,204,204,1)" data-class="box"/>
</svg>
"#;
        let rects = parse_svg(svg);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].x, 18.0);
        assert_eq!(rects[0].y, 18.0);
        assert_eq!(rects[0].width, 240.0);
        assert_eq!(rects[0].height, 140.0);
        assert_eq!(rects[0].class, "box");
    }

    #[test]
    fn test_compare_svgs_within_tolerance() {
        let baseline = vec![SvgRect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0,
            fill: "rgba(204,204,204,1)".to_string(),
            class: "test".to_string(),
        }];

        let actual = vec![SvgRect {
            x: 10.5,
            y: 20.5,
            width: 99.5,
            height: 50.5,
            fill: "rgba(204,204,204,1)".to_string(),
            class: "test".to_string(),
        }];

        // Should pass with 1px tolerance
        assert!(compare_svgs(&baseline, &actual, 1.0).is_ok());

        // Should fail with 0.1px tolerance
        assert!(compare_svgs(&baseline, &actual, 0.1).is_err());
    }

    #[test]
    fn test_compare_svgs_count_mismatch() {
        let baseline = vec![
            SvgRect {
                x: 10.0,
                y: 20.0,
                width: 100.0,
                height: 50.0,
                fill: "rgba(204,204,204,1)".to_string(),
                class: "test".to_string(),
            },
            SvgRect {
                x: 30.0,
                y: 40.0,
                width: 100.0,
                height: 50.0,
                fill: "rgba(204,204,204,1)".to_string(),
                class: "test2".to_string(),
            },
        ];

        let actual = vec![SvgRect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0,
            fill: "rgba(204,204,204,1)".to_string(),
            class: "test".to_string(),
        }];

        let result = compare_svgs(&baseline, &actual, 1.0);
        assert!(result.is_err());
        let diffs = result.unwrap_err();
        assert_eq!(diffs[0].field, "element_count");
    }
}
