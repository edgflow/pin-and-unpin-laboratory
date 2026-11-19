use super::*;
use crate::is_unpin;
use crate::lesson07_future_poll::ReadFrame;
use std::cell::Cell;
use std::rc::Rc;

fn header_and_body() -> Timing<String> {
    ("HEADER | BODY".to_string(), 3)
}

#[test]
fn all_four_wrappers_agree() {
    let inbox = Inbox::new();
    let network = inbox.feed(&["HEADER", "BODY"]);
    let a = block_on_for(TimedUnpin::new(Box::pin(read_two(&inbox))), PATIENCE);
    network.join().unwrap();
    let network = inbox.feed(&["HEADER", "BODY"]);
    let b = block_on_for(TimedBoxed::new(read_two(&inbox)), PATIENCE);
    network.join().unwrap();
    let network = inbox.feed(&["HEADER", "BODY"]);
    let c = block_on_for(TimedByHand::new(read_two(&inbox)), PATIENCE);
    network.join().unwrap();
    let network = inbox.feed(&["HEADER", "BODY"]);
    let d = block_on_for(Timed::new(read_two(&inbox)), PATIENCE);
    network.join().unwrap();
    for output in [a.output, b.output, c.output, d.output] {
        assert_eq!(output, Some(header_and_body()));
    }
}

#[test]
fn who_is_unpin() {
    // Around an Unpin future, every wrapper is Unpin.
    assert!(is_unpin!(TimedUnpin<ReadFrame>));
    assert!(is_unpin!(TimedByHand<ReadFrame>));
    assert!(is_unpin!(Timed<ReadFrame>));
    // Around an async future, only the boxing one still is.
    let inbox = Inbox::new();
    assert!(is_unpin_value!(TimedBoxed::new(read_two(&inbox))));
    assert!(!is_unpin_value!(TimedByHand::new(read_two(&inbox))));
    assert!(!is_unpin_value!(Timed::new(read_two(&inbox))));
}

#[test]
fn boxing_trades_size_for_an_allocation() {
    let inbox = Inbox::new();
    let inline = mem::size_of_val(&Timed::new(read_two(&inbox)));
    let boxed = mem::size_of_val(&TimedBoxed::new(read_two(&inbox)));
    let bare = mem::size_of_val(&read_two(&inbox));
    assert!(inline >= bare, "the future is stored inside: {inline}");
    assert!(boxed < bare, "only a pointer is stored inside: {boxed}");
}

#[test]
fn projection_by_hand_keeps_a_self_referential_future_working() {
    let inbox = Inbox::new();
    let network = inbox.feed(&["PING"]);
    let wrapped = TimedByHand::new(borrows_its_own_local(&inbox));
    let report = block_on_for(wrapped, PATIENCE);
    network.join().unwrap();
    assert_eq!(report.output, Some((5, 2)));
}

#[test]
fn pin_project_keeps_a_self_referential_future_working() {
    let inbox = Inbox::new();
    let network = inbox.feed(&["PING"]);
    let wrapped = Timed::new(borrows_its_own_local(&inbox));
    let report = block_on_for(wrapped, PATIENCE);
    network.join().unwrap();
    assert_eq!(report.output, Some((5, 2)));
}

#[test]
fn a_shared_pinned_reference_reads_fields_without_unsafe() {
    let inbox = Inbox::new();
    let pinned = Box::pin(TimedByHand::new(read_two(&inbox)));
    assert_eq!(pinned.as_ref().polls_so_far(), 0);
}

#[test]
fn wrappers_nest() {
    let inbox = Inbox::new();
    let network = inbox.feed(&["HEADER", "BODY"]);
    let nested = Timed::new(TimedByHand::new(read_two(&inbox)));
    let report = block_on_for(nested, PATIENCE);
    network.join().unwrap();
    assert_eq!(report.output, Some((header_and_body(), 3)));
}

/// A future that tells us when it is dropped.
struct Noisy {
    dropped: Rc<Cell<bool>>,
}

impl Future for Noisy {
    type Output = &'static str;
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<&'static str> {
        Poll::Ready("done")
    }
}

impl Drop for Noisy {
    fn drop(&mut self) {
        self.dropped.set(true);
    }
}

#[test]
fn fused_drops_its_future_at_once_and_then_answers_none() {
    // The challenge at the end of the lesson.
    let dropped = Rc::new(Cell::new(false));
    let noisy = Noisy {
        dropped: Rc::clone(&dropped),
    };
    let mut fused = Box::pin(Fused::new(noisy));
    let mut context = Context::from_waker(std::task::Waker::noop());
    assert_eq!(fused.as_mut().poll(&mut context), Poll::Ready(Some("done")));
    assert!(dropped.get(), "dropped in place, as soon as it finished");
    assert!(fused.is_finished());
    // Polling again is fine now, as often as we like.
    assert_eq!(fused.as_mut().poll(&mut context), Poll::Ready(None));
    assert_eq!(fused.as_mut().poll(&mut context), Poll::Ready(None));
}
