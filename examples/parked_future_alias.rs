//! An experiment for lesson 8: looking at a parked future.
//!
//!   cargo run --example parked_future_alias
//!   cargo +nightly miri run --example parked_future_alias
//!   MIRIFLAGS=-Zmiri-tree-borrows \
//!       cargo +nightly miri run --example parked_future_alias
//!
//! Every line here is safe Rust, and the program prints the right answer.
//! Under Miri's default aliasing model (Stacked Borrows, an experimental
//! model) the second poll is reported as an error. Under its other model
//! (Tree Borrows) it is accepted.
//!
//! What it shows is what is INSIDE the parked future: a `&mut` to a
//! local variable that lives in the same future. When we take a shared
//! reference to the whole future, the stricter model decides that the
//! inner `&mut` can no longer be unique.
use pin_and_unpin_laboratory::lesson07_future_poll::Inbox;
use pin_and_unpin_laboratory::lesson08_async_state_machine as lesson08;
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

fn main() {
    let inbox = Inbox::new();
    let mut future = pin!(lesson08::borrows_its_own_local(&inbox));
    let mut context = Context::from_waker(Waker::noop());

    // First poll: the future parks at its `.await`, holding `counter`,
    // a `&mut` to `frames_seen`, and both are stored inside the future.
    assert!(future.as_mut().poll(&mut context).is_pending());

    // A shared reference to the whole parked future. Safe code.
    let whole = &*future;
    println!("the parked future is {} bytes", size_of_val(whole));

    // Second poll: the future goes on and writes through `counter`.
    inbox.deliver("PING");
    match future.as_mut().poll(&mut context) {
        Poll::Ready(answer) => println!("answer: {answer}"),
        Poll::Pending => println!("still pending"),
    }
}
