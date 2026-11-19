use super::*;

#[test]
fn pin_is_the_same_size_as_the_pointer_it_wraps() {
    assert_eq!(
        mem::size_of::<Pin<Box<Frame>>>(),
        mem::size_of::<Box<Frame>>()
    );
    assert_eq!(
        mem::size_of::<Pin<&mut Frame>>(),
        mem::size_of::<&mut Frame>()
    );
}

#[test]
fn an_unlinked_frame_has_nothing_to_read() {
    let frame = Box::pin(Frame::unlinked(1, "hello"));
    assert_eq!(frame.as_ref().payload(), None);
    assert!(!frame.points_at_itself());
}

#[test]
fn a_pinned_frame_reads_its_payload_through_the_cursor() {
    let frame = Frame::pinned(1, "hello");
    assert!(frame.points_at_itself());
    assert_eq!(frame.as_ref().payload(), Some("hello"));
}

#[test]
fn moving_the_pin_does_not_move_the_frame() {
    let frame = Frame::pinned(1, "hello");
    let before = frame.payload_is_at();
    let mut shelf = Vec::with_capacity(1);
    shelf.push(frame);
    shelf.reserve(4096);
    let other_shelf = shelf;
    assert_eq!(other_shelf[0].payload_is_at(), before);
    assert_eq!(other_shelf[0].as_ref().payload(), Some("hello"));
}

#[test]
fn a_frame_can_be_pinned_on_the_stack() {
    let mut frame = pin!(Frame::unlinked(1, "hello"));
    frame.as_mut().link();
    assert_eq!(frame.as_ref().payload(), Some("hello"));
}

#[test]
fn linking_twice_is_harmless() {
    let mut frame = Frame::pinned(1, "hello");
    frame.as_mut().link();
    assert_eq!(frame.as_ref().payload(), Some("hello"));
}

#[test]
fn many_pinned_frames_keep_their_own_payloads() {
    let mut frames: Vec<Pin<Box<Frame>>> = Vec::new();
    for id in 0..100 {
        frames.push(Frame::pinned(id, &format!("frame {id}")));
    }
    frames.reverse();
    for frame in &frames {
        let expected = format!("frame {}", frame.id);
        assert_eq!(frame.as_ref().payload(), Some(expected.as_str()));
    }
}

#[test]
fn pin_does_not_protect_a_type_that_never_asked_for_it() {
    // Lesson 2's frame has no PhantomPinned. The same swap compiles.
    let mut a = Box::pin(LooseFrame::new(1, "first"));
    let mut b = Box::pin(LooseFrame::new(2, "second"));
    a.as_mut().get_mut().link();
    b.as_mut().get_mut().link();
    mem::swap(a.as_mut().get_mut(), b.as_mut().get_mut());
    assert!(!a.points_at_itself() && !b.points_at_itself());
}

#[test]
fn set_replaces_the_value_without_moving_anything_out() {
    // The challenge at the end of the lesson. `Pin::set` drops the old
    // frame where it stands and writes a new one into the same place.
    let mut frame = Frame::pinned(1, "old");
    let place = frame.payload_is_at();
    frame.set(Frame::unlinked(9, "new"));
    assert_eq!(frame.id, 9);
    assert_eq!(frame.payload_is_at(), place, "same address");
    assert_eq!(frame.as_ref().payload(), None, "the new frame is unlinked");
    frame.as_mut().link();
    assert_eq!(frame.as_ref().payload(), Some("new"));
}
