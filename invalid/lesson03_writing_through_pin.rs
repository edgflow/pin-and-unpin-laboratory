// INTENTIONALLY INVALID. This file is not part of the build.
// Check it:  rustc --edition 2024 invalid/lesson03_writing_through_pin.rs
//
// `Pin<Box<Frame>>` can be read through like a `Box` (it implements
// `Deref`). It cannot be written through or moved out of: for a type that
// is not `Unpin` there is no `DerefMut`.
use std::marker::PhantomPinned;
use std::pin::Pin;

struct Frame {
    id: u32,
    _address_sensitive: PhantomPinned,
}

fn frame(id: u32) -> Pin<Box<Frame>> {
    Box::pin(Frame { id, _address_sensitive: PhantomPinned })
}

fn main() {
    let mut a = frame(1);
    let b = frame(2);

    // 1. Assign to a field through the pin.
    a.id = 9;

    // 2. Move the frame out by dereferencing.
    let moved: Frame = *b;
    let _ = (a.id, moved.id);
}
