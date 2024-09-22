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
            value: CssValue::String(value.to_string()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CssSelectorToken {
    pub token_type: CssSelectorTokenType,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CssSelectorTokenType {
    Tag,
    Id,
    Class,
    Attribute,
    AttributeValue,
    AttributeOperator,
    Pseudo,
    PseudoElement,
    Separator,
    Combinator,
    End,
}
// div#id.class > a[href~='xd'] > b + div ~ c

pub fn tokenize_css_selector(input: &str) -> Vec<CssSelectorToken> {
    let mut tokens: Vec<CssSelectorToken> = vec![];
    let mut chars = input.trim().chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            '#' => {
                chars.next();
                let value = collect_while(&mut chars, |c| c.is_alphanumeric() || c == '-');
                tokens.push(CssSelectorToken {
                    token_type: CssSelectorTokenType::Id,
                    value,
                });
            }
            '.' => {
                chars.next();
                let value = collect_while(&mut chars, |c| c.is_alphanumeric() || c == '-');
                tokens.push(CssSelectorToken {
                    token_type: CssSelectorTokenType::Class,
                    value,
                });
            }
            '[' => {
                chars.next();
                let attribute = collect_while(&mut chars, |c| c != ']' && c != '=' && c != '~' && c != '*' && c != '|' && c != '$' && c != '^');
                tokens.push(CssSelectorToken {
                    token_type: CssSelectorTokenType::Attribute,
                    value: attribute.trim().to_string(),
                });

                if let Some(&next_ch) = chars.peek() {
                    if next_ch == '=' || next_ch == '~' || next_ch == '*' || next_ch == '|' || next_ch == '$' || next_ch == '^' {
                        let operator = chars.next().unwrap().to_string();
                        tokens.push(CssSelectorToken {
                            token_type: CssSelectorTokenType::AttributeOperator,
                            value: operator,
                        });

                        if let Some('=') = chars.peek() {
                            chars.next();
                            let value = collect_while(&mut chars, |c| c != ']').trim().to_string();
                            // Strip the quotes from the attribute value
                            let value = value.trim_matches(|c| c == '\'' || c == '\"').to_string();
                            tokens.push(CssSelectorToken {
                                token_type: CssSelectorTokenType::AttributeValue,
                                value: value,
                            });
                        }
                    }
                }

                if let Some(']') = chars.next() {
                    // End of attribute selector
                }
            }
            ':' => {
                chars.next();
                if let Some(':') = chars.peek() {
                    chars.next();
                    let value = collect_while(&mut chars, |c| c.is_alphanumeric() || c == '-');
                    tokens.push(CssSelectorToken {
                        token_type: CssSelectorTokenType::PseudoElement,
                        value,
                    });
                } else {
                    let value = collect_while(&mut chars, |c| c.is_alphanumeric() || c == '-');
                    tokens.push(CssSelectorToken {
                        token_type: CssSelectorTokenType::Pseudo,
                        value,
                    });
                }
            }
            '>' | '+' | '~' => {
                if tokens.last().is_some() && matches!(tokens.last().unwrap().token_type, CssSelectorTokenType::Combinator) {
                    tokens.pop();
                }

                let combinator = chars.next().unwrap().to_string();
                tokens.push(CssSelectorToken {
                    token_type: CssSelectorTokenType::Combinator,
                    value: combinator,
                });
            }
            ',' => {
                chars.next();
                tokens.push(CssSelectorToken {
                    token_type: CssSelectorTokenType::Separator,
                    value: ",".to_string(),
                });
            }
            '*' => {
              chars.next();
              tokens.push(CssSelectorToken {
                token_type: CssSelectorTokenType::Tag,
                value: "*".to_string(),
              });
            }
            _ if ch.is_alphanumeric() || ch == '_' || ch == '-' => {
                let value = collect_while(&mut chars, |c| c.is_alphanumeric() || c == '-' || c == '_');
                tokens.push(CssSelectorToken {
                    token_type: CssSelectorTokenType::Tag,
                    value,
                });
            }
            _ => {
                chars.next();
            },
        }

        if let Some(token) = tokens.last() {
          let last_char = chars.peek();
          if  last_char.is_some() && last_char.unwrap() == &' ' {
            if !matches!(token.token_type, CssSelectorTokenType::Combinator) && !matches!(token.token_type, CssSelectorTokenType::Separator) {
              tokens.push(CssSelectorToken {
                token_type: CssSelectorTokenType::Combinator,
                value: " ".to_string(),
              });
            }
          }
        }
    }

    tokens.push(CssSelectorToken {
        token_type: CssSelectorTokenType::End,
        value: "".to_string(),
    });

    tokens
}

fn collect_while<F>(chars: &mut std::iter::Peekable<std::str::Chars>, condition: F) -> String
where
    F: Fn(char) -> bool,
{
    let mut result = String::new();
    while let Some(&ch) = chars.peek() {
        if condition(ch) {
            result.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    result
}

#[derive(Debug, Clone)]
pub enum CssSelector {
    Tag(String),
    Id(String),
    Class(String),
    Attribute {
        name: String,
        operator: Option<String>,
        value: Option<String>,
    },
    PseudoClass(String),
    PseudoElement(String),
    Combinator {
        combinator: String,
        selectors: Vec<CssSelector>,
    },
    OrGroup {
        selectors: Vec<CssSelector>,
    },
    AndGroup {
        selectors: Vec<CssSelector>,
    },
}

impl CssSelector {
  pub fn to_string(&self) -> String {
    match self {
      CssSelector::Tag(tag) => tag.clone(),
      CssSelector::Id(id) => format!("#{}", id),
      CssSelector::Class(class) => format!(".{}", class),
      CssSelector::Attribute { name, operator, value } => {
        let def = "".to_string();
        let operator = operator.as_ref().unwrap_or(&def);
        let value = value.as_ref().unwrap_or(&def);
        format!("[{}{}{}]", name, operator, value)
      },
      CssSelector::PseudoClass(pseudo) => format!(":{}", pseudo),
      CssSelector::PseudoElement(pseudo) => format!("::{}", pseudo),
      CssSelector::Combinator { combinator, selectors } => {
        if is_parent_combinator(combinator) {
          // reversed tree
        //   println!("{:#?}", self);
          let selectors = selectors.iter().map(|s| s.to_string()).rev().collect::<Vec<String>>();
          // insert combinators between selectors
          let selectors = selectors.iter().enumerate().map(|(i, s)| {
            if i == 0 {
              s.to_string()
            } else {
              format!("{}{}", s, combinator)
            }
          }).collect::<Vec<String>>().join("");
          format!("{}", selectors)
        } else {
          let selectors = selectors.iter().map(|s| s.to_string()).collect::<Vec<String>>().join(&combinator);
          format!("{}", selectors)
        }
      },
      CssSelector::OrGroup { selectors } => {
        let selectors = selectors.iter().map(|s| s.to_string()).collect::<Vec<String>>().join(", ");
        format!("{}", selectors)
      },
      CssSelector::AndGroup { selectors } => {
        let selectors = selectors.iter().map(|s| s.to_string()).collect::<Vec<String>>().join("");
        format!("{}", selectors)
      },
    }
  }
}

fn add_selector_to_ctx(ctx: &mut CssSelector, selector: CssSelector) {
  match ctx {
      CssSelector::OrGroup { selectors } => {
          selectors.push(selector);
      }
      CssSelector::AndGroup { selectors } => {
          selectors.push(selector);
      }
      CssSelector::Combinator { selectors, .. } => {
        selectors.push(selector);
    }
      _ => {}
  }
}

fn add_selector_to_current_ctx(contexts: &mut Vec<CssSelector>, selector: CssSelector) {
    let ctx = contexts.last_mut().unwrap();
    add_selector_to_ctx(ctx, selector);
}

pub fn wrap_ctx_selectors_into_new_ctx(contexts: &mut Vec<CssSelector>, new_ctx: CssSelector) {
    let ctx = contexts.last_mut().unwrap();
    let mut new_ctx = new_ctx;
    match ctx {
      CssSelector::AndGroup { selectors } => {
        while let Some(selector) = selectors.pop() {
          add_selector_to_ctx(&mut new_ctx, selector);
        }
        selectors.push(new_ctx);
      },
      CssSelector::Combinator { combinator, selectors } => {
        let ctx = contexts.pop().unwrap();
        add_selector_to_ctx(&mut new_ctx, ctx);
      },
      _ => unreachable!()
    }
}

pub fn is_parent_combinator(combinator: &str) -> bool {
    return combinator == ">" || combinator == " ";
}

pub fn parse_css_selector(tokens: &[CssSelectorToken]) -> CssSelector {
    let mut tokens = tokens.iter().peekable();
    let mut contexts = vec![CssSelector::AndGroup { selectors: vec![] }];

    while let Some(token) = tokens.next() {
        match &token.token_type {
            CssSelectorTokenType::Tag => {
                add_selector_to_current_ctx(&mut contexts, CssSelector::Tag(token.value.clone()));
            }
            CssSelectorTokenType::Id => {
                add_selector_to_current_ctx(&mut contexts, CssSelector::Id(token.value.clone()));
            }
            CssSelectorTokenType::Class => {
                add_selector_to_current_ctx(&mut contexts, CssSelector::Class(token.value.clone()));
            }
            CssSelectorTokenType::Attribute => {
                let mut operator = None;
                let mut value = None;

                if let Some(op) = tokens.peek() {
                    if matches!(op.token_type, CssSelectorTokenType::AttributeOperator) {
                        operator = Some(op.value.clone());
                    }
                }

                if operator.is_some() {
                    tokens.next(); // Consume AttributeOperator
                }

                if let Some(val) = tokens.peek() {
                    if matches!(val.token_type, CssSelectorTokenType::AttributeValue) {
                        value = Some(val.value.clone());
                    }
                }

                if value.is_some() {
                    tokens.next(); // Consume AttributeValue
                }

                add_selector_to_current_ctx(
                    &mut contexts,
                    CssSelector::Attribute {
                        name: token.value.clone(),
                        operator,
                        value,
                    },
                );
            }
            CssSelectorTokenType::Pseudo => {
                add_selector_to_current_ctx(
                    &mut contexts,
                    CssSelector::PseudoClass(token.value.clone()),
                );
            }
            CssSelectorTokenType::PseudoElement => {
                add_selector_to_current_ctx(
                    &mut contexts,
                    CssSelector::PseudoElement(token.value.clone()),
                );
            }
            CssSelectorTokenType::Combinator => {
                let combinator = CssSelector::Combinator {
                    combinator: token.value.clone(),
                    selectors: vec![],
                };
    
                if is_parent_combinator(&token.value) {
                  wrap_ctx_selectors_into_new_ctx(&mut contexts, combinator);
                } else {
                  contexts.push(combinator);
                }
            }
            CssSelectorTokenType::Separator => {
              // close all contexts
              for i in 1..contexts.len() - 1 {
                let ctx = contexts.pop().unwrap();
                add_selector_to_current_ctx(&mut contexts, ctx);
              }
              
              if contexts.len() > 1 {
                let ctx = contexts.pop().unwrap();
                add_selector_to_current_ctx(&mut contexts, ctx);
            }

            let ctx = contexts.first_mut().unwrap();

                // check if there already is an OrGroup
                match ctx {
                  // Ignore if there is already an OrGroup
                  CssSelector::OrGroup { .. } => {}
                  _ => {
                    let ctx = contexts.pop().unwrap();
                    contexts.push(CssSelector::OrGroup { selectors: vec![ctx] });
                  }
                }
              
              contexts.push(CssSelector::AndGroup { selectors: vec![] });
            }
            _ => {}
        }
    }

    // close all contexts
    for _ in 1..contexts.len() {
      let ctx = contexts.pop().unwrap();
      add_selector_to_current_ctx(&mut contexts, ctx);
    }

    let ctx = contexts.pop().unwrap();

    return ctx;
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

    let mut inside_media_query = false;
    let mut query_body_count = 0;

    let mut escape = false;

    let mut no_effect_media_queries = 0;
    let mut inside_rule = false;

    for (i, c) in chars {
        if inside_media_query {
            if c == '}' {
                if query_body_count > 0 {
                    query_body_count -= 1;
                    captured_text = "".to_string();
                    captured_code = "".to_string();
                    continue;
                }

                if query_body_count <= 0 {
                    inside_media_query = false;
                    captured_text = "".to_string();
                    captured_code = "".to_string();
                    is_capturing_selector = true;
                    query_body_count = 0;
                    continue;
                }
            } else if c == '{' {
                query_body_count += 1;
            }
            continue;
        }

        if c == '}' && no_effect_media_queries > 0 && !inside_rule {
            no_effect_media_queries -= 1;
            captured_text = "".to_string();
            captured_code = "".to_string();
            continue;
        }

        if c == '"' || c == '\'' {
            if inside_quotes == "" {
                inside_quotes = c.to_string();
            } else if inside_quotes == c.to_string() && !escape {
                inside_quotes = "".to_string();
            }
        }

        if c == '\\' {
            escape = true;
        } else {
            escape = false;
        }

        if inside_quotes != "" {
            captured_text.push(c);
            continue;
        }

        if (c == '/' || c == '*') && captured_code.ends_with("/") && !inside_comment {
            inside_comment = true;
        } else if inside_comment {
            if (c == '/' && captured_code.ends_with("*")) {
                inside_comment = false;
                captured_code = "".to_string();
                continue;
            }
        }

        captured_code.push(c);

        if inside_comment {
            continue;
        }

        if c == '/' {
            continue;
        }

        if c == '{' {
            if captured_code.trim().starts_with("@") {
                if captured_code.trim() == "@media screen {" {
                    no_effect_media_queries += 1;
                } else {
                    inside_media_query = true;
                    is_capturing_selector = false;
                    query_body_count = 0;
                }
                captured_text = "".to_string();
                captured_code = "".to_string();
                continue;
            }
            inside_rule = true;
            style_rule.selector = parse_css_selector(&tokenize_css_selector(&captured_text));
            captured_text = "".to_string();
            is_capturing_selector = false;
        } else if c == ':' && !is_capturing_selector {
            declaration.key = captured_text.trim().to_string();
            captured_text = "".to_string();
        } else if (c == ';' || c == '}') {
            if is_capturing_selector {
                println!("Error: Unexpected character: {:?} at position: {:?}", c, i);
            }
            if (declaration.key != "") {
                let mut text = captured_text.trim().to_string();
                let important = text.ends_with("!important");
                if important {
                    text = text.replace("!important", "").trim().to_string();
                }
                // println!("tokens: {:?} {:?} {:#?}", declaration.key, text, tokenize_css_value(&text));
                style_rule.declarations.push(Declaration {
                    key: declaration.key.clone(),
                    value:  parse_css_value(tokenize_css_value(&text)),
                    // value: CssValue::String(text),
                    important,
                });
                declaration = CssKeyValue::empty();
                captured_text = "".to_string();
            }

            if c == '}' {
                style_rule.css = captured_code.trim().to_string();
                list.push(style_rule);
                style_rule = StyleRule::new();
                inside_rule = false;
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
