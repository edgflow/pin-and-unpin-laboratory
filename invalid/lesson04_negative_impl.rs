// INTENTIONALLY INVALID on stable Rust. Not part of the build.
// Check it:  rustc --edition 2024 invalid/lesson04_negative_impl.rs
//
// The direct way to say "this type is not Unpin" would be a negative
// implementation. Stable Rust does not have that syntax. Lesson 5 shows
// what stable code uses instead.
struct Frame {
    id: u32,
}

impl !Unpin for Frame {}

fn main() {
    println!("{}", Frame { id: 1 }.id);
}
