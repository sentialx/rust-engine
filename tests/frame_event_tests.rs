use graviton::frame::Frame;
use graviton::html::ElementKind;
use graviton::layout::Rect;

#[test]
fn test_input_default_click_runs() {
    let html = r#"
<!DOCTYPE html>
<html>
<body>
  <input id="a" />
</body>
</html>
"#;

    let mut frame = Frame::new(Rect { x: 0.0, y: 0.0, width: 200.0, height: 200.0 });
    frame.load_html(html);

    let node = frame.get_element_by_id("a").expect("input should exist");
    {
        let borrowed = node.borrow();
        match &borrowed.element_kind {
            ElementKind::Input(input) => assert_eq!(input.clicks, 0),
            _ => panic!("expected input element kind"),
        }
    }

    let handled = frame.dispatch_click(&node);
    assert!(handled);

    let borrowed = node.borrow();
    match &borrowed.element_kind {
        ElementKind::Input(input) => assert_eq!(input.clicks, 1),
        _ => panic!("expected input element kind"),
    }
}

#[test]
fn test_prevent_default_skips_input_default() {
    let html = r#"
<!DOCTYPE html>
<html>
<body>
  <input id="b" />
</body>
</html>
"#;

    let mut frame = Frame::new(Rect { x: 0.0, y: 0.0, width: 200.0, height: 200.0 });
    frame.load_html(html);

    let node = frame.get_element_by_id("b").expect("input should exist");
    frame.on_click(&node, |_frame, _node, event| {
        event.prevent_default();
    });

    let handled = frame.dispatch_click(&node);
    assert!(handled);

    let borrowed = node.borrow();
    match &borrowed.element_kind {
        ElementKind::Input(input) => assert_eq!(input.clicks, 0),
        _ => panic!("expected input element kind"),
    }
}
