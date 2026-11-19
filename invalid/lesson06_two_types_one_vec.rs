// INTENTIONALLY INVALID. This file is not part of the build.
// Check it:  rustc --edition 2024 invalid/lesson06_two_types_one_vec.rs
//
// Two async functions with the same signature return two different types.
// A Vec holds one type.
async fn ping() -> String {
    String::from("pong")
}

async fn close() -> String {
    String::from("closed")
}

fn main() {
    let jobs = vec![ping(), close()];
    println!("{}", jobs.len());
}
