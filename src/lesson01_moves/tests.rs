use super::*;

#[test]
fn the_field_is_inside_the_struct() {
    let frame = Frame::new(1, "hello");
    let at = locate(&frame);
    let end = at.frame + mem::size_of::<Frame>();
    assert!(at.payload_field >= at.frame && at.payload_field < end);
    assert!(at.id_field >= at.frame && at.id_field < end);
    assert_ne!(at.payload_field, at.id_field, "two fields, two places");
}

#[test]
fn the_text_is_not_inside_the_struct() {
    let frame = Frame::new(1, "hello");
    let at = locate(&frame);
    let end = at.frame + mem::size_of::<Frame>();
    assert!(at.text < at.frame || at.text >= end);
}

#[test]
fn a_box_puts_the_struct_somewhere_else_and_leaves_the_text() {
    let frame = Frame::new(1, "hello");
    let before = locate(&frame);
    let boxed = Box::new(frame);
    let after = locate(&boxed);
    assert_ne!(
        after.frame, before.frame,
        "the struct was copied to the heap"
    );
    assert_ne!(
        after.payload_field, before.payload_field,
        "its field went with it"
    );
    assert_eq!(after.text, before.text, "the text was not copied");
}

#[test]
fn the_field_keeps_its_offset_wherever_the_struct_goes() {
    let frame = Frame::new(1, "hello");
    let before = locate(&frame);
    let boxed = Box::new(frame);
    let after = locate(&boxed);
    assert_eq!(
        after.payload_field - after.frame,
        before.payload_field - before.frame
    );
    assert_eq!(after.id_field - after.frame, before.id_field - before.frame);
}

#[test]
fn a_swap_exchanges_the_structs_not_the_texts() {
    let mut a = Frame::new(1, "first");
    let mut b = Frame::new(2, "second");
    let (text_a, text_b) = (locate(&a).text, locate(&b).text);
    mem::swap(&mut a, &mut b);
    assert_eq!((a.id, b.id), (2, 1));
    assert_eq!((locate(&a).text, locate(&b).text), (text_b, text_a));
}

#[test]
fn a_moved_frame_is_still_the_same_frame() {
    let (frame, _) = into_function(Frame::new(9, "payload"));
    assert_eq!((frame.id, frame.payload.as_str()), (9, "payload"));
}

#[test]
fn moving_a_box_moves_the_pointer_not_the_frame() {
    // The challenge at the end of the lesson. A Box is one pointer.
    // Moving the Box copies that pointer. The Frame it points at stays.
    let boxed = Box::new(Frame::new(1, "hello"));
    let before = locate(&boxed);
    let moved_box = boxed;
    let shelf = [moved_box];
    assert_eq!(locate(&shelf[0]), before);
}
