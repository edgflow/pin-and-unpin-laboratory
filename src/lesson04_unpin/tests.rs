use super::*;

/// Compiles only when `T: Unpin`. The compiler's own answer.
fn must_be_unpin<T: ?Sized + Unpin>() {}

#[test]
fn ordinary_types_are_unpin() {
    must_be_unpin::<u32>();
    must_be_unpin::<String>();
    must_be_unpin::<Vec<u8>>();
    must_be_unpin::<*const String>();
    assert!(is_unpin!(u32) && is_unpin!(String) && is_unpin!(Vec<u8>));
}

#[test]
fn a_raw_pointer_does_not_make_a_struct_address_sensitive_to_the_compiler() {
    // Lesson 2's frame IS address-sensitive. The compiler cannot know.
    must_be_unpin::<LooseFrame>();
    assert!(is_unpin!(LooseFrame));
}

#[test]
fn one_marker_field_removes_unpin() {
    assert!(!is_unpin!(PhantomPinned));
    assert!(!is_unpin!(PinnedFrame));
}

#[test]
fn a_wrapper_follows_its_fields() {
    assert!(is_unpin!(Envelope<String>));
    assert!(!is_unpin!(Envelope<PinnedFrame>));
    assert!(!is_unpin!((u32, PinnedFrame)));
    assert!(!is_unpin!(Option<PinnedFrame>));
    assert!(!is_unpin!([PinnedFrame; 2]));
}

#[test]
fn pointers_are_unpin_whatever_they_point_at() {
    must_be_unpin::<Box<PinnedFrame>>();
    must_be_unpin::<&mut PinnedFrame>();
    must_be_unpin::<&PinnedFrame>();
    must_be_unpin::<Pin<Box<PinnedFrame>>>();
    assert!(is_unpin!(Envelope<Box<PinnedFrame>>));
}

#[test]
fn pinning_an_unpin_value_restricts_nothing() {
    let mut text = String::from("abc");
    let mut pinned = Pin::new(&mut text);
    pinned.push('d'); // DerefMut is there
    let plain: &mut String = pinned.get_mut(); // and so is get_mut
    let taken = mem::take(plain); // so the value can be moved out
    assert_eq!(taken, "abcd");
    assert_eq!(text, "");
}

#[test]
fn a_hand_written_unpin_switches_the_protection_off() {
    assert!(is_unpin!(Reckless));
    let mut one = Box::pin(Reckless::new(1));
    let mut two = Box::pin(Reckless::new(2));
    mem::swap(one.as_mut().get_mut(), two.as_mut().get_mut());
    assert_eq!((one.id, two.id), (2, 1));
    let unpinned: Box<Reckless> = Pin::into_inner(one);
    assert_eq!(unpinned.id, 2);
}

#[test]
fn the_outer_pointer_can_move_and_the_frame_still_cannot() {
    // The challenge at the end of the lesson.
    let mut frame: Pin<Box<PinnedFrame>> = PinnedFrame::pinned(1, "hello");
    let at = frame.payload_is_at();
    // Pin<Box<_>> is Unpin, so pinning IT gives full &mut access to IT...
    let outer: Pin<&mut Pin<Box<PinnedFrame>>> = Pin::new(&mut frame);
    let pointer: &mut Pin<Box<PinnedFrame>> = outer.get_mut();
    // ...and we can swap the two POINTERS.
    let mut other = PinnedFrame::pinned(2, "world");
    mem::swap(pointer, &mut other);
    // Frame 1 is now owned by `other`. It never left its address.
    assert_eq!(other.id, 1);
    assert_eq!(other.payload_is_at(), at);
    assert_eq!(other.as_ref().payload(), Some("hello"));
    assert_eq!(frame.as_ref().payload(), Some("world"));
}
