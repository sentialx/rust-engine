mod layout_comparison;

use layout_comparison::comparison::{compare_svgs, format_differences, parse_svg};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const TOLERANCE: f32 = 1.0;
const DEFAULT_VIEWPORT: (u32, u32) = (800, 600);

/// Parse viewport from HTML meta tag: <meta name="viewport-size" content="WIDTHxHEIGHT">
fn parse_viewport_from_html(html_path: &PathBuf) -> (u32, u32) {
    let content = fs::read_to_string(html_path).unwrap_or_default();

    // Simple regex-free parsing
    if let Some(start) = content.find("viewport-size") {
        if let Some(content_start) = content[start..].find("content=\"") {
            let rest = &content[start + content_start + 9..];
            if let Some(end) = rest.find('"') {
                let value = &rest[..end];
                let parts: Vec<&str> = value.split('x').collect();
                if parts.len() == 2 {
                    if let (Ok(w), Ok(h)) = (parts[0].parse(), parts[1].parse()) {
                        return (w, h);
                    }
                }
            }
        }
    }
    DEFAULT_VIEWPORT
}

/// Get the path to the test_fixtures directory
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_fixtures")
}

/// Get the path to the offscreen_render binary
fn offscreen_render_binary() -> PathBuf {
    // In test mode, the binary should be in target/debug
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("debug");
    path.push("offscreen_render");
    path
}

/// Get the path to the graviton output directory
fn graviton_output_dir() -> PathBuf {
    fixtures_dir().join("graviton_output")
}

/// Run Graviton layout on HTML file and return SVG output
fn run_graviton(html_path: &PathBuf) -> Result<String, String> {
    // Output to graviton_output directory for debugging
    let output_dir = graviton_output_dir();
    let _ = fs::create_dir_all(&output_dir);

    let filename = html_path.file_stem().unwrap().to_str().unwrap();
    let output_path = output_dir.join(format!("{}.svg", filename));

    let binary = offscreen_render_binary();
    if !binary.exists() {
        return Err(format!(
            "offscreen_render binary not found at {:?}. Run `cargo build --bin offscreen_render` first.",
            binary
        ));
    }

    // Parse viewport from HTML file
    let (width, height) = parse_viewport_from_html(html_path);

    let output = Command::new(&binary)
        .arg(html_path)
        .arg(&output_path)
        .arg(width.to_string())
        .arg(height.to_string())
        .output()
        .map_err(|e| format!("Failed to run offscreen_render: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "offscreen_render failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let svg = fs::read_to_string(&output_path)
        .map_err(|e| format!("Failed to read generated SVG: {}", e))?;

    // Keep the file for debugging (output is gitignored)

    Ok(svg)
}

/// Compare Graviton output against baseline SVG
fn compare_layout(fixture_name: &str) -> Result<(), String> {
    let fixtures = fixtures_dir();
    let html_path = fixtures.join(format!("{}.html", fixture_name));
    let baseline_path = fixtures.join(format!("{}.svg", fixture_name));

    // Check files exist
    if !html_path.exists() {
        return Err(format!("HTML fixture not found: {:?}", html_path));
    }
    if !baseline_path.exists() {
        return Err(format!(
            "Baseline SVG not found: {:?}. Run `cd scripts && npm install && npm run generate` to generate baselines.",
            baseline_path
        ));
    }

    // Read baseline
    let baseline_svg =
        fs::read_to_string(&baseline_path).map_err(|e| format!("Failed to read baseline: {}", e))?;
    let baseline_rects = parse_svg(&baseline_svg);

    // Run Graviton
    let actual_svg = run_graviton(&html_path)?;
    let actual_rects = parse_svg(&actual_svg);

    // Compare
    match compare_svgs(&baseline_rects, &actual_rects, TOLERANCE) {
        Ok(()) => Ok(()),
        Err(diffs) => Err(format!(
            "Layout comparison failed for {}:\n{}",
            fixture_name,
            format_differences(&diffs)
        )),
    }
}

// =============================================================================
// Consolidated Test Fixtures
// Each file tests multiple related scenarios
// =============================================================================

#[test]
fn test_block_layout() {
    // Tests: basic block, block stacking, auto width, nested blocks, whitespace between blocks
    if let Err(e) = compare_layout("block_layout") {
        panic!("{}", e);
    }
}

#[test]
#[ignore = "Requires baseline SVGs to be generated first"]
fn test_margins() {
    // Tests: sibling margin collapsing (equal, asymmetric, chain)
    // Tests: parent-child margin collapsing (NOT YET IMPLEMENTED)
    //   - parent-top/child-top: margins should collapse when parent has no padding/border
    //   - parent-top-padded/child-top-padded: padding prevents collapse
    if let Err(e) = compare_layout("margins") {
        panic!("{}", e);
    }
}

#[test]
#[ignore = "Requires baseline SVGs to be generated first"]
fn test_inline() {
    // Tests: horizontal inline, wrapping, mixed sizes, nested inline
    if let Err(e) = compare_layout("inline") {
        panic!("{}", e);
    }
}

#[test]
#[ignore = "Requires baseline SVGs to be generated first"]
fn test_inline_block() {
    // Tests: exact fit, one-pixel-over wrap, overflow, margins, padding,
    //        shrink-to-fit, nested content, inline-blocks in inline-blocks
    if let Err(e) = compare_layout("inline_block") {
        panic!("{}", e);
    }
}

// =============================================================================
// Individual Test Fixtures
// =============================================================================

#[test]
#[ignore = "Requires baseline SVGs to be generated first"]
fn test_block_margins() {
    if let Err(e) = compare_layout("block_margins") {
        panic!("{}", e);
    }
}

#[test]
#[ignore = "Requires baseline SVGs to be generated first"]
fn test_padding_nested() {
    if let Err(e) = compare_layout("padding_nested") {
        panic!("{}", e);
    }
}

#[test]
#[ignore = "Requires baseline SVGs to be generated first"]
fn test_sidebar_content() {
    if let Err(e) = compare_layout("sidebar_content") {
        panic!("{}", e);
    }
}

#[test]
#[ignore = "Requires baseline SVGs to be generated first"]
fn test_inline_block_wrap() {
    if let Err(e) = compare_layout("inline_block_wrap") {
        panic!("{}", e);
    }
}

#[test]
fn test_inline_block_mixed() {
    // Tests: mixed inline and inline-block elements, inline elements followed by block elements
    if let Err(e) = compare_layout("inline_block_mixed") {
        panic!("{}", e);
    }
}

#[test]
#[ignore = "Requires baseline SVGs to be generated first"]
fn test_inline_block_wrap_mixed_sizes() {
    if let Err(e) = compare_layout("inline_block_wrap_mixed_sizes") {
        panic!("{}", e);
    }
}

#[test]
#[ignore = "Requires baseline SVGs to be generated first"]
fn test_inline_block_zero_size() {
    if let Err(e) = compare_layout("inline_block_zero_size") {
        panic!("{}", e);
    }
}

// =============================================================================
// Unit tests for comparison logic
// =============================================================================

#[test]
fn test_svg_parsing() {
    let svg = r#"
<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600" viewBox="0 0 800 600">
  <rect x="0" y="0" width="800" height="600" fill="white"/>
  <rect x="18" y="18" width="240" height="140" fill="rgba(204,204,204,1)" data-class="box"/>
</svg>
"#;
    let rects = parse_svg(svg);
    assert_eq!(rects.len(), 1);
    assert_eq!(rects[0].class, "box");
}
