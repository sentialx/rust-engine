// Inline layout tests
// Tests inline element positioning without cross-browser comparison

use std::cell::RefCell;
use std::rc::Rc;

use graviton::frame::Frame;
use graviton::dom::{DomElement, NodeType};
use graviton::layout::Rect;

/// Collect inline element boxes by class name
fn collect_inline_boxes(elements: &[Rc<RefCell<DomElement>>]) -> Vec<(String, f32, f32, f32, f32)> {
    let mut boxes = Vec::new();
    collect_boxes_recursive(elements, &mut boxes);
    boxes
}

fn collect_boxes_recursive(elements: &[Rc<RefCell<DomElement>>], out: &mut Vec<(String, f32, f32, f32, f32)>) {
    for el in elements {
        let borrowed = el.borrow();
        if borrowed.node_type == NodeType::Element {
            if let Some(ref flow) = borrowed.computed_flow {
                // Use class name as identifier
                if let Some(class) = borrowed.class_list.first() {
                    out.push((class.clone(), flow.x, flow.y, flow.width, flow.height));
                }
            }
        }
        collect_boxes_recursive(&borrowed.children, out);
    }
}

fn find_container_left(frame: &Frame, class: &str) -> f32 {
    fn find(elements: &[Rc<RefCell<DomElement>>], class: &str) -> Option<f32> {
        for el in elements {
            let borrowed = el.borrow();
            if borrowed.class_list.contains(&class.to_string()) {
                if let Some(ref flow) = borrowed.computed_flow {
                    let padding_left = borrowed.computed_style.as_ref()
                        .map(|s| s.padding.left).unwrap_or(0.0);
                    return Some(flow.x + padding_left);
                }
            }
            if let Some(x) = find(&borrowed.children, class) {
                return Some(x);
            }
        }
        None
    }
    find(&frame.dom_tree, class).unwrap_or(0.0)
}

#[test]
fn test_inline_horizontal_flow() {
    let html = r#"
<!DOCTYPE html>
<html>
<head>
  <style>
    body { margin: 0; padding: 8px; }
    .container { width: 500px; }
    .a, .b, .c { display: inline; background: red; }
  </style>
</head>
<body>
  <div class="container">
    <span class="a">aaa</span> <span class="b">bbb</span> <span class="c">ccc</span>
  </div>
</body>
</html>
"#;

    let frame = Frame::new(Rect { x: 0.0, y: 0.0, width: 600.0, height: 200.0 });
    frame.borrow_mut().load_html(html);

    let boxes = collect_inline_boxes(&frame.borrow().dom_tree);
    let spans: Vec<_> = boxes.iter().filter(|(c, _, _, _, _)| c == "a" || c == "b" || c == "c").collect();

    println!("Inline boxes: {:?}", spans);

    // x positions should be strictly increasing (left to right flow)
    for i in 1..spans.len() {
        assert!(spans[i].1 > spans[i-1].1,
            "Span {} x={:.2} should be after span {} x={:.2}",
            spans[i].0, spans[i].1, spans[i-1].0, spans[i-1].1);
    }
}

#[test]
fn test_inline_wrap_resets_x() {
    let html = r#"
<!DOCTYPE html>
<html>
<head>
  <style>
    body { margin: 0; padding: 8px; }
    .container { width: 80px; }
    .a, .b, .c, .d { display: inline; background: red; }
  </style>
</head>
<body>
  <div class="container">
    <span class="a">first</span> <span class="b">second</span> <span class="c">third</span> <span class="d">fourth</span>
  </div>
</body>
</html>
"#;

    let frame = Frame::new(Rect { x: 0.0, y: 0.0, width: 320.0, height: 200.0 });
    frame.borrow_mut().load_html(html);

    let boxes = collect_inline_boxes(&frame.borrow().dom_tree);
    let container_left = find_container_left(&frame.borrow(), "container");

    println!("Container left: {:.2}", container_left);
    println!("Inline boxes:");
    for (class, x, y, w, h) in &boxes {
        if class == "a" || class == "b" || class == "c" || class == "d" {
            println!("  .{}: x={:.2}, y={:.2}, w={:.2}, h={:.2}", class, x, y, w, h);
        }
    }

    // Group by y to find lines
    let spans: Vec<_> = boxes.iter()
        .filter(|(c, _, _, _, _)| c == "a" || c == "b" || c == "c" || c == "d")
        .collect();

    let first_y = spans[0].2;
    let mut lines: Vec<Vec<&(String, f32, f32, f32, f32)>> = vec![vec![]];

    for span in &spans {
        if (span.2 - first_y).abs() < 2.0 {
            lines[0].push(span);
        } else {
            // New line
            let line_idx = ((span.2 - first_y) / 14.0).round() as usize;
            while lines.len() <= line_idx {
                lines.push(vec![]);
            }
            lines[line_idx].push(span);
        }
    }

    println!("\nLines:");
    for (i, line) in lines.iter().enumerate() {
        println!("  Line {}: {:?}", i + 1, line.iter().map(|(c, x, _, _, _)| (c.as_str(), *x)).collect::<Vec<_>>());
    }

    // Check: first element of each line should be at container left edge
    let tolerance = 2.0;
    for (i, line) in lines.iter().enumerate() {
        if line.is_empty() { continue; }
        let min_x = line.iter().map(|(_, x, _, _, _)| *x).fold(f32::MAX, f32::min);
        assert!(
            (min_x - container_left).abs() < tolerance,
            "Line {} starts at x={:.2}, should be at container left {:.2}",
            i + 1, min_x, container_left
        );
    }
}

#[test]
fn test_fast_reflow_reuses_cached_layout_tree() {
    let html = r#"
<!DOCTYPE html>
<html>
<head>
  <style>
    body { margin: 0; padding: 8px; }
    .container { width: 200px; }
    .box { display: inline-block; width: 40px; height: 20px; }
  </style>
</head>
<body>
  <div class="container">
    <span class="box"></span>
    <span class="box"></span>
  </div>
</body>
</html>
"#;

    let frame = Frame::new(Rect { x: 0.0, y: 0.0, width: 320.0, height: 200.0 });
    frame.borrow_mut().load_html(html);

    let initial_ptr = frame.borrow()
        .cached_layout_tree
        .as_ref()
        .map(|tree| tree.as_ptr())
        .expect("cached layout tree should be built after full layout");

    frame.borrow_mut().set_viewport(280.0, 200.0);
    frame.borrow_mut().fast_reflow();

    let after_ptr = frame.borrow()
        .cached_layout_tree
        .as_ref()
        .map(|tree| tree.as_ptr())
        .expect("cached layout tree should remain after fast reflow");

    assert_eq!(initial_ptr, after_ptr, "fast reflow should reuse cached layout tree");
}

#[test]
fn test_inline_wrap_advances_y_for_wrapped_inline() {
    let html = r#"
<!DOCTYPE html>
<html>
<head>
  <style>
    body { margin: 0; padding: 8px; }
    .container { width: 80px; }
    .a, .b, .c { display: inline; background: red; }
  </style>
</head>
<body>
  <div class="container">
    <span class="a">first</span> <span class="b">second</span> <span class="c">third</span>
  </div>
</body>
</html>
"#;

    let frame = Frame::new(Rect { x: 0.0, y: 0.0, width: 320.0, height: 200.0 });
    frame.borrow_mut().load_html(html);

    let boxes = collect_inline_boxes(&frame.borrow().dom_tree);
    let a = boxes.iter().find(|(c, _, _, _, _)| c == "a").unwrap();
    let b = boxes.iter().find(|(c, _, _, _, _)| c == "b").unwrap();
    let c = boxes.iter().find(|(c, _, _, _, _)| c == "c").unwrap();

    println!("Inline boxes: a={:?}, b={:?}, c={:?}", a, b, c);

    // If the third inline wrapped, it should be positioned on a later line (y increased).
    assert!(c.2 > a.2, "Wrapped inline should advance y: a.y={:.2}, c.y={:.2}", a.2, c.2);
    assert!(c.2 >= b.2, "Wrapped inline should not be above previous inline: b.y={:.2}, c.y={:.2}", b.2, c.2);
}

#[test]
fn test_inline_transparent_with_block_child_layouts_block() {
    let html = r#"
<!DOCTYPE html>
<html>
<head>
  <style>
    body { margin: 0; padding: 8px; }
    .container { width: 200px; }
    .wrap { display: inline; }
    .block { display: block; width: 40px; height: 20px; background: red; }
  </style>
</head>
<body>
  <div class="container">
    <span class="wrap"><div class="block"></div></span>
  </div>
</body>
</html>
"#;

    let frame = Frame::new(Rect { x: 0.0, y: 0.0, width: 240.0, height: 200.0 });
    frame.borrow_mut().load_html(html);

    let boxes = collect_inline_boxes(&frame.borrow().dom_tree);
    let block = boxes.iter().find(|(c, _, _, _, _)| c == "block");
    assert!(block.is_some(), "Block child should be laid out for transparent inline wrapper");

    let (_, _, _, _, h) = block.unwrap();
    assert!(*h >= 18.0, "Block child height should be preserved (got {:.2})", h);
}

#[test]
fn test_inline_block_wraps_as_unit() {
    let html = r#"
<!DOCTYPE html>
<html>
<head>
  <style>
    body { margin: 0; padding: 8px; }
    .container { width: 80px; }
    .ib1, .ib2 { display: inline-block; width: 60px; height: 12px; background: red; }
  </style>
</head>
<body>
  <div class="container">
    <span class="ib1"></span><span class="ib2"></span>
  </div>
</body>
</html>
"#;

    let frame = Frame::new(Rect { x: 0.0, y: 0.0, width: 200.0, height: 200.0 });
    frame.borrow_mut().load_html(html);

    let boxes = collect_inline_boxes(&frame.borrow().dom_tree);
    let ib1 = boxes.iter().find(|(c, _, _, _, _)| c == "ib1").unwrap();
    let ib2 = boxes.iter().find(|(c, _, _, _, _)| c == "ib2").unwrap();
    let container_left = find_container_left(&frame.borrow(), "container");

    assert!(
        (ib1.1 - container_left).abs() < 2.0,
        "First inline-block should start at container left: {:.2} vs {:.2}",
        ib1.1,
        container_left
    );
    assert!(
        (ib2.1 - container_left).abs() < 2.0,
        "Wrapped inline-block should reset x: {:.2} vs {:.2}",
        ib2.1,
        container_left
    );
    assert!(
        ib2.2 > ib1.2,
        "Wrapped inline-block should advance y: ib1.y={:.2}, ib2.y={:.2}",
        ib1.2,
        ib2.2
    );
}
