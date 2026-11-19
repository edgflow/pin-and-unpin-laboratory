//! pin-and-unpin-laboratory: every lesson is a module with a `run`.
//!
//!   cargo run -- 1      runs lesson 1
//!   cargo run           lists the lessons
mod lesson01_moves;

const LESSONS: [(&str, &str, fn()); 1] =
    [("1", "What a move really is", lesson01_moves::run)];

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
