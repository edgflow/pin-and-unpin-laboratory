//! Every lesson is a module with a `run`.
//!
//!   cargo run -- 1      runs lesson 1
//!   cargo run           lists the lessons
use pin_and_unpin_laboratory as lab;

type Lesson = (&'static str, &'static str, fn());

const LESSONS: &[Lesson] = &[
    ("1", "What a move really is", lab::lesson01_moves::run),
    (
        "2",
        "A struct that points at itself",
        lab::lesson02_self_referential::run,
    ),
    (
        "3",
        "Pin: a promise about the pointee",
        lab::lesson03_pin::run,
    ),
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
