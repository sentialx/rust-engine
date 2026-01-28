// DevTools Agent - standalone binary for AI integration
//
// Usage:
//   cargo run --bin devtools_agent -- <html_file> [width] [height]
//
// Example:
//   cargo run --bin devtools_agent -- page.html 1366 768
//
// Reads JSON CDP commands from stdin, writes responses to stdout.

use std::env;
use std::process;

use graviton::devtools_protocol::DevtoolsServer;
use graviton::frame::Frame;
use graviton::layout::Size;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <html_file> [width] [height]", args[0]);
        eprintln!("");
        eprintln!("DevTools Agent - CDP-compatible protocol for AI integration");
        eprintln!("");
        eprintln!("Arguments:");
        eprintln!("  html_file    Path to HTML file to load");
        eprintln!("  width        Viewport width in pixels (default: 1366)");
        eprintln!("  height       Viewport height in pixels (default: 768)");
        eprintln!("");
        eprintln!("The agent reads JSON CDP commands from stdin and writes responses to stdout.");
        eprintln!("");
        eprintln!("Example commands:");
        eprintln!(r#"  {{"id":1,"method":"DOM.getDocument","params":{{"depth":2}}}}"#);
        eprintln!(r#"  {{"id":2,"method":"DOM.querySelector","params":{{"nodeId":1,"selector":".my-class"}}}}"#);
        eprintln!(r#"  {{"id":3,"method":"CSS.getComputedStyleForNode","params":{{"nodeId":5}}}}"#);
        eprintln!(r#"  {{"id":4,"method":"Page.captureScreenshot","params":{{"format":"png"}}}}"#);
        process::exit(1);
    }

    let html_file = &args[1];
    let width: f32 = args.get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1366.0);
    let height: f32 = args.get(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(768.0);

    // Suppress layout timing output
    // (In a real implementation, we'd have a quiet mode flag)

    // Create frame with specified viewport
    let viewport = Size { width, height };

    let frame = Frame::new(viewport);
    frame.borrow_mut().load_url(html_file);

    // Create and run the devtools server
    let mut server = DevtoolsServer::new();

    {
        let mut frame_ref = frame.borrow_mut();
        if let Err(e) = server.run_stdio(&mut frame_ref) {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}
