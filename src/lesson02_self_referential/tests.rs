use super::*;

#[test]
fn a_new_frame_points_nowhere() {
    let frame = Frame::new(1, "hello");
    assert_eq!(frame.cursor_says(), 0);
    assert!(!frame.points_at_itself());
}

#[test]
fn linked_where_it_stands_it_points_at_itself() {
    let mut frame = Frame::new(1, "hello");
    frame.link();
    assert!(frame.points_at_itself());
}

#[test]
fn a_move_into_a_box_leaves_the_cursor_behind() {
    let mut frame = Frame::new(1, "hello");
    frame.link();
    let on_the_stack = frame.payload_is_at();
    let boxed = Box::new(frame);
    // The cursor was copied bit for bit: it still names the stack slot.
    assert_eq!(boxed.cursor_says(), on_the_stack);
    assert_ne!(boxed.payload_is_at(), on_the_stack);
    assert!(!boxed.points_at_itself());
}

#[test]
fn a_cursor_into_the_text_survives_a_move() {
    let text = TextCursor::new("hello");
    let boxed = Box::new(text);
    assert!(boxed.points_at_its_text());
}

#[test]
fn a_position_survives_every_move() {
    let parsed = OffsetFrame::parse("GET /health");
    let mut shelf = vec![Box::new(parsed)];
    shelf.reserve(64);
    assert_eq!(shelf[0].payload(), "/health");
    assert_eq!(OffsetFrame::parse("PING").payload(), "PING");
}

#[test]
fn boxed_then_linked_survives_moving_the_box() {
    let mut frame = Box::new(Frame::new(1, "hello"));
    frame.link();
    let moved = frame;
    let shelf = [moved];
    assert!(shelf[0].points_at_itself());
}

#[test]
fn swapping_the_contents_crosses_the_cursors() {
    let mut a = Box::new(Frame::new(1, "first"));
    let mut b = Box::new(Frame::new(2, "second"));
    a.link();
    b.link();
    mem::swap(&mut *a, &mut *b);
    assert!(!a.points_at_itself() && !b.points_at_itself());
    // Not dangling: each cursor names a live field of the OTHER frame.
    assert_eq!(a.cursor_says(), b.payload_is_at());
    assert_eq!(b.cursor_says(), a.payload_is_at());
    assert_eq!((a.id, b.id), (2, 1));
}

#[test]
fn replace_moves_a_frame_out_through_a_mutable_reference() {
    // The challenge at the end of the lesson.
    let mut boxed = Box::new(Frame::new(1, "first"));
    boxed.link();
    let taken = mem::replace(&mut *boxed, Frame::new(2, "second"));
    // The linked frame left the box. Its cursor names the place it left,
    // and that place now holds frame 2.
    assert_eq!(taken.id, 1);
    assert!(!taken.points_at_itself());
    assert_eq!(taken.cursor_says(), boxed.payload_is_at());
}
