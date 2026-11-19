//! Lesson 1 · What a move really is.
//!
//! One `Frame` is moved five ways. After every move we ask two questions:
//! where is the struct now, and where are the bytes of its text?
//!
//! Run it:  cargo run -- 1
use std::mem;

/// One frame of data read from a connection: the type this whole course
/// is built around. Nothing in it points at itself. Remember that.
#[derive(Debug)]
pub struct Frame {
    pub id: u32,
    pub payload: String,
}

impl Frame {
    pub fn new(id: u32, payload: &str) -> Frame {
        Frame {
            id,
            payload: payload.to_string(),
        }
    }
}

/// Where a `Frame` is, at one moment: four addresses, as plain numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Location {
    /// The address of the struct itself.
    pub frame: usize,
    /// The address of its `payload` field: the String's three words
    /// (pointer, capacity, length). They are INSIDE the struct.
    pub payload_field: usize,
    /// The address of its `id` field, also inside the struct.
    pub id_field: usize,
    /// The address of the text. The String keeps it on the heap.
    pub text: usize,
}

/// Reads the four addresses. Taking an address is safe: we only look
/// at the number, we never read through it.
pub fn locate(frame: &Frame) -> Location {
    Location {
        frame: std::ptr::from_ref(frame).addr(),
        payload_field: std::ptr::from_ref(&frame.payload).addr(),
        id_field: std::ptr::from_ref(&frame.id).addr(),
        text: frame.payload.as_ptr().addr(),
    }
}

/// Takes the frame by value: the caller's frame moves into `frame`.
fn into_function(frame: Frame) -> (Frame, Location) {
    let here = locate(&frame);
    (frame, here)
}

fn show(what: &str, at: Location, before: Location) {
    let moved = |a: usize, b: usize| if a == b { "same" } else { "MOVED" };
    println!("{what}");
    let line = |name: &str, now: usize, was: usize| {
        println!("  {name:<14} {now:#014x}  {}", moved(now, was));
    };
    line("struct", at.frame, before.frame);
    line("payload field", at.payload_field, before.payload_field);
    line("id field", at.id_field, before.id_field);
    line("text", at.text, before.text);
}

pub fn run() {
    println!("size of Frame: {} bytes", mem::size_of::<Frame>());
    println!("size of String: {} bytes\n", mem::size_of::<String>());

    // 0. Born on the stack.
    let frame = Frame::new(7, "GET /health");
    let start = locate(&frame);
    show("0. created in a local variable", start, start);
    let payload_at = start.payload_field - start.frame;
    let id_at = start.id_field - start.frame;
    println!(
        "  payload is {payload_at} bytes into the struct, id is {id_at}\n"
    );

    // 1. Moved into a function, and handed back.
    let (frame, inside) = into_function(frame);
    show("1. moved into a function", inside, start);

    // 2. Moved into a Box: the struct itself now lives on the heap.
    let boxed = Box::new(frame);
    let in_box = locate(&boxed);
    show("\n2. moved into a Box", in_box, inside);

    // 3. Moved out of the Box, into a Vec.
    let mut frames = Vec::with_capacity(1);
    frames.push(*boxed);
    let in_vec = locate(&frames[0]);
    show("\n3. moved into a Vec", in_vec, in_box);

    // 4. The Vec grows. We do not touch frames[0], and it moves anyway.
    frames.reserve(1024);
    let after_growth = locate(&frames[0]);
    show("\n4. the Vec grew", after_growth, in_vec);

    // 5. Two frames trade places.
    frames.push(Frame::new(8, "GET /items"));
    let (first, second) = frames.split_at_mut(1);
    mem::swap(&mut first[0], &mut second[0]);
    let swapped = locate(&frames[1]);
    show("\n5. swapped with its neighbour", swapped, after_growth);

    println!(
        "\nfive moves, and the text never moved: {}",
        swapped.text == start.text
    );
    println!("frame {} still reads {:?}", frames[1].id, frames[1].payload);
}

#[cfg(test)]
mod tests;
