//! Where can a value live so that its address stays the same?
//!
//!   cargo run --example address_stability
//!
//! Each row puts a `Frame` somewhere, does one ordinary thing to its
//! owner, and reports whether the frame itself is still at the same
//! address. Addresses are compared, never read through.
use pin_and_unpin_laboratory::lesson02_self_referential::Frame;
use std::mem;

fn row(place: &str, action: &str, before: usize, after: usize) {
    let verdict = if before == after { "stayed" } else { "MOVED" };
    println!("{place:<22} {action:<26} {verdict}");
}

fn pass_along(frame: Frame) -> Frame {
    frame
}

fn main() {
    println!(
        "{:<22} {:<26} the frame",
        "the frame lives in", "what we did"
    );

    // A local variable, handed to a function and back.
    let frame = Frame::new(1, "a");
    let before = frame.payload_is_at();
    let frame = pass_along(frame);
    row(
        "a local variable",
        "passed it to a function",
        before,
        frame.payload_is_at(),
    );

    // An element of a Vec that has to grow.
    let mut frames = Vec::with_capacity(1);
    frames.push(frame);
    let before = frames[0].payload_is_at();
    frames.reserve(4096);
    row(
        "a Vec<Frame>",
        "the Vec grew",
        before,
        frames[0].payload_is_at(),
    );

    // A Box, moved as a box.
    let boxed = Box::new(Frame::new(2, "b"));
    let before = boxed.payload_is_at();
    let boxed = [boxed];
    row(
        "a Box<Frame>",
        "moved the box",
        before,
        boxed[0].payload_is_at(),
    );

    // A Vec of boxes that has to grow: the boxes move, the frames stay.
    let mut boxes = Vec::with_capacity(1);
    boxes.push(Box::new(Frame::new(3, "c")));
    let before = boxes[0].payload_is_at();
    boxes.reserve(4096);
    row(
        "a Vec<Box<Frame>>",
        "the Vec grew",
        before,
        boxes[0].payload_is_at(),
    );

    // A Box again, but this time we reach inside with &mut.
    let mut left = Box::new(Frame::new(4, "d"));
    let mut right = Box::new(Frame::new(5, "e"));
    let before = left.payload_is_at();
    let id_before = left.id;
    mem::swap(&mut *left, &mut *right);
    // The box still points at the same place, but frame 4 is not there.
    let frame_four = if left.id == id_before { &left } else { &right };
    row(
        "a Box<Frame>",
        "swapped the contents",
        before,
        frame_four.payload_is_at(),
    );
}
