//! A job queue of boxed futures.
//!
//!   cargo run --example boxed_future
//!
//! Three unrelated async functions go into one queue as `BoxFuture`s.
//! The queue is drained round-robin: each job gets one poll per round,
//! and a job that is not finished goes to the back of the queue. Being
//! moved around the queue is fine, because only the 16-byte box moves.
use pin_and_unpin_laboratory::lesson06_pin_box::{
    BoxFuture, close, countdown, ping, read_frame,
};
use std::collections::VecDeque;
use std::task::{Context, Poll, Waker};

fn main() {
    let mut queue: VecDeque<(&str, BoxFuture<'static, String>)> =
        VecDeque::new();
    queue.push_back(("read", Box::pin(read_frame(7))));
    queue.push_back(("ping", Box::pin(ping())));
    queue.push_back(("close", Box::pin(close())));
    queue.push_back((
        "countdown",
        Box::pin(async { countdown(2).await.to_string() }),
    ));

    let mut context = Context::from_waker(Waker::noop());
    let mut round = 1;
    while !queue.is_empty() {
        for _ in 0..queue.len() {
            let (name, mut job) = queue.pop_front().expect("not empty");
            match job.as_mut().poll(&mut context) {
                Poll::Ready(answer) => {
                    println!("round {round}: {name:<9} done: {answer:?}");
                }
                Poll::Pending => {
                    println!("round {round}: {name:<9} not yet");
                    queue.push_back((name, job));
                }
            }
        }
        round += 1;
    }
}
