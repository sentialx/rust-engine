use crate::colors::*;
use crate::css_value::parse_css_value;
use crate::css_value::tokenize_css_value;
use crate::css_value::CssValue;
use crate::styles::*;
use crate::utils::*;

#[derive(Clone, Debug)]
pub struct CssKeyValue {
    pub key: String,
    pub value: CssValue,
}

impl CssKeyValue {
    pub fn empty() -> CssKeyValue {
        CssKeyValue::new("", "")
    }

    pub fn new(key: &str, value: &str) -> CssKeyValue {
        CssKeyValue {
            key: key.to_string(),
            value: CssValue::String(value.to_string())
        }
    }
}

pub fn parse_css(css: &str) -> Vec<StyleRule> {
    let mut list: Vec<StyleRule> = vec![];

    let mut captured_text = "".to_string();
    let mut captured_code = "".to_string();

    let mut style_rule = StyleRule::new();
    let mut declaration = CssKeyValue::empty();

    let mut is_capturing_selector = true;
    let mut inside_comment = false;

    let mut inside_quotes = "".to_string();

    let chars = css.chars().enumerate();

    for (i, c) in chars {
        if (c == '/' || c == '*') && captured_code.ends_with("/") && !inside_comment {
            inside_comment = true;
        } else if inside_comment {
            if (c == '/' && captured_code.ends_with("*")) {
                inside_comment = false;
                captured_code = "".to_string();
                continue;
            }
        }

        if c == '"' || c == '\'' {
            if inside_quotes == "" {
                inside_quotes = c.to_string();
            } else if inside_quotes == c.to_string() {
                inside_quotes = "".to_string();
            }
        }

        captured_code.push(c);

        if inside_quotes != "" {
            captured_text.push(c);
            continue;
        }

        if inside_comment {
            continue;
        }

        if c == '/' {
            continue;
        }

        if c == '{' {
            style_rule.selector = captured_text.trim().to_string();
            captured_text = "".to_string();
            is_capturing_selector = false;
        } else if c == ':' && !is_capturing_selector {
            declaration.key = captured_text.trim().to_string();
            captured_text = "".to_string();
        } else if c == ';' || c == '}' {
            if (declaration.key != "") {
                let mut text = captured_text.trim().to_string();
                let important =  text.ends_with("!important");
                if important {
                    text = text.replace("!important", "").trim().to_string();
                }
                // println!("tokens: {:?} {:?} {:#?}", declaration.key, text, tokenize_css_value(&text));
                style_rule.declarations.push(Declaration {
                    key: declaration.key.clone(),
                    value: parse_css_value(tokenize_css_value(&text)),
                    important,
                });
                declaration = CssKeyValue::empty();
                captured_text = "".to_string();
            }

            if c == '}' {
                style_rule.css = captured_code.trim().to_string();
                list.push(style_rule);
                style_rule = StyleRule::new();
                captured_code = "".to_string();
                captured_text = "".to_string();
                is_capturing_selector = true;
            }
        } else {
            captured_text.push(c);
        }
    }

    return list;
}
