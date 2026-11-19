// INTENTIONALLY INVALID. This file is not part of the build.
// Check it:  rustc --edition 2024 invalid/lesson06_box_without_pin.rs
//
// A `Box<dyn Future>` can be stored. It cannot be run: awaiting needs to
// pin the future, and pinning something whose type is erased would only
// be safe if that type were `Unpin`. The box does not know that it is.
use std::future::Future;
use std::pin::Pin;

async fn ping() -> String {
    String::from("pong")
}

async fn run_it() -> String {
    let job: Box<dyn Future<Output = String>> = Box::new(ping());
    job.await
}

fn pin_it_by_hand() {
    let mut job: Box<dyn Future<Output = String>> = Box::new(ping());
    let _pinned = Pin::new(&mut *job);
}

fn main() {
    drop(run_it());
    pin_it_by_hand();
}
