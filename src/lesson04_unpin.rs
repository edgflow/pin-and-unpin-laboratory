//! Lesson 4 · `Unpin` and `!Unpin`.
//!
//! Lesson 3 ended on a swap that compiled inside a `Pin`. The reason is
//! one trait. This lesson measures which types have it, shows what it
//! switches on, and shows who decides.
//!
//! Run it:  cargo run -- 4
use crate::is_unpin;
use crate::lesson02_self_referential::Frame as LooseFrame;
use crate::lesson03_pin::Frame as PinnedFrame;
use std::marker::PhantomPinned;
use std::mem;
use std::pin::Pin;

/// A wrapper with one field of any type. It is `Unpin` exactly when
/// `T` is: the compiler works it out from the fields.
pub struct Envelope<T> {
    pub id: u32,
    pub inner: T,
}

/// A type that holds the address-sensitivity marker AND declares itself
/// `Unpin` by hand. Writing that `impl` is safe. Lesson 5 comes back to
/// what it costs.
pub struct Reckless {
    pub id: u32,
    _address_sensitive: PhantomPinned,
}

impl Reckless {
    pub fn new(id: u32) -> Reckless {
        Reckless {
            id,
            _address_sensitive: PhantomPinned,
        }
    }
}

impl Unpin for Reckless {}

fn line(name: &str, unpin: bool) {
    let verdict = if unpin { "Unpin" } else { "!Unpin" };
    println!("  {name:<34} {verdict}");
}

pub fn run() {
    // 1. Ask the compiler, type by type.
    println!("1. which types are Unpin?");
    line("u32", is_unpin!(u32));
    line("String", is_unpin!(String));
    line("Vec<u8>", is_unpin!(Vec<u8>));
    line("*const String", is_unpin!(*const String));
    line("PhantomPinned", is_unpin!(PhantomPinned));
    line("lesson 2's Frame", is_unpin!(LooseFrame));
    line("lesson 3's Frame", is_unpin!(PinnedFrame));

    // 2. A struct is Unpin when all of its fields are.
    println!("\n2. a wrapper follows its fields");
    line("Envelope<String>", is_unpin!(Envelope<String>));
    line(
        "Envelope<lesson 3's Frame>",
        is_unpin!(Envelope<PinnedFrame>),
    );
    line("(u32, lesson 3's Frame)", is_unpin!((u32, PinnedFrame)));
    line("Option<lesson 3's Frame>", is_unpin!(Option<PinnedFrame>));
    line("[lesson 3's Frame; 2]", is_unpin!([PinnedFrame; 2]));

    // 3. A pointer is Unpin whatever it points at.
    println!("\n3. pointers do not follow their target");
    line("Box<lesson 3's Frame>", is_unpin!(Box<PinnedFrame>));
    line("&mut lesson 3's Frame", is_unpin!(&mut PinnedFrame));
    line(
        "Pin<Box<lesson 3's Frame>>",
        is_unpin!(Pin<Box<PinnedFrame>>),
    );
    line(
        "Envelope<Box<lesson 3's Frame>>",
        is_unpin!(Envelope<Box<PinnedFrame>>),
    );

    // 4. For an Unpin type, Pin is an ordinary pointer again.
    let mut first = String::from("first");
    let mut second = String::from("second");
    let mut a = Pin::new(&mut first);
    let mut b = Pin::new(&mut second);
    mem::swap(a.as_mut().get_mut(), b.as_mut().get_mut());
    a.push_str(", and written through the pin");
    println!("\n4. two pinned Strings, swapped, then written through");
    println!("  first:  {first:?}");
    println!("  second: {second:?}");

    // 5. Unpin can be claimed by hand, in safe code.
    println!("\n5. a type that claims Unpin by hand");
    line("Reckless (holds PhantomPinned)", is_unpin!(Reckless));
    let mut one = Box::pin(Reckless::new(1));
    let mut two = Box::pin(Reckless::new(2));
    mem::swap(one.as_mut().get_mut(), two.as_mut().get_mut());
    println!(
        "  swapped inside Pin<Box<_>>: ids are now {} {}",
        one.id, two.id
    );
}

#[cfg(test)]
mod tests;
