// CSS domain handlers

use serde_json::{json, Value};

use crate::devtools_protocol::{DevtoolsServer, types::*};
use crate::frame::Frame;

/// Handle CSS domain commands
pub fn handle(
    server: &mut DevtoolsServer,
    _frame: &mut Frame,
    id: u64,
    command: &str,
    params: &Value,
) -> Response {
    match command {
        "enable" => {
            Response::success(id, json!({}))
        }

        "disable" => {
            Response::success(id, json!({}))
        }

        "getComputedStyleForNode" => {
            let node_id = match params.get("nodeId").and_then(|v| v.as_u64()) {
                Some(id) => id,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Missing nodeId parameter"),
            };

            let element = match server.get_element_by_id(node_id) {
                Some(el) => el,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Node not found"),
            };

            let el = element.borrow();
            let mut computed_style: Vec<CSSComputedStyleProperty> = Vec::new();

            if let Some(ref style) = el.computed_style {
                // Add all computed style properties
                computed_style.push(CSSComputedStyleProperty {
                    name: "display".to_string(),
                    value: style.display.clone(),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "position".to_string(),
                    value: style.position.clone(),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "float".to_string(),
                    value: style.float.clone(),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "visibility".to_string(),
                    value: style.visibility.clone(),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "white-space".to_string(),
                    value: style.white_space.clone(),
                });

                // Font properties
                computed_style.push(CSSComputedStyleProperty {
                    name: "font-family".to_string(),
                    value: style.font_family.clone(),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "font-size".to_string(),
                    value: format!("{}px", style.font_size),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "font-weight".to_string(),
                    value: style.font_weight.to_string(),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "font-style".to_string(),
                    value: style.font_style.clone(),
                });

                // Color properties
                computed_style.push(CSSComputedStyleProperty {
                    name: "color".to_string(),
                    value: format_color(style.color),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "background-color".to_string(),
                    value: format_color(style.background_color),
                });

                // Text decoration
                computed_style.push(CSSComputedStyleProperty {
                    name: "text-decoration".to_string(),
                    value: style.text_decoration.clone(),
                });

                // Box model - margin
                computed_style.push(CSSComputedStyleProperty {
                    name: "margin-top".to_string(),
                    value: format!("{}px", style.margin.top),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "margin-right".to_string(),
                    value: format!("{}px", style.margin.right),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "margin-bottom".to_string(),
                    value: format!("{}px", style.margin.bottom),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "margin-left".to_string(),
                    value: format!("{}px", style.margin.left),
                });

                // Box model - padding
                computed_style.push(CSSComputedStyleProperty {
                    name: "padding-top".to_string(),
                    value: format!("{}px", style.padding.top),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "padding-right".to_string(),
                    value: format!("{}px", style.padding.right),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "padding-bottom".to_string(),
                    value: format!("{}px", style.padding.bottom),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "padding-left".to_string(),
                    value: format!("{}px", style.padding.left),
                });

                // Border properties
                computed_style.push(CSSComputedStyleProperty {
                    name: "border-top-width".to_string(),
                    value: format!("{}px", style.border.top.width),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "border-top-style".to_string(),
                    value: style.border.top.style.clone(),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "border-top-color".to_string(),
                    value: format_color(style.border.top.color),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "border-right-width".to_string(),
                    value: format!("{}px", style.border.right.width),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "border-right-style".to_string(),
                    value: style.border.right.style.clone(),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "border-right-color".to_string(),
                    value: format_color(style.border.right.color),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "border-bottom-width".to_string(),
                    value: format!("{}px", style.border.bottom.width),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "border-bottom-style".to_string(),
                    value: style.border.bottom.style.clone(),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "border-bottom-color".to_string(),
                    value: format_color(style.border.bottom.color),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "border-left-width".to_string(),
                    value: format!("{}px", style.border.left.width),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "border-left-style".to_string(),
                    value: style.border.left.style.clone(),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "border-left-color".to_string(),
                    value: format_color(style.border.left.color),
                });

                // Sizing
                if style.width > 0.0 {
                    computed_style.push(CSSComputedStyleProperty {
                        name: "width".to_string(),
                        value: format!("{}px", style.width),
                    });
                }
                if style.height > 0.0 {
                    computed_style.push(CSSComputedStyleProperty {
                        name: "height".to_string(),
                        value: format!("{}px", style.height),
                    });
                }

                // Positioning
                if style.inset.top != 0.0 {
                    computed_style.push(CSSComputedStyleProperty {
                        name: "top".to_string(),
                        value: format!("{}px", style.inset.top),
                    });
                }
                if style.inset.right != 0.0 {
                    computed_style.push(CSSComputedStyleProperty {
                        name: "right".to_string(),
                        value: format!("{}px", style.inset.right),
                    });
                }
                if style.inset.bottom != 0.0 {
                    computed_style.push(CSSComputedStyleProperty {
                        name: "bottom".to_string(),
                        value: format!("{}px", style.inset.bottom),
                    });
                }
                if style.inset.left != 0.0 {
                    computed_style.push(CSSComputedStyleProperty {
                        name: "left".to_string(),
                        value: format!("{}px", style.inset.left),
                    });
                }

                // Box shadow
                if style.box_shadow.blur_radius > 0.0 || style.box_shadow.spread_radius > 0.0
                    || style.box_shadow.offset_x != 0.0 || style.box_shadow.offset_y != 0.0 {
                    computed_style.push(CSSComputedStyleProperty {
                        name: "box-shadow".to_string(),
                        value: format!(
                            "{}px {}px {}px {}px {}{}",
                            style.box_shadow.offset_x,
                            style.box_shadow.offset_y,
                            style.box_shadow.blur_radius,
                            style.box_shadow.spread_radius,
                            format_color(style.box_shadow.color),
                            if style.box_shadow.inset { " inset" } else { "" }
                        ),
                    });
                }
            }

            // Add layout dimensions if available
            if let Some(ref flow) = el.computed_flow {
                computed_style.push(CSSComputedStyleProperty {
                    name: "x".to_string(),
                    value: format!("{}px", flow.x),
                });
                computed_style.push(CSSComputedStyleProperty {
                    name: "y".to_string(),
                    value: format!("{}px", flow.y),
                });
            }

            Response::success(id, json!({ "computedStyle": computed_style }))
        }

        "getMatchedStylesForNode" => {
            let node_id = match params.get("nodeId").and_then(|v| v.as_u64()) {
                Some(id) => id,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Missing nodeId parameter"),
            };

            let element = match server.get_element_by_id(node_id) {
                Some(el) => el,
                None => return Response::error(id, ERROR_INVALID_PARAMS, "Node not found"),
            };

            let el = element.borrow();
            let mut matched_rules: Vec<RuleMatch> = Vec::new();

            // Get matched CSS rules from the element
            for rule in &el.matched_styles {
                let css_properties: Vec<CSSProperty> = rule.declarations
                    .iter()
                    .map(|decl| CSSProperty {
                        name: decl.key.clone(),
                        value: format!("{:?}", decl.value),
                    })
                    .collect();

                matched_rules.push(RuleMatch {
                    rule: CSSRule {
                        style_sheet_id: None,
                        selector_list: SelectorList {
                            selectors: vec![SelectorData {
                                text: rule.selector.to_string(),
                            }],
                            text: rule.selector.to_string(),
                        },
                        style: CSSStyle {
                            css_properties,
                            shorthand_entries: vec![],
                        },
                    },
                    matching_selectors: vec![0],
                });
            }

            // Also include inline styles if present
            let inline_style = if !el.attributes.get("style").map(|s| s.is_empty()).unwrap_or(true) {
                Some(CSSStyle {
                    css_properties: vec![CSSProperty {
                        name: "style".to_string(),
                        value: el.attributes.get("style").cloned().unwrap_or_default(),
                    }],
                    shorthand_entries: vec![],
                })
            } else {
                None
            };

            Response::success(id, json!({
                "matchedCSSRules": matched_rules,
                "inlineStyle": inline_style
            }))
        }

        _ => Response::error(id, ERROR_METHOD_NOT_FOUND, &format!("Unknown CSS method: {}", command)),
    }
}

/// Format a color tuple as rgba() string
fn format_color(color: (f32, f32, f32, f32)) -> String {
    format!("rgba({}, {}, {}, {})", color.0 as u8, color.1 as u8, color.2 as u8, color.3)
}
