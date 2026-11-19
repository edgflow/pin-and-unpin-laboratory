//! Every lesson is a module with a `run`.
//!
//!   cargo run -- 1      runs lesson 1
//!   cargo run           lists the lessons
use pin_and_unpin_laboratory::{lesson01_moves, lesson02_self_referential};

const LESSONS: [(&str, &str, fn()); 2] = [
    ("1", "What a move really is", lesson01_moves::run),
    (
        "2",
        "A struct that points at itself",
        lesson02_self_referential::run,
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
