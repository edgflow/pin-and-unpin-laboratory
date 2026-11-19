//! Lesson 9 · Pin projection.
//!
//! `Timed<F>` wraps a future and counts how often it is polled. Inside
//! its `poll` we hold `Pin<&mut Timed<F>>` and need two different things
//! from it: `Pin<&mut F>` for the inner future, and a plain `&mut u32`
//! for the counter. Getting from a pinned struct to its fields is called
//! projection. This lesson does it four ways.
//!
//! Run it:  cargo run -- 9
use crate::is_unpin_value;
use crate::lesson07_future_poll::{Inbox, block_on_for};
use crate::lesson08_async_state_machine::{borrows_its_own_local, read_two};
use pin_project::pin_project;
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

/// What every wrapper in this lesson returns: the inner future's output
/// and the number of polls it took.
pub type Timing<T> = (T, u32);

// ------------------------------------ 1. demand that the inner is Unpin

/// The easy way out: only accept futures that do not care about moving.
pub struct TimedUnpin<F> {
    inner: F,
    polls: u32,
}

impl<F: Future + Unpin> TimedUnpin<F> {
    pub fn new(inner: F) -> Self {
        TimedUnpin { inner, polls: 0 }
    }
}

impl<F: Future + Unpin> Future for TimedUnpin<F> {
    type Output = Timing<F::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // `F: Unpin` makes the whole struct `Unpin`, so this is safe.
        let this = self.get_mut();
        this.polls += 1;
        let polls = this.polls;
        Pin::new(&mut this.inner)
            .poll(cx)
            .map(|output| (output, polls))
    }
}

// --------------------------------------- 2. put the inner behind a box

/// Accept any future, and pin it on the heap ourselves.
pub struct TimedBoxed<F> {
    inner: Pin<Box<F>>,
    polls: u32,
}

impl<F: Future> TimedBoxed<F> {
    pub fn new(inner: F) -> Self {
        TimedBoxed {
            inner: Box::pin(inner),
            polls: 0,
        }
    }
}

impl<F: Future> Future for TimedBoxed<F> {
    type Output = Timing<F::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // A `Pin<Box<F>>` is `Unpin` whatever `F` is (lesson 4), so the
        // struct is `Unpin` too, and this is safe again.
        let this = self.get_mut();
        this.polls += 1;
        let polls = this.polls;
        this.inner.as_mut().poll(cx).map(|output| (output, polls))
    }
}

// --------------------------------------------- 3. project by hand

/// Accept any future, store it inline, and reach it with `unsafe`.
///
/// PINNING DECISIONS, one per field, and they must never be mixed:
/// - `inner` is STRUCTURALLY PINNED: when a `TimedByHand` is pinned, so
///   is its `inner`. We hand out `Pin<&mut F>` to it and never `&mut F`.
/// - `polls` is NOT structurally pinned: it is a plain number. We hand
///   out `&mut u32` to it and never `Pin<&mut u32>`.
///
/// What we owe for the first decision (the standard library's list):
/// 1. No `impl Unpin` for this type beyond the automatic one, which is
///    "Unpin exactly when `F` is".
/// 2. No `Drop` impl that moves `inner` out. (There is no `Drop` impl.)
/// 3. No `#[repr(packed)]`: a packed field cannot be pinned in place.
/// 4. No method that moves `inner` out of a pinned `Self`, or replaces
///    it without running its destructor.
pub struct TimedByHand<F> {
    inner: F,
    polls: u32,
}

impl<F: Future> TimedByHand<F> {
    pub fn new(inner: F) -> Self {
        TimedByHand { inner, polls: 0 }
    }

    /// The structural projection: pinned struct in, pinned field out.
    fn inner(self: Pin<&mut Self>) -> Pin<&mut F> {
        // SAFETY: `inner` is structurally pinned (see the type's notes).
        // The closure returns a reference into `self`, and `self` will
        // not move for as long as that reference is used.
        unsafe { self.map_unchecked_mut(|this| &mut this.inner) }
    }

    /// The non-structural projection: pinned struct in, plain `&mut` out.
    fn polls(self: Pin<&mut Self>) -> &mut u32 {
        // SAFETY: `polls` is never pinned, and through this `&mut Self`
        // we touch only that field. Nothing is moved out of `inner`.
        unsafe { &mut self.get_unchecked_mut().polls }
    }

    /// Reading needs no `unsafe`: a shared reference cannot move anything.
    /// (It does take `&Self` over the whole struct. Call it before the
    /// first poll or after the last one, not on a parked wrapper that
    /// will be polled again: see lesson 8.)
    pub fn polls_so_far(self: Pin<&Self>) -> u32 {
        self.get_ref().polls
    }
}

impl<F: Future> Future for TimedByHand<F> {
    type Output = Timing<F::Output>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        // `as_mut()` reborrows the pin, so `self` can be used again.
        let counter = self.as_mut().polls();
        *counter += 1;
        let polls = *counter;
        // We read the count through the field's own projection, and NOT
        // through `&Self`. A shared reference to the whole struct would
        // cover the parked inner future too: lesson 8's experiment.
        self.inner().poll(cx).map(|output| (output, polls))
    }
}

// ------------------------------------------ 4. let a macro write it

/// The same wrapper with `pin-project`. `#[pin]` marks the structurally
/// pinned field. The macro writes the projection, and it refuses the
/// things on the list above at compile time.
#[pin_project]
pub struct Timed<F> {
    #[pin]
    inner: F,
    polls: u32,
}

impl<F: Future> Timed<F> {
    pub fn new(inner: F) -> Self {
        Timed { inner, polls: 0 }
    }
}

impl<F: Future> Future for Timed<F> {
    type Output = Timing<F::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // `this.inner` is `Pin<&mut F>`. `this.polls` is `&mut u32`.
        let this = self.project();
        *this.polls += 1;
        let polls = *this.polls;
        this.inner.poll(cx).map(|output| (output, polls))
    }
}

// ----------------------------------- the challenge: projecting an Option

/// Polls its future until it is done, then drops it on the spot and
/// answers `None` to every later poll. Safe to poll after completion.
#[pin_project]
pub struct Fused<F> {
    #[pin]
    inner: Option<F>,
}

impl<F: Future> Fused<F> {
    pub fn new(inner: F) -> Self {
        Fused { inner: Some(inner) }
    }

    pub fn is_finished(&self) -> bool {
        self.inner.is_none()
    }
}

impl<F: Future> Future for Fused<F> {
    type Output = Option<F::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        // Pin<&mut Option<F>>  →  Option<Pin<&mut F>>: a safe projection
        // that the standard library provides for `Option`.
        let Some(future) = this.inner.as_mut().as_pin_mut() else {
            return Poll::Ready(None);
        };
        let output = std::task::ready!(future.poll(cx));
        // Finished. `set` drops the future where it stands (lesson 3).
        this.inner.set(None);
        Poll::Ready(Some(output))
    }
}

// ------------------------------------------------------------- run

const PATIENCE: Duration = Duration::from_secs(5);

fn fact(name: &str, bytes: usize, unpin: bool) {
    let unpin = if unpin { "Unpin" } else { "!Unpin" };
    println!("  {name:<30} {bytes:>4} bytes  {unpin}");
}

pub fn run() {
    let inbox = Inbox::new();

    // 1. The four wrappers around the same kind of future.
    println!("1. four wrappers around read_two's future");
    let bare = read_two(&inbox);
    fact(
        "the future itself",
        mem::size_of_val(&bare),
        is_unpin_value!(bare),
    );
    drop(bare);
    let a = TimedUnpin::new(Box::pin(read_two(&inbox)));
    let b = TimedBoxed::new(read_two(&inbox));
    let c = TimedByHand::new(read_two(&inbox));
    let d = Timed::new(read_two(&inbox));
    fact(
        "TimedUnpin (boxed by caller)",
        mem::size_of_val(&a),
        is_unpin_value!(a),
    );
    fact("TimedBoxed", mem::size_of_val(&b), is_unpin_value!(b));
    fact("TimedByHand", mem::size_of_val(&c), is_unpin_value!(c));
    fact(
        "Timed (pin-project)",
        mem::size_of_val(&d),
        is_unpin_value!(d),
    );

    // 2. All four give the same answer.
    println!("\n2. run each one");
    let network = inbox.feed(&["HEADER", "BODY"]);
    println!("  TimedUnpin   {:?}", block_on_for(a, PATIENCE).output);
    network.join().unwrap();
    let network = inbox.feed(&["HEADER", "BODY"]);
    println!("  TimedBoxed   {:?}", block_on_for(b, PATIENCE).output);
    network.join().unwrap();
    let network = inbox.feed(&["HEADER", "BODY"]);
    println!("  TimedByHand  {:?}", block_on_for(c, PATIENCE).output);
    network.join().unwrap();
    let network = inbox.feed(&["HEADER", "BODY"]);
    println!("  Timed        {:?}", block_on_for(d, PATIENCE).output);
    network.join().unwrap();

    // 3. The projection keeps a self-referential inner future intact.
    let network = inbox.feed(&["PING"]);
    let by_hand = TimedByHand::new(borrows_its_own_local(&inbox));
    let report = block_on_for(by_hand, PATIENCE);
    network.join().unwrap();
    println!("\n3. a self-referential future inside TimedByHand");
    println!("  {:?}", report.output);

    // 4. Reading a field of a pinned value needs no unsafe.
    let pinned = Box::pin(TimedByHand::new(read_two(&inbox)));
    println!("\n4. polls so far, read through Pin<&Self>: {}", {
        pinned.as_ref().polls_so_far()
    });

    // 5. Fused: a wrapper that may be polled after it finished.
    let network = inbox.feed(&["LAST"]);
    let mut fused = Box::pin(Fused::new(inbox.read_frame()));
    let first = block_on_for(fused.as_mut(), PATIENCE).output;
    network.join().unwrap();
    let second = block_on_for(fused.as_mut(), PATIENCE).output;
    println!("\n5. Fused, polled to the end and then again");
    println!("  first:  {first:?}");
    println!("  second: {second:?}");
    println!("  the inner future is gone: {}", fused.is_finished());
}

#[cfg(test)]
mod tests;
