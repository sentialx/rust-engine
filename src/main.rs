mod browser_window;
mod colors;
mod css;
mod debug;
mod html;
mod layout;
mod styles;
mod utils;

use browser_window::*;

fn main() {
    let mut window = BrowserWindow::create();
    window.load_file("index.html");
    while true {}
}
