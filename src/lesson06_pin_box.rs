//! Lesson 6 · `Pin<Box<T>>` and `BoxFuture`.
//!
//! A connection can do several things: read a frame, send a ping, close.
//! Each is an `async fn`, and each `async fn` returns a future of its own
//! secret type. To keep them in one list we must erase those types, and
//! to run them afterwards the box has to be pinned.
//!
//! Run it:  cargo run -- 6
use crate::crank::{pause, turn};
use crate::is_unpin_value;
use std::future::Future;
use std::mem;
use std::pin::Pin;

/// A future of any type, on the heap, that yields a `T`.
///
/// - `dyn Future<Output = T>`: the concrete type is erased.
/// - `Box<…>`: it lives on the heap, behind one pointer.
/// - `Pin<…>`: and it will not be moved out of that box.
/// - `Send`: it may be handed to another thread.
/// - `'a`: it may borrow data that lives at least as long as `'a`.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Reads one frame. Two pauses stand in for waiting on the network.
pub async fn read_frame(id: u32) -> String {
    let mut buffer = [0u8; 64];
    pause().await;
    buffer[0] = b'G';
    pause().await;
    format!("frame {id}, first byte {:?}", buffer[0] as char)
}

/// Sends a ping and waits once for the answer.
pub async fn ping() -> String {
    pause().await;
    String::from("pong")
}

/// Closes the connection. Nothing to wait for.
pub async fn close() -> String {
    String::from("closed")
}

/// A boxed future that borrows: it may not outlive `text`.
pub fn length_later<'a>(text: &'a str) -> BoxFuture<'a, usize> {
    Box::pin(async move {
        pause().await;
        text.len()
    })
}

/// A recursive async function. Its future would have to contain itself,
/// so each level goes into a box of its own.
pub fn countdown(from: u32) -> BoxFuture<'static, u32> {
    Box::pin(async move {
        if from == 0 {
            return 0;
        }
        pause().await;
        1 + countdown(from - 1).await
    })
}

/// One operation on a connection, usable as a trait object. A trait
/// object cannot have a method that returns "some future type", so the
/// method returns the erased, boxed, pinned one.
pub trait Operation {
    fn name(&self) -> &'static str;
    fn start(&self) -> BoxFuture<'_, String>;
}

pub struct Read(pub u32);
pub struct Ping;
pub struct Close;

impl Operation for Read {
    fn name(&self) -> &'static str {
        "read"
    }
    fn start(&self) -> BoxFuture<'_, String> {
        Box::pin(read_frame(self.0))
    }
}

impl Operation for Ping {
    fn name(&self) -> &'static str {
        "ping"
    }
    fn start(&self) -> BoxFuture<'_, String> {
        Box::pin(ping())
    }
}

impl Operation for Close {
    fn name(&self) -> &'static str {
        "close"
    }
    fn start(&self) -> BoxFuture<'_, String> {
        Box::pin(close())
    }
}

/// The address of the future inside the box, without its vtable half.
fn inside<T: ?Sized>(boxed: &Pin<Box<T>>) -> usize {
    std::ptr::from_ref::<T>(&**boxed).cast::<()>().addr()
}

pub fn run() {
    // 1. Three async fns, three different types.
    let (read, ping_once, close_now) = (read_frame(7), ping(), close());
    println!("1. three futures, three types");
    for (name, size, unpin) in [
        ("read_frame", mem::size_of_val(&read), is_unpin_value!(read)),
        (
            "ping",
            mem::size_of_val(&ping_once),
            is_unpin_value!(ping_once),
        ),
        (
            "close",
            mem::size_of_val(&close_now),
            is_unpin_value!(close_now),
        ),
    ] {
        let unpin = if unpin { "Unpin" } else { "!Unpin" };
        println!("  {name:<12} {size:>4} bytes  {unpin}");
    }

    // 2. Erased, boxed and pinned: one type, one size.
    let mut jobs: Vec<BoxFuture<'static, String>> =
        vec![Box::pin(read), Box::pin(ping_once), Box::pin(close_now)];
    println!("\n2. the same three as BoxFuture<String>");
    println!("  each is {} bytes", mem::size_of::<BoxFuture<String>>());
    println!("  and is Unpin: {}", is_unpin_value!(jobs[0]));

    // 3. Moving the boxes around does not move the futures inside.
    let before: Vec<usize> = jobs.iter().map(inside).collect();
    jobs.reverse();
    jobs.reserve(4096);
    let mut after: Vec<usize> = jobs.iter().map(inside).collect();
    after.reverse();
    println!("\n3. reversed the list and grew it");
    println!("  every future stayed where it was: {}", before == after);

    // 4. One loop, three different pieces of code: dynamic dispatch.
    println!("\n4. run them, last to first");
    for job in &mut jobs {
        let (answer, polls) = turn(job.as_mut());
        println!("  {answer:?} after {polls} poll(s)");
    }

    // 5. A boxed future that borrows a local.
    let line = String::from("GET /health");
    let (length, _) = turn(length_later(&line).as_mut());
    println!("\n5. a BoxFuture that borrows: length {length}");

    // 6. Recursion: one box per level.
    let (count, polls) = turn(countdown(3).as_mut());
    println!("\n6. countdown(3) = {count}, after {polls} polls");

    // 7. The same idea behind a trait object.
    let operations: Vec<Box<dyn Operation>> =
        vec![Box::new(Read(9)), Box::new(Ping), Box::new(Close)];
    println!("\n7. operations as trait objects");
    for operation in &operations {
        let (answer, _) = turn(operation.start().as_mut());
        println!("  {:<6} {answer:?}", operation.name());
    }
}

#[cfg(test)]
mod tests;
