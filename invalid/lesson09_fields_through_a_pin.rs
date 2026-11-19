// INTENTIONALLY INVALID. This file is not part of the build.
// Check it:  rustc --edition 2024 invalid/lesson09_fields_through_a_pin.rs
//
// The wrapper everybody writes first. Inside `poll`, `self` is
// `Pin<&mut Self>`, and for a generic `F` that gives no `&mut` access to
// the fields at all.
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

struct Timed<F> {
    inner: F,
    polls: u32,
}

impl<F: Future> Future for Timed<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.polls += 1;
        self.inner.poll(cx)
    }
}

fn main() {}
