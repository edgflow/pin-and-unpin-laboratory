//! A timer, as a hand-written future.
//!
//!   cargo run --example custom_future
//!
//! `Delay` is `Pending` until its time has come. On its first poll it
//! starts one thread that sleeps and then calls the waker. It shows the
//! three duties of a `poll`: check, register the waker, report.
use pin_and_unpin_laboratory::lesson07_future_poll::block_on_for;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

struct Delay {
    until: Instant,
    /// Shared with the timer thread, which wakes whatever is in here.
    waker: Arc<Mutex<Option<Waker>>>,
    timer_started: bool,
}

impl Delay {
    fn new(wait: Duration) -> Delay {
        Delay {
            until: Instant::now() + wait,
            waker: Arc::default(),
            timer_started: false,
        }
    }
}

impl Future for Delay {
    type Output = Duration;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Duration> {
        // 1. Check.
        let now = Instant::now();
        if now >= self.until {
            return Poll::Ready(now - self.until);
        }
        // 2. Register: always the waker of THIS poll.
        *self.waker.lock().unwrap() = Some(cx.waker().clone());
        if !self.timer_started {
            self.timer_started = true;
            let (until, waker) = (self.until, Arc::clone(&self.waker));
            thread::spawn(move || {
                thread::sleep(until.saturating_duration_since(Instant::now()));
                if let Some(waker) = waker.lock().unwrap().take() {
                    waker.wake();
                }
            });
        }
        // 3. Report.
        Poll::Pending
    }
}

fn main() {
    let started = Instant::now();
    let patience = Duration::from_secs(5);
    let report = block_on_for(Delay::new(Duration::from_millis(150)), patience);
    let waited = started.elapsed().as_millis();
    println!("waited about {} ms", waited / 50 * 50);
    println!("polls: {}, wakes: {}", report.polls, report.wakes);
    println!("finished: {}", report.output.is_some());
}
