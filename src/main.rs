mod browser_window;
mod colors;
mod css;
mod debug;
mod html;
mod layout;
mod styles;
mod utils;
mod lisia_colors;

use std::string::String;
use browser_window::*;

fn main() {
    create_browser_window(String::from("index.html"));
}
