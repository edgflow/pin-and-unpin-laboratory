//! Lesson 2 · A struct that points at itself.
//!
//! The `Frame` gets a cursor: a pointer to its own `payload` field. It
//! compiles, it works, and one move later it points at the wrong place.
//! We never read through a stale pointer. We only compare addresses.
//!
//! Run it:  cargo run -- 2
use std::{mem, ptr};

/// A frame with a cursor into itself. Compare lesson 1: one new field.
#[derive(Debug)]
pub struct Frame {
    pub id: u32,
    pub payload: String,
    /// The address of `payload`, the field right above this one.
    /// Null until `link` is called.
    cursor: *const String,
}

impl Frame {
    /// A frame whose cursor points nowhere yet.
    pub fn new(id: u32, payload: &str) -> Frame {
        Frame {
            id,
            payload: payload.to_string(),
            cursor: ptr::null(),
        }
    }

    /// Aims the cursor at this frame's own payload field, where the
    /// frame is RIGHT NOW. The pointer is a number. It will not follow.
    pub fn link(&mut self) {
        self.cursor = ptr::from_ref(&self.payload);
    }

    /// Where the cursor says the payload is.
    pub fn cursor_says(&self) -> usize {
        self.cursor.addr()
    }

    /// Where the payload field really is, at this moment.
    pub fn payload_is_at(&self) -> usize {
        ptr::from_ref(&self.payload).addr()
    }

    /// Does the cursor still point at this frame's own payload?
    /// An address comparison. Nothing is read through the cursor.
    pub fn points_at_itself(&self) -> bool {
        self.cursor_says() == self.payload_is_at()
    }
}

/// The same idea with a cursor into the TEXT, on the heap, not into the
/// struct. A move of the struct does not touch the text (lesson 1).
pub struct TextCursor {
    pub payload: String,
    first_byte: *const u8,
}

impl TextCursor {
    pub fn new(payload: &str) -> TextCursor {
        let payload = payload.to_string();
        let first_byte = payload.as_ptr();
        TextCursor {
            payload,
            first_byte,
        }
    }

    pub fn points_at_its_text(&self) -> bool {
        self.first_byte.addr() == self.payload.as_ptr().addr()
    }
}

/// The cursor as a position, not an address: "the payload starts at
/// byte 4". A position means the same thing wherever the frame is.
pub struct OffsetFrame {
    pub line: String,
    payload_starts: usize,
}

impl OffsetFrame {
    /// Splits "GET /health" after the first space.
    pub fn parse(line: &str) -> OffsetFrame {
        let payload_starts = line.find(' ').map_or(0, |space| space + 1);
        OffsetFrame {
            line: line.to_string(),
            payload_starts,
        }
    }

    pub fn payload(&self) -> &str {
        &self.line[self.payload_starts..]
    }
}

fn report(what: &str, frame: &Frame) {
    println!("{what}");
    println!("  payload field is at  {:#014x}", frame.payload_is_at());
    println!("  cursor says          {:#014x}", frame.cursor_says());
    println!("  points at itself:    {}", frame.points_at_itself());
}

pub fn run() {
    println!("size of Frame: {} bytes\n", mem::size_of::<Frame>());

    // 1. Link it where it was born. Everything is fine.
    let mut frame = Frame::new(7, "GET /health");
    frame.link();
    report("1. linked in a local variable", &frame);

    // 2. One move. The bytes are copied, the cursor among them.
    let boxed = Box::new(frame);
    report("\n2. moved into a Box", &boxed);
    println!("  the cursor still holds the OLD address of the field");

    // 3. A cursor into the text survives the same move.
    let text = TextCursor::new("GET /health");
    let before = text.points_at_its_text();
    let text = Box::new(text);
    println!("\n3. a cursor into the text, moved the same way");
    println!("  before {before}, after {}", text.points_at_its_text());

    // 4. A cursor that is a position has nothing to go stale.
    let parsed = Box::new(OffsetFrame::parse("GET /health"));
    println!("\n4. a cursor that is a position, moved the same way");
    println!("  payload: {:?}", parsed.payload());

    // 5. Box first, link second. The frame is linked where it will stay.
    let mut first = Box::new(Frame::new(1, "first"));
    first.link();
    report("\n5. boxed first, then linked", &first);
    let first_moved = first;
    println!("  after moving the box: {}", first_moved.points_at_itself());

    // 6. And still: anyone with &mut Frame can move the frame itself.
    let mut a = first_moved;
    let mut b = Box::new(Frame::new(2, "second"));
    b.link();
    mem::swap(&mut *a, &mut *b);
    println!("\n6. the contents of two boxes swapped");
    println!("  a points at itself: {}", a.points_at_itself());
    println!("  b points at itself: {}", b.points_at_itself());
    let crossed = a.cursor_says() == b.payload_is_at();
    println!("  a's cursor points into b: {crossed}");
}

#[cfg(test)]
mod tests;
