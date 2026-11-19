// INTENTIONALLY INVALID. This file is not part of the build.
// Check:  rustc --edition 2024 invalid/lesson08_async_future_needs_pinning.rs
//
// Two everyday ways of meeting "cannot be unpinned" with a future that
// came from `async`. Both try to use the future through a plain `&mut`.
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

async fn work() -> u32 {
    7
}

/// 1. Polling by hand without pinning first.
fn poll_it() -> Poll<u32> {
    let mut future = work();
    let mut context = Context::from_waker(Waker::noop());
    Pin::new(&mut future).poll(&mut context)
}

/// 2. Awaiting through `&mut`, to keep the future for later.
async fn await_by_reference() -> u32 {
    let mut future = work();
    (&mut future).await
}

fn main() {
    let _ = poll_it();
    drop(await_by_reference());
}
