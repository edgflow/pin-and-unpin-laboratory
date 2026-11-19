use super::*;
use crate::is_unpin;

const PATIENCE: Duration = Duration::from_secs(10);

#[test]
fn ready_at_once_takes_one_poll_and_no_wake() {
    let inbox = Inbox::new();
    inbox.deliver("here");
    let report = block_on_for(inbox.read_frame(), PATIENCE);
    assert_eq!(
        report,
        Report {
            output: Some("here".to_string()),
            polls: 1,
            wakes: 0
        }
    );
}

#[test]
fn pending_then_one_wake_then_ready() {
    let inbox = Inbox::new();
    let network = inbox.deliver_later("GET /health");
    let report = block_on_for(inbox.read_frame(), PATIENCE);
    network.join().unwrap();
    assert_eq!(report.output.as_deref(), Some("GET /health"));
    assert_eq!(report.polls, 2, "asked twice, not in a loop");
    assert_eq!(report.wakes, 1, "woken exactly once");
}

#[test]
fn a_future_that_keeps_no_waker_is_never_polled_again() {
    let inbox = Inbox::new();
    let network = inbox.deliver_later("GET /health");
    let short = Duration::from_millis(200);
    let report = block_on_for(inbox.read_frame_forgetfully(), short);
    network.join().unwrap();
    assert_eq!(
        report,
        Report {
            output: None,
            polls: 1,
            wakes: 0
        }
    );
    assert!(inbox.has_frame(), "the frame arrived, and nobody was told");
}

#[test]
fn the_waker_can_be_called_from_another_thread() {
    fn must_be_send_and_sync<T: Send + Sync>() {}
    must_be_send_and_sync::<Waker>();
}

#[test]
fn our_read_frame_is_unpin_and_may_move_between_polls() {
    assert!(is_unpin!(ReadFrame));
    let inbox = Inbox::new();
    let mut future = inbox.read_frame();
    let mut context = Context::from_waker(Waker::noop());
    // First poll, pinned only for the length of this one call.
    assert!(Pin::new(&mut future).poll(&mut context).is_pending());
    // Move it. Legal: it is Unpin, nothing in it depends on its address.
    let mut moved = Box::new(future);
    inbox.deliver("after the move");
    let second = Pin::new(&mut *moved).poll(&mut context);
    assert_eq!(second, Poll::Ready("after the move".to_string()));
}

#[test]
fn an_address_sensitive_future_sees_one_address_for_life() {
    assert!(!is_unpin!(Anchored));
    let report = block_on_for(Anchored::new(4), PATIENCE);
    assert_eq!(report.output, Some(true));
    assert_eq!(report.polls, 5);
    assert_eq!(report.wakes, 4, "it woke itself after each Pending");
}

#[test]
fn an_async_block_can_await_the_hand_written_future() {
    let inbox = Inbox::new();
    let network = inbox.deliver_later("PING");
    let length = block_on(async { inbox.read_frame().await.len() });
    network.join().unwrap();
    assert_eq!(length, 4);
}

#[test]
fn a_newer_waker_replaces_the_stored_one() {
    // The challenge at the end of the lesson. The future is polled by
    // one task, then by another. Only the LAST one must be woken.
    struct Flag(AtomicBool);
    impl Wake for Flag {
        fn wake(self: Arc<Self>) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let (first, second) = (
        Arc::new(Flag(AtomicBool::new(false))),
        Arc::new(Flag(AtomicBool::new(false))),
    );
    let inbox = Inbox::new();
    let mut future = pin!(inbox.read_frame());
    for flag in [&first, &second] {
        let waker = Waker::from(Arc::clone(flag));
        let mut context = Context::from_waker(&waker);
        assert!(future.as_mut().poll(&mut context).is_pending());
    }
    inbox.deliver("frame");
    assert!(!first.0.load(Ordering::SeqCst), "the old waker is not used");
    assert!(second.0.load(Ordering::SeqCst), "the newest waker is");
}
