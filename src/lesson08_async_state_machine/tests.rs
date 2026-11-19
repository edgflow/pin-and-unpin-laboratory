use super::*;
use crate::is_unpin;

#[test]
fn what_lives_across_an_await_is_stored_in_the_future() {
    let inbox = Inbox::new();
    let across = mem::size_of_val(&buffer_across_the_await(&inbox));
    let before = mem::size_of_val(&buffer_before_the_await(&inbox));
    assert!(across >= 1024, "the buffer is in there: {across} bytes");
    assert!(before < 1024, "the buffer is gone before parking: {before}");
}

#[test]
fn an_awaited_future_is_stored_inside_the_one_that_awaits_it() {
    let inbox = Inbox::new();
    let inner = mem::size_of_val(&buffer_across_the_await(&inbox));
    let outer = mem::size_of_val(&outer(&inbox));
    assert!(outer >= inner, "outer {outer} holds inner {inner}");
}

#[test]
fn every_async_future_here_is_not_unpin() {
    let inbox = Inbox::new();
    assert!(!is_unpin_value!(keeps_nothing(&inbox)));
    assert!(!is_unpin_value!(buffer_before_the_await(&inbox)));
    assert!(!is_unpin_value!(borrows_its_own_local(&inbox)));
    assert!(!is_unpin_value!(read_two(&inbox)));
    assert!(!is_unpin_value!(async {}), "even an empty async block");
}

#[test]
fn the_hand_written_machine_is_unpin() {
    assert!(is_unpin!(ReadTwo<'static>));
}

#[test]
fn the_async_fn_and_the_hand_written_machine_behave_the_same() {
    let inbox = Inbox::new();
    let network = inbox.feed(&["HEADER", "BODY"]);
    let by_compiler = block_on_for(read_two(&inbox), PATIENCE);
    network.join().unwrap();
    let network = inbox.feed(&["HEADER", "BODY"]);
    let by_hand = block_on_for(ReadTwo::new(&inbox), PATIENCE);
    network.join().unwrap();
    assert_eq!(by_compiler, by_hand);
    assert_eq!(by_hand.output.as_deref(), Some("HEADER | BODY"));
    assert_eq!((by_hand.polls, by_hand.wakes), (3, 2));
}

#[test]
fn a_future_may_be_moved_freely_before_its_first_poll() {
    let inbox = Inbox::new();
    let future = borrows_its_own_local(&inbox);
    let mut shelf = vec![Box::new(future)];
    shelf.reserve(1024);
    let future = *shelf.pop().unwrap();
    let network = inbox.feed(&["PING"]);
    let report = block_on_for(future, PATIENCE);
    network.join().unwrap();
    assert_eq!(report.output, Some(5));
}

#[test]
#[should_panic(expected = "polled after completion")]
fn the_hand_written_machine_refuses_a_poll_after_it_returned() {
    let inbox = Inbox::new();
    inbox.deliver("HEADER");
    let mut machine = ReadTwo::new(&inbox);
    let mut context = Context::from_waker(std::task::Waker::noop());
    assert!(Pin::new(&mut machine).poll(&mut context).is_pending());
    inbox.deliver("BODY");
    assert!(Pin::new(&mut machine).poll(&mut context).is_ready());
    let _ = Pin::new(&mut machine).poll(&mut context);
}
