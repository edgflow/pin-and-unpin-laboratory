//! Lesson 5 · `PhantomPinned`.
//!
//! A struct with no fields and no size that changes which methods exist
//! on `Pin`. This lesson measures what it costs (nothing), what it does
//! (one thing), and what it does not do (everything else people expect).
//!
//! Run it:  cargo run -- 5
use crate::is_unpin;
use std::cell::{Cell, RefCell};
use std::marker::{PhantomData, PhantomPinned};
use std::pin::{Pin, pin};
use std::rc::Rc;
use std::{mem, ptr};

/// The frame of lesson 2: three ordinary fields.
pub struct Bare {
    pub id: u32,
    pub payload: String,
    pub cursor: *const String,
}

/// The same three fields, plus the marker.
pub struct Marked {
    pub id: u32,
    pub payload: String,
    pub cursor: *const String,
    pub _address_sensitive: PhantomPinned,
}

/// The marker hidden one level down still counts.
pub struct ViaPhantomData {
    pub id: u32,
    pub _address_sensitive: PhantomData<PhantomPinned>,
}

/// What happened to one `Session`, written down when it is dropped.
#[derive(Debug, Clone, PartialEq)]
pub struct Farewell {
    pub name: String,
    pub pinned_at: usize,
    pub dropped_at: usize,
}

pub type Log = Rc<RefCell<Vec<Farewell>>>;

/// A value that remembers where it was pinned, and reports where it was
/// when it died. It has no pointer into itself. The marker alone makes
/// it address-sensitive in the eyes of the compiler.
pub struct Session {
    name: String,
    pinned_at: Cell<usize>,
    log: Log,
    _address_sensitive: PhantomPinned,
}

impl Session {
    pub fn new(name: &str, log: &Log) -> Session {
        Session {
            name: name.to_string(),
            pinned_at: Cell::new(0),
            log: Rc::clone(log),
            _address_sensitive: PhantomPinned,
        }
    }

    pub fn address(&self) -> usize {
        ptr::from_ref(self).addr()
    }

    /// Records the address. Only callable once the session is pinned.
    pub fn remember_where_i_am(self: Pin<&Self>) {
        self.pinned_at.set(self.address());
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // `drop` always receives `&mut self`, even for a value that was
        // pinned. The standard library's advice: treat it as pinned
        // anyway, by turning it back into a `Pin` straight away.
        //
        // SAFETY: the value is being dropped and is never used again, so
        // nothing can move it after this point. And `finish` only reads.
        let this = unsafe { Pin::new_unchecked(self) };
        Session::finish(this);
    }
}

impl Session {
    fn finish(self: Pin<&mut Self>) {
        let farewell = Farewell {
            name: self.name.clone(),
            pinned_at: self.pinned_at.get(),
            dropped_at: self.address(),
        };
        self.log.borrow_mut().push(farewell);
    }
}

fn line(name: &str, unpin: bool) {
    let verdict = if unpin { "Unpin" } else { "!Unpin" };
    println!("  {name:<30} {verdict}");
}

pub fn run() {
    // 1. What the marker costs.
    println!("1. what it costs");
    println!(
        "  size of PhantomPinned:  {}",
        mem::size_of::<PhantomPinned>()
    );
    println!("  size of Bare:           {}", mem::size_of::<Bare>());
    println!("  size of Marked:         {}", mem::size_of::<Marked>());

    // 2. What it does.
    println!("\n2. what it does");
    line("Bare", is_unpin!(Bare));
    line("Marked", is_unpin!(Marked));
    line("PhantomData<String>", is_unpin!(PhantomData<String>));
    line(
        "PhantomData<PhantomPinned>",
        is_unpin!(PhantomData<PhantomPinned>),
    );
    line("ViaPhantomData", is_unpin!(ViaPhantomData));

    // 3. What it does NOT do: an unpinned Session moves like anything.
    let log: Log = Rc::default();
    let session = Session::new("before", &log);
    let first = session.address();
    let boxed = Box::new(session);
    let second = boxed.address();
    println!("\n3. a Session that is not pinned yet");
    println!("  moved into a Box: {}", first != second);
    drop(boxed);

    // 4. Pinned on the heap: one address, from the pin to the drop.
    let session = Box::pin(Session::new("on the heap", &log));
    session.as_ref().remember_where_i_am();
    let mut shelf = Vec::with_capacity(1);
    shelf.push(session);
    shelf.reserve(4096);
    drop(shelf);

    // 5. Pinned on the stack: the same promise, for one scope.
    {
        let session = pin!(Session::new("on the stack", &log));
        session.as_ref().remember_where_i_am();
    }

    println!("\n4 and 5. where each Session was pinned, and where it died");
    for farewell in log.borrow().iter() {
        let stayed = farewell.pinned_at == farewell.dropped_at;
        let verdict = match (farewell.pinned_at, stayed) {
            (0, _) => "never pinned",
            (_, true) => "same address",
            (_, false) => "MOVED",
        };
        println!("  {:<14} {verdict}", farewell.name);
    }
}

#[cfg(test)]
mod tests;
