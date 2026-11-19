// INTENTIONALLY INVALID. This file is not part of the build.
// Check it:  rustc --edition 2024 invalid/lesson02_reference_to_self.rs
//
// The first thing everyone tries: a real reference to the struct's own
// field. The borrow checker cannot accept it, and its two errors say why:
// returning the frame is a move, and a move would leave the reference
// pointing at the old place.
struct Frame<'a> {
    payload: String,
    cursor: &'a String,
}

fn build<'a>() -> Frame<'a> {
    let payload = String::from("GET /health");
    Frame { cursor: &payload, payload }
}

fn main() {
    let frame = build();
    println!("{} {}", frame.payload, frame.cursor);
}
