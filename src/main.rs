mod browser_window;
mod colors;
mod css;
mod debug;
mod html;
mod layout;
mod styles;
mod utils;
mod lisia_colors;
mod css_value;
mod properties;
mod render_frame;

use std::{fs, string::String};
use browser_window::*;
use css::parse_css;
use css_value::{parse_css_value, tokenize_css_value};

fn main() {
create_browser_window(String::from("index.html"));

    let val = "rgba(255, 255, 255, 0.4)";
    let parsed = parse_css_value(tokenize_css_value(val));

    println!("{:#?}", parsed);

// let style = fs::read_to_string("style.css").expect("error while reading the file");
// let parsed_css = parse_css(&style);

// for rule in parsed_css {
//     println!("{:#?}", rule.selector.to_string());
//     // println!("{:#?}", rule.declarations);
// }
// println!("{:#?}", parsed_css);

    // selector_tests();
}

fn selector_tests() {
    let selector = "span.enum,
a.enum,
span.struct,
a.struct,
span.union,
a.union,
span.primitive,
a.primitive,
span.type,
a.type,
span.foreigntype,
a.foreigntype";
    // let selector = "   .fJhMgF.fJhMgF   x  >  d  ";
    // let selector = "cd > e > fg > hj, a";

    let tokens = css::tokenize_css_selector(selector);

    // println!("{:#?}", tokens);

    let parsed = css::parse_css_selector(&tokens);
    println!("{:#?}", parsed);

    assert_eq!(tokens, [
        css::CssSelectorToken {
            token_type: css::CssSelectorTokenType::Tag,
            value: "div".to_string(),
        },
        css::CssSelectorToken {
            token_type: css::CssSelectorTokenType::Id,
            value: "id".to_string(),
        },
        css::CssSelectorToken {
            token_type: css::CssSelectorTokenType::Class,
            value: "class".to_string(),
        },
        css::CssSelectorToken {
            token_type: css::CssSelectorTokenType::Combinator,
            value: ">".to_string(),
        },
        css::CssSelectorToken {
            token_type: css::CssSelectorTokenType::Tag,
            value: "a".to_string(),
        },
        css::CssSelectorToken {
            token_type: css::CssSelectorTokenType::Attribute,
            value: "href".to_string(),
        },
        css::CssSelectorToken {
            token_type: css::CssSelectorTokenType::AttributeOperator,
            value: "~".to_string(),
        },
        css::CssSelectorToken {
            token_type: css::CssSelectorTokenType::AttributeValue,
            value: "xd".to_string(),
        },
        css::CssSelectorToken {
            token_type: css::CssSelectorTokenType::Separator,
            value: ",".to_string(),
        },
        css::CssSelectorToken {
            token_type: css::CssSelectorTokenType::Combinator,
            value: ">".to_string(),
        },
        css::CssSelectorToken {
            token_type: css::CssSelectorTokenType::Tag,
            value: "b".to_string(),
        },
        css::CssSelectorToken {
            token_type: css::CssSelectorTokenType::Combinator,
            value: "+".to_string(),
        },
        css::CssSelectorToken {
            token_type: css::CssSelectorTokenType::Tag,
            value: "div".to_string(),
        },
        css::CssSelectorToken {
            token_type: css::CssSelectorTokenType::Combinator,
            value: "~".to_string(),
        },
        css::CssSelectorToken {
            token_type: css::CssSelectorTokenType::Tag,
            value: "c".to_string(),
        },
        css::CssSelectorToken {
            token_type: css::CssSelectorTokenType::End,
            value: "".to_string(),
        },
    ]);
}