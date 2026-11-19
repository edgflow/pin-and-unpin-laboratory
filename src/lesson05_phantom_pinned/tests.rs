use super::*;

fn must_be_send_and_sync<T: Send + Sync>() {}
fn must_be_unpin<T: Unpin>() {}

#[test]
fn the_marker_has_no_size_and_adds_none() {
    assert_eq!(mem::size_of::<PhantomPinned>(), 0);
    assert_eq!(mem::align_of::<PhantomPinned>(), 1);
    assert_eq!(mem::size_of::<Marked>(), mem::size_of::<Bare>());
}

#[test]
fn the_marker_removes_unpin_and_nothing_else() {
    must_be_unpin::<Bare>();
    assert!(!is_unpin!(Marked));
    // It is still Send and Sync, and it is Copy.
    must_be_send_and_sync::<PhantomPinned>();
    let marker = PhantomPinned;
    let copy = marker;
    let _both = (marker, copy);
}

#[test]
fn phantom_data_passes_the_answer_through() {
    must_be_unpin::<PhantomData<String>>();
    assert!(!is_unpin!(PhantomData<PhantomPinned>));
    assert!(!is_unpin!(ViaPhantomData));
}

#[test]
fn before_it_is_pinned_a_marked_value_moves_freely() {
    let log: Log = Rc::default();
    let session = Session::new("loose", &log);
    let on_the_stack = session.address();
    let boxed = Box::new(session);
    assert_ne!(boxed.address(), on_the_stack);
    let back_out: Session = *boxed; // moving out of a plain Box is fine
    drop(back_out);
    assert_eq!(log.borrow()[0].pinned_at, 0, "it was never pinned");
}

#[test]
fn a_pinned_session_dies_where_it_was_pinned() {
    let log: Log = Rc::default();
    let session = Box::pin(Session::new("heap", &log));
    session.as_ref().remember_where_i_am();
    let pinned_at = session.address();
    let mut shelf = vec![session];
    shelf.reserve(4096);
    let moved_shelf = shelf;
    drop(moved_shelf);
    let farewell = log.borrow()[0].clone();
    assert_eq!(farewell.pinned_at, pinned_at);
    assert_eq!(farewell.dropped_at, pinned_at, "same address at the drop");
}

#[test]
fn a_session_pinned_on_the_stack_dies_there_too() {
    let log: Log = Rc::default();
    {
        let session = pin!(Session::new("stack", &log));
        session.as_ref().remember_where_i_am();
    }
    let farewell = log.borrow()[0].clone();
    assert_ne!(farewell.pinned_at, 0);
    assert_eq!(farewell.pinned_at, farewell.dropped_at);
}

#[test]
fn set_drops_the_old_value_in_place_before_writing_the_new_one() {
    // The challenge at the end of the lesson.
    let log: Log = Rc::default();
    let mut slot = Box::pin(Session::new("old", &log));
    slot.as_ref().remember_where_i_am();
    let place = slot.address();
    slot.set(Session::new("new", &log));
    // "old" was dropped at once, at the address it was pinned at.
    assert_eq!(log.borrow().len(), 1);
    assert_eq!(log.borrow()[0].name, "old");
    assert_eq!(log.borrow()[0].dropped_at, place);
    // "new" lives in the same place, and has not been told so yet.
    assert_eq!(slot.address(), place);
    drop(slot);
    assert_eq!(log.borrow()[1].name, "new");
    assert_eq!(log.borrow()[1].pinned_at, 0);
    assert_eq!(log.borrow()[1].dropped_at, place);
}
