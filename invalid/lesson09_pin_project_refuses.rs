// INTENTIONALLY INVALID. Not part of the build: it needs the crate's
// dependencies, so check it by copying it into examples/ for a moment:
//   cp invalid/lesson09_pin_project_refuses.rs examples/refuses.rs
//   cargo check --example refuses ; rm examples/refuses.rs
//
// Two items from the list of things a structurally pinned field forbids.
// Written by hand, both compile and are unsound. `pin-project` turns
// them into compile errors.
use pin_project::pin_project;

/// 1. A `Drop` impl gets `&mut self`, and could move `inner` out.
#[pin_project]
struct WithDrop<F> {
    #[pin]
    inner: F,
}

impl<F> Drop for WithDrop<F> {
    fn drop(&mut self) {}
}

/// 2. A packed struct cannot keep a field at a stable, aligned place.
#[pin_project]
#[repr(packed)]
struct Packed<F> {
    #[pin]
    inner: F,
    polls: u32,
}

fn main() {}
