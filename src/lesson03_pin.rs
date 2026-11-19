//! Lesson 3 · `Pin<P>`: a promise about the thing a pointer points at.
//!
//! Lesson 2 ended with a need: a pointer to a `Frame` that lets us use the
//! frame and does not hand out the unrestricted `&mut Frame` that every
//! move goes through. That pointer is `Pin`.
//!
//! Run it:  cargo run -- 3
use crate::lesson02_self_referential::Frame as LooseFrame;
use std::marker::PhantomPinned;
use std::pin::{Pin, pin};
use std::{mem, ptr};

/// The address-sensitive frame, done properly.
///
/// INVARIANT: `cursor` is either null, or it points at the `payload`
/// field of this very frame, and this frame is pinned. Only `link` sets
/// it, and `link` can only be called on a pinned frame.
pub struct Frame {
    pub id: u32,
    payload: String,
    cursor: *const String,
    /// Says: "my address matters". Lessons 4 and 5 explain how.
    _address_sensitive: PhantomPinned,
}

impl Frame {
    /// A frame that does not point at itself yet. Moving it is harmless:
    /// a null cursor is the same null cursor everywhere.
    pub fn unlinked(id: u32, payload: &str) -> Frame {
        Frame {
            id,
            payload: payload.to_string(),
            cursor: ptr::null(),
            _address_sensitive: PhantomPinned,
        }
    }

    /// Aims the cursor at this frame's own payload. The receiver is
    /// `Pin<&mut Self>`: a caller can only get here with a frame that
    /// has been pinned, which is a promise that it will not move again.
    pub fn link(self: Pin<&mut Self>) {
        // SAFETY: `get_unchecked_mut` hands us `&mut Frame`, and with it
        // the power to move the frame. We promise not to use that power:
        // we write one field in place and move nothing out.
        let this = unsafe { self.get_unchecked_mut() };
        this.cursor = ptr::from_ref(&this.payload);
    }

    /// Boxed, pinned and linked, in that order. Entirely safe to call.
    pub fn pinned(id: u32, payload: &str) -> Pin<Box<Frame>> {
        let mut frame = Box::pin(Frame::unlinked(id, payload));
        frame.as_mut().link();
        frame
    }

    /// Reads the payload THROUGH the cursor. This is the read that was
    /// undefined behaviour waiting to happen in lesson 2.
    pub fn payload(self: Pin<&Self>) -> Option<&str> {
        if self.cursor.is_null() {
            return None;
        }
        // SAFETY: by the invariant, a non-null cursor was set by `link`
        // on this frame while it was pinned. A pinned frame is not moved
        // and its memory is not reused before it is dropped, so the
        // cursor still points at this frame's own, live `payload`.
        let payload: &String = unsafe { &*self.cursor };
        Some(payload.as_str())
    }

    pub fn cursor_says(&self) -> usize {
        self.cursor.addr()
    }

    pub fn payload_is_at(&self) -> usize {
        ptr::from_ref(&self.payload).addr()
    }

    pub fn points_at_itself(&self) -> bool {
        self.cursor_says() == self.payload_is_at()
    }
}

pub fn run() {
    // 1. Pin costs nothing at run time: it is the same pointer.
    println!("size of Box<Frame>:      {}", mem::size_of::<Box<Frame>>());
    println!(
        "size of Pin<Box<Frame>>: {}",
        mem::size_of::<Pin<Box<Frame>>>()
    );

    // 2. A frame that was born pinned.
    let frame = Frame::pinned(7, "GET /health");
    println!("\n2. Frame::pinned");
    println!("  points at itself: {}", frame.points_at_itself());
    println!("  read through the cursor: {:?}", frame.as_ref().payload());
    println!("  read a field directly: id {}", frame.id);

    // 3. The Pin<Box<_>> moves like any pointer. The frame does not.
    let before = frame.payload_is_at();
    let mut shelf = Vec::with_capacity(1);
    shelf.push(frame);
    shelf.reserve(4096);
    let frame = shelf.pop().expect("one frame");
    println!("\n3. the Pin<Box<Frame>> moved into a Vec, the Vec grew");
    println!("  the frame stayed: {}", frame.payload_is_at() == before);
    println!("  points at itself: {}", frame.points_at_itself());
    println!("  read through the cursor: {:?}", frame.as_ref().payload());

    // 4. Pinned on the stack, borrowed instead of owned.
    let mut local = pin!(Frame::unlinked(8, "GET /items"));
    println!("\n4. pinned in a local with pin!");
    println!("  before link: {:?}", local.as_ref().payload());
    local.as_mut().link();
    println!("  after link:  {:?}", local.as_ref().payload());
    println!("  points at itself: {}", local.points_at_itself());

    // 5. And the frame of lesson 2, inside the very same Pin<Box<_>>.
    let mut a = Box::pin(LooseFrame::new(1, "first"));
    let mut b = Box::pin(LooseFrame::new(2, "second"));
    a.as_mut().get_mut().link();
    b.as_mut().get_mut().link();
    mem::swap(a.as_mut().get_mut(), b.as_mut().get_mut());
    println!("\n5. lesson 2's frame, pinned the same way, then swapped");
    println!("  a points at itself: {}", a.points_at_itself());
    println!("  b points at itself: {}", b.points_at_itself());
    println!("  that swap COMPILED. Pin protected nothing here. Why?");
}

#[cfg(test)]
mod tests;
