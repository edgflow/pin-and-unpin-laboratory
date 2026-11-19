//! Lesson 8 · The async state machine.
//!
//! What does the compiler put into the future of an `async fn`? We do
//! not read its output. We measure it from the outside (sizes, `Unpin`,
//! behaviour) and write the same machine by hand to compare.
//!
//! The hand-written machine is a MODEL. The compiler's real output is
//! not this code and its layout is not specified.
//!
//! Run it:  cargo run -- 8
use crate::is_unpin_value;
use crate::lesson07_future_poll::{Inbox, ReadFrame, Report, block_on_for};
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

// ------------------------------------------- five small async functions

/// Awaits once, keeps nothing.
pub async fn keeps_nothing(inbox: &Inbox) -> usize {
    inbox.read_frame().await.len()
}

/// A 1 KB buffer that is finished with BEFORE the await.
pub async fn buffer_before_the_await(inbox: &Inbox) -> usize {
    let checksum = {
        let buffer = [1u8; 1024];
        buffer.iter().map(|byte| *byte as usize).sum::<usize>()
    };
    inbox.read_frame().await.len() + checksum
}

/// The same buffer, still needed AFTER the await.
pub async fn buffer_across_the_await(inbox: &Inbox) -> usize {
    let buffer = [1u8; 1024];
    let frame = inbox.read_frame().await;
    frame.len() + buffer.iter().map(|byte| *byte as usize).sum::<usize>()
}

/// A local, and a mutable reference to that local, both alive across
/// the await. The reference points into the future's own storage.
pub async fn borrows_its_own_local(inbox: &Inbox) -> usize {
    let mut frames_seen = 0;
    let counter = &mut frames_seen;
    let frame = inbox.read_frame().await;
    *counter += 1;
    frame.len() + *counter
}

/// Awaits another async fn. The inner future lives inside this one.
pub async fn outer(inbox: &Inbox) -> usize {
    buffer_across_the_await(inbox).await + 1
}

// --------------------------------- one async fn, and the same by hand

/// Reads a header frame, then a body frame. `header` is alive across
/// the second await.
pub async fn read_two(inbox: &Inbox) -> String {
    let header = inbox.read_frame().await;
    let body = inbox.read_frame().await;
    format!("{header} | {body}")
}

/// The same function as a state machine written by hand: one variant
/// per place where the function can be parked, each holding exactly
/// what is still needed from there on.
pub enum ReadTwo<'a> {
    /// Not polled yet. Holds only the argument.
    Start { inbox: &'a Inbox },
    /// Parked at the first `.await`.
    WaitingForHeader { inbox: &'a Inbox, future: ReadFrame },
    /// Parked at the second `.await`. `header` has to be kept.
    WaitingForBody { header: String, future: ReadFrame },
    /// Returned. Must not be polled again.
    Done,
}

impl<'a> ReadTwo<'a> {
    pub fn new(inbox: &'a Inbox) -> ReadTwo<'a> {
        ReadTwo::Start { inbox }
    }
}

impl Future for ReadTwo<'_> {
    type Output = String;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<String> {
        // Every field here is `Unpin`, so `ReadTwo` is too, and we may
        // have plain `&mut Self`. The compiler's futures cannot do this.
        let this = self.get_mut();
        loop {
            match mem::replace(this, ReadTwo::Done) {
                ReadTwo::Start { inbox } => {
                    let future = inbox.read_frame();
                    *this = ReadTwo::WaitingForHeader { inbox, future };
                }
                ReadTwo::WaitingForHeader { inbox, mut future } => {
                    match Pin::new(&mut future).poll(cx) {
                        Poll::Ready(header) => {
                            let future = inbox.read_frame();
                            *this = ReadTwo::WaitingForBody { header, future };
                        }
                        Poll::Pending => {
                            *this = ReadTwo::WaitingForHeader { inbox, future };
                            return Poll::Pending;
                        }
                    }
                }
                ReadTwo::WaitingForBody { header, mut future } => {
                    match Pin::new(&mut future).poll(cx) {
                        Poll::Ready(body) => {
                            return Poll::Ready(format!("{header} | {body}"));
                        }
                        Poll::Pending => {
                            *this = ReadTwo::WaitingForBody { header, future };
                            return Poll::Pending;
                        }
                    }
                }
                ReadTwo::Done => panic!("ReadTwo polled after completion"),
            }
        }
    }
}

// ------------------------------------------------------------- run

const PATIENCE: Duration = Duration::from_secs(5);

fn size_line(name: &str, bytes: usize, unpin: bool) {
    let unpin = if unpin { "Unpin" } else { "!Unpin" };
    println!("  {name:<28} {bytes:>5} bytes  {unpin}");
}

pub fn run() {
    let inbox = Inbox::new();

    // 1. What a future has to carry decides how big it is.
    println!("1. the size of a future, measured");
    let a = keeps_nothing(&inbox);
    let b = buffer_before_the_await(&inbox);
    let c = buffer_across_the_await(&inbox);
    let d = borrows_its_own_local(&inbox);
    let e = outer(&inbox);
    size_line("keeps_nothing", mem::size_of_val(&a), is_unpin_value!(a));
    size_line(
        "buffer_before_the_await",
        mem::size_of_val(&b),
        is_unpin_value!(b),
    );
    size_line(
        "buffer_across_the_await",
        mem::size_of_val(&c),
        is_unpin_value!(c),
    );
    size_line(
        "borrows_its_own_local",
        mem::size_of_val(&d),
        is_unpin_value!(d),
    );
    size_line(
        "outer (awaits the third)",
        mem::size_of_val(&e),
        is_unpin_value!(e),
    );
    drop((a, b, c, d, e));

    // 2. The async fn and the hand-written machine, side by side.
    println!("\n2. read_two: the compiler's future and ours");
    let by_compiler = read_two(&inbox);
    let by_hand = ReadTwo::new(&inbox);
    let compiler_unpin = is_unpin_value!(by_compiler);
    size_line(
        "async fn read_two",
        mem::size_of_val(&by_compiler),
        compiler_unpin,
    );
    size_line(
        "enum ReadTwo",
        mem::size_of_val(&by_hand),
        is_unpin_value!(by_hand),
    );

    let network = inbox.feed(&["HEADER", "BODY"]);
    let first: Report<String> = block_on_for(by_compiler, PATIENCE);
    network.join().unwrap();
    let network = inbox.feed(&["HEADER", "BODY"]);
    let second: Report<String> = block_on_for(by_hand, PATIENCE);
    network.join().unwrap();
    println!("  async fn: {first:?}");
    println!("  by hand:  {second:?}");
    println!("  same answer, same polls, same wakes: {}", first == second);

    // 3. Until its first poll, a future is an ordinary value.
    let future = borrows_its_own_local(&inbox);
    let moved_once = future;
    let boxed = Box::new(moved_once);
    let mut shelf = vec![boxed];
    shelf.reserve(1024);
    let future = *shelf.pop().expect("one future");
    let network = inbox.feed(&["PING"]);
    let report = block_on_for(future, PATIENCE);
    network.join().unwrap();
    println!("\n3. moved four times BEFORE its first poll, then run");
    println!("  {report:?}");
}

#[cfg(test)]
mod tests;
