// INTENTIONALLY INVALID. This file is not part of the build.
// Check it:  rustc --edition 2024 invalid/lesson06_not_send.rs
//
// `BoxFuture` promises `Send`. A future that keeps an `Rc` alive across
// an `.await` is not `Send`, so it cannot become a `BoxFuture`.
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

async fn pause() {}

fn main() {
    let shared = Rc::new(String::from("not thread-safe"));
    let job: BoxFuture<'static, usize> = Box::pin(async move {
        pause().await;
        shared.len()
    });
    drop(job);
}
