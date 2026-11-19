// INTENTIONALLY INVALID. This file is not part of the build.
// Check it:  rustc --edition 2024 invalid/lesson04_unpin_required.rs
//
// How `Unpin` usually shows up in real code: as a bound on somebody
// else's function. `take_back` wants to move the value out of its pin,
// so it has to ask for `T: Unpin`. An address-sensitive type is refused.
use std::marker::PhantomPinned;
use std::pin::Pin;

struct Frame {
    id: u32,
    _address_sensitive: PhantomPinned,
}

/// Moves the value out of the box. Only sound when `T: Unpin`.
fn take_back<T: Unpin>(pinned: Pin<Box<T>>) -> T {
    *Pin::into_inner(pinned)
}

fn main() {
    let text = take_back(Box::pin(String::from("fine")));
    let frame = take_back(Box::pin(Frame {
        id: 1,
        _address_sensitive: PhantomPinned,
    }));
    println!("{text} {}", frame.id);
}
