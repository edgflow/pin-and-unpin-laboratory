// INTENTIONALLY INVALID. This file is not part of the build.
// Check it yourself:  rustc --edition 2024 invalid/lesson01_use_after_move.rs
//
// A move leaves the old variable's bytes where they were. What makes a
// move safe is that the compiler refuses to let us look at them again.
struct Frame {
    id: u32,
    payload: String,
}

fn main() {
    let frame = Frame { id: 7, payload: String::from("GET /health") };
    let moved = frame;
    println!("{} {}", frame.id, frame.payload);
    println!("{}", moved.id);
}
