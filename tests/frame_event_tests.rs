use std::cell::RefCell;
use std::rc::Rc;

use graviton::events::{DefaultEventHandler, EventSink, InputEventKind};
use graviton::frame::Frame;
use graviton::dom::ElementKind;
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

    let frame = Frame::new(Rect { x: 0.0, y: 0.0, width: 200.0, height: 200.0 });
    frame.borrow_mut().load_html(html);

    let node = frame.borrow().get_element_by_id("a").expect("input should exist");
    {
        let borrowed = node.borrow();
        match &borrowed.element_kind {
            ElementKind::Input(input) => assert_eq!(input.clicks, 0),
            _ => panic!("expected input element kind"),
        }
    }

    // Use EventSink to dispatch the click
    let mut sink = EventSink::new(frame.clone());
    sink.prepend_handler(Box::new(DefaultEventHandler::new()));

    // Get the element's position for hit testing
    let (x, y) = {
        let borrowed = node.borrow();
        borrowed.computed_flow
            .as_ref()
            .map(|f| (f.x + f.width / 2.0, f.y + f.height / 2.0))
            .unwrap_or((50.0, 50.0))
    };

    let result = sink.dispatch(InputEventKind::Click, x, y);
    assert!(result.handled);

    let borrowed = node.borrow();
    match &borrowed.element_kind {
        ElementKind::Input(input) => assert_eq!(input.clicks, 1),
        _ => panic!("expected input element kind"),
    }
}

#[test]
fn test_prevent_default_with_custom_handler() {
    use graviton::events::{EventHandler, EventResult, InputEvent};

    let html = r#"
<!DOCTYPE html>
<html>
<body>
  <input id="b" />
</body>
</html>
"#;

    let frame = Frame::new(Rect { x: 0.0, y: 0.0, width: 200.0, height: 200.0 });
    frame.borrow_mut().load_html(html);

    let node = frame.borrow().get_element_by_id("b").expect("input should exist");

    // Create a custom handler that prevents default
    struct PreventDefaultHandler;
    impl EventHandler for PreventDefaultHandler {
        fn handle(&mut self, event: &mut InputEvent, _frame: &Rc<RefCell<Frame>>) -> EventResult {
            if event.kind == InputEventKind::Click {
                event.prevent_default();
                return EventResult::handled();
            }
            EventResult::default()
        }
    }

    // Use EventSink with custom handler before default
    let mut sink = EventSink::new(frame.clone());
    sink.prepend_handler(Box::new(PreventDefaultHandler));
    sink.prepend_handler(Box::new(DefaultEventHandler::new()));

    // Get the element's position for hit testing
    let (x, y) = {
        let borrowed = node.borrow();
        borrowed.computed_flow
            .as_ref()
            .map(|f| (f.x + f.width / 2.0, f.y + f.height / 2.0))
            .unwrap_or((50.0, 50.0))
    };

    let result = sink.dispatch(InputEventKind::Click, x, y);
    assert!(result.handled);

    // Default behavior should have been prevented
    let borrowed = node.borrow();
    match &borrowed.element_kind {
        ElementKind::Input(input) => assert_eq!(input.clicks, 0),
        _ => panic!("expected input element kind"),
    }
}
