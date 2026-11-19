//! A crank for futures: just enough to run one, and to count the turns.
//!
//! Lesson 6 needs to run futures before lesson 7 has explained `poll`
//! and wakers. So this module is a deliberately crude stand-in: it polls
//! in a loop until the future is done. Lesson 7 builds the real thing.
use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

/// Polls `future` until it is ready. Returns its output and how many
/// times it had to be polled.
///
/// It takes the future ALREADY PINNED: `Pin<&mut F>`. That is the only
/// form `Future::poll` accepts.
pub fn turn<F: Future + ?Sized>(mut future: Pin<&mut F>) -> (F::Output, u32) {
    let mut context = Context::from_waker(Waker::noop());
    let mut polls = 0;
    loop {
        polls += 1;
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return (output, polls);
        }
    }
}

/// A future that is not ready the first time it is asked, and ready the
/// second time. Awaiting it gives the caller back one turn of the crank.
pub async fn pause() {
    let mut asked_before = false;
    poll_fn(|_| {
        if asked_before {
            Poll::Ready(())
        } else {
            asked_before = true;
            Poll::Pending
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::pin;

    #[test]
    fn a_ready_future_takes_one_turn() {
        let future = pin!(async { 7 });
        assert_eq!(turn(future), (7, 1));
    }

    #[test]
    fn every_pause_costs_one_more_turn() {
        let future = pin!(async {
            pause().await;
            pause().await;
            "done"
        });
        assert_eq!(turn(future), ("done", 3));
    }
}
