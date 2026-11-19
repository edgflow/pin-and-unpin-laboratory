//! INTENTIONALLY INVALID. Not part of the build.
//!
//! One timer, used in every turn of a loop through `&mut`. Without
//! `tokio::pin!(quiet)` the `&mut Sleep` is not a future: `Sleep` is
//! `!Unpin`, and `&mut F` is a future only when `F: Unpin`.
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut quiet = sleep(Duration::from_millis(300));
    let mut turns = 0;
    loop {
        tokio::select! {
            () = sleep(Duration::from_millis(50)) => turns += 1,
            () = &mut quiet => break,
        }
    }
    println!("{turns}");
}
