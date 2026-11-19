//! Every lesson is a module with a `run`.
//!
//!   cargo run -- 1      runs lesson 1
//!   cargo run           lists the lessons
use pin_and_unpin_laboratory as lab;

use lab::lesson01_moves as l1;
use lab::lesson02_self_referential as l2;
use lab::lesson03_pin as l3;
use lab::lesson04_unpin as l4;
use lab::lesson05_phantom_pinned as l5;
use lab::lesson06_pin_box as l6;
use lab::lesson07_future_poll as l7;
use lab::lesson08_async_state_machine as l8;

type Lesson = (&'static str, &'static str, fn());

const LESSONS: &[Lesson] = &[
    ("1", "What a move really is", l1::run),
    ("2", "A struct that points at itself", l2::run),
    ("3", "Pin: a promise about the pointee", l3::run),
    ("4", "Unpin and !Unpin", l4::run),
    ("5", "PhantomPinned", l5::run),
    ("6", "Pin<Box<T>> and BoxFuture", l6::run),
    ("7", "Future::poll, Pending, Ready, Waker", l7::run),
    ("8", "The async state machine", l8::run),
];

fn main() {
    let wanted = std::env::args().nth(1);
    let found = LESSONS.iter().find(|(n, ..)| Some(*n) == wanted.as_deref());
    match found {
        Some((_, _, run)) => run(),
        None => {
            println!("usage: cargo run -- <lesson>\n");
            for (number, title, _) in LESSONS {
                println!("  {number:>2}  {title}");
            }
        }
    }
}
