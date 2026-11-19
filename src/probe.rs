//! A measuring instrument: is this type `Unpin`?
//!
//! `Unpin` is a fact the compiler knows and normally only reports as an
//! error. `is_unpin!(SomeType)` turns it into a `bool` we can print.
//!
//! How it works: `Probe<T>` gets an associated constant twice. Once
//! through a trait that every `Probe<T>` implements (`false`), and once
//! directly on `Probe<T>`, but only where `T: Unpin` (`true`). When both
//! exist, Rust picks the constant defined directly on the type.
//!
//! It needs a concrete type at the place where the macro is written. It
//! cannot answer for a generic `T` inside a generic function.
use std::marker::PhantomData;

pub struct Probe<T: ?Sized>(pub PhantomData<T>);

pub trait Fallback {
    const UNPIN: bool = false;
}

impl<T: ?Sized> Fallback for Probe<T> {}

impl<T: ?Sized + Unpin> Probe<T> {
    pub const UNPIN: bool = true;
}

/// `is_unpin!(String)` is `true`. `is_unpin!(PhantomPinned)` is `false`.
#[macro_export]
macro_rules! is_unpin {
    ($type:ty) => {{
        #[allow(unused_imports)]
        use $crate::probe::Fallback as _;
        <$crate::probe::Probe<$type>>::UNPIN
    }};
}

/// The same question for a VALUE whose type has no name we can write,
/// such as the future an `async fn` returns: `is_unpin_value!(future)`.
///
/// How it works: method lookup tries `Wrap<T>` before `&Wrap<T>`. The
/// first has an `answer` method only where `T: Unpin`.
pub struct Wrap<'a, T: ?Sized>(pub &'a T);

pub trait WhenUnpin {
    fn answer(&self) -> bool;
}

impl<T: ?Sized + Unpin> WhenUnpin for Wrap<'_, T> {
    fn answer(&self) -> bool {
        true
    }
}

pub trait Otherwise {
    fn answer(&self) -> bool;
}

impl<T: ?Sized> Otherwise for &Wrap<'_, T> {
    fn answer(&self) -> bool {
        false
    }
}

#[macro_export]
macro_rules! is_unpin_value {
    ($value:expr) => {{
        #[allow(unused_imports)]
        use $crate::probe::{Otherwise as _, WhenUnpin as _};
        (&$crate::probe::Wrap(&$value)).answer()
    }};
}

#[cfg(test)]
mod tests {
    use std::marker::PhantomPinned;

    #[test]
    fn the_probe_agrees_with_the_compiler() {
        fn compiles_only_for_unpin<T: Unpin>() {}
        compiles_only_for_unpin::<String>();
        assert!(is_unpin!(String));
        assert!(is_unpin!(u32));
        assert!(!is_unpin!(PhantomPinned));
        assert!(is_unpin!(str), "unsized types can be asked too");
    }

    #[test]
    fn values_can_be_asked_too() {
        let text = String::from("x");
        let marker = PhantomPinned;
        let future = async {};
        assert!(is_unpin_value!(text));
        assert!(!is_unpin_value!(marker));
        assert!(!is_unpin_value!(future), "an async block's future");
        assert!(is_unpin_value!(Box::pin(async {})), "boxed and pinned");
    }
}
