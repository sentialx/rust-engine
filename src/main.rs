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

use std::string::String;
use browser_window::*;

fn main() {
create_browser_window(String::from("index.html"));

    // selector_tests();
}

fn selector_tests() {
    // let selector = "div#id.class > a[href ~= 'xd'] , a[h] >  b +  div ~ c , cd > e > fg>hj";
    let selector = "   .fJhMgF.fJhMgF   x  >  d  ";
    // let selector = "cd > e > fg > hj, a";

    let tokens = css::tokenize_css_selector(selector);

    // println!("{:#?}", tokens);

    let parsed = css::parse_css_selector(&tokens);
    println!("{:#?}", parsed.to_string());

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