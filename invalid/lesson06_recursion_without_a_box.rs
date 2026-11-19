// INTENTIONALLY INVALID. This file is not part of the build.
// Check:  rustc --edition 2024 invalid/lesson06_recursion_without_a_box.rs
//
// The future of `countdown` would have to contain the future of
// `countdown`, which contains the future of `countdown`... A type cannot
// contain itself. A pointer to itself, it can.
async fn countdown(from: u32) -> u32 {
    if from == 0 {
        return 0;
    }
    1 + countdown(from - 1).await
}

fn main() {
    drop(countdown(3));
}
