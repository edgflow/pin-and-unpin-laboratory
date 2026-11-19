// INTENTIONALLY INVALID. This file is not part of the build.
// Check it:  rustc --edition 2024 invalid/lesson03_moving_out_of_pin.rs
//
// Two ordinary ways to get a value or `&mut` to it back out of a pointer.
// Through a `Pin` of an address-sensitive type, neither compiles: both
// ask for `Unpin`. (Two more ways are in lesson03_writing_through_pin.rs.
// They fail in a later phase of the compiler, so they need their own file.)
use std::marker::PhantomPinned;
use std::mem;
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
    let mut b = frame(2);

    // 1. Swap the two frames.
    mem::swap(a.as_mut().get_mut(), b.as_mut().get_mut());

    // 2. Take the frame back out of its pin.
    let loose: Box<Frame> = Pin::into_inner(a);

    let _ = (loose.id, b.id);
}
