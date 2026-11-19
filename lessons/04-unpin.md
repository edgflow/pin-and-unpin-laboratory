# Lesson 4 · `Unpin` and `!Unpin`

Part 2, Understand Pin. Code: `src/lesson04_unpin.rs`, `src/probe.rs`, tests in `src/lesson04_unpin/tests.rs`.

```console
$ cargo run -- 4
$ cargo test lesson04
$ cargo test probe
```

## A · The problem

Lesson 3 ended on a result that should bother you. Two frames, both inside `Pin<Box<_>>`:

- lesson 3's `Frame`: `mem::swap` through `get_mut` is **refused** (E0277).
- lesson 2's `Frame`: the very same line **compiles**, and the cursors end up crossed.

Same `Pin`, same `Box`, same method. So the protection is not in `Pin`. It is switched on and off by something
about the *type inside*. That something is one trait: `Unpin`.

## B · The working example first

The compiler knows, for every type, whether it is `Unpin`. Normally it only tells us through an error. The
small macro in `src/probe.rs` turns that knowledge into a `bool`, so we can simply ask. One real run:

```text
1. which types are Unpin?
  u32                                Unpin
  String                             Unpin
  Vec<u8>                            Unpin
  *const String                      Unpin
  PhantomPinned                      !Unpin
  lesson 2's Frame                   Unpin
  lesson 3's Frame                   !Unpin

2. a wrapper follows its fields
  Envelope<String>                   Unpin
  Envelope<lesson 3's Frame>         !Unpin
  (u32, lesson 3's Frame)            !Unpin
  Option<lesson 3's Frame>           !Unpin
  [lesson 3's Frame; 2]              !Unpin

3. pointers do not follow their target
  Box<lesson 3's Frame>              Unpin
  &mut lesson 3's Frame              Unpin
  Pin<Box<lesson 3's Frame>>         Unpin
  Envelope<Box<lesson 3's Frame>>    Unpin

4. two pinned Strings, swapped, then written through
  first:  "second, and written through the pin"
  second: "first"

5. a type that claims Unpin by hand
  Reckless (holds PhantomPinned)     Unpin
  swapped inside Pin<Box<_>>: ids are now 2 1
```

Read the table before the explanation. Four facts are already visible:

1. **Almost everything is `Unpin`.** Numbers, `String`, `Vec`, even a raw pointer.
2. **Lesson 2's frame is `Unpin`; lesson 3's is not.** That is the difference we were looking for.
3. **A struct, tuple, `Option` or array follows its fields** (block 2).
4. **A pointer does not follow what it points at** (block 3). `Box<Frame>` is `Unpin` although `Frame` is not.

Blocks 4 and 5 show what `Unpin` switches on: two pinned `String`s are swapped and written through, and a type
that *claims* `Unpin` by hand is swapped inside `Pin<Box<_>>` exactly as if `Pin` were not there.

## C · What happens in memory

Nothing. That is the point of this lesson, and it is worth a diagram to make it stick. Illustrative addresses.

```text
 Pin<Box<String>>                        Pin<Box<Frame>>   (lesson 3's Frame)

 0x7000 ┌──────────────┐                 0x7010 ┌──────────────┐
        │ ptr = 0x6000 │                        │ ptr = 0x6100 │
        └──────┬───────┘                        └──────┬───────┘
               ▼                                       ▼
 0x6000 ┌──────────────┐                 0x6100 ┌──────────────┐
        │   String     │                        │    Frame     │
        └──────────────┘                        └──────────────┘

 The two pictures are the same: one pointer, one value behind it.
 No bit anywhere says "pinned", and no bit says "Unpin".

 The whole difference lives in the type checker:

   String: Unpin          →  get_mut, DerefMut, into_inner, Pin::new   all available
   Frame: !Unpin          →  none of them
```

`Unpin` has no methods, no data and no run-time presence. It is a **marker trait**: a yes-or-no fact about a
type, used only to decide which methods of `Pin` exist.

And why a pointer is `Unpin` whatever it points at:

```text
 0x7010 ┌──────────────┐  moving THIS (8 bytes) ...
        │ ptr = 0x6100 │
        └──────┬───────┘
               ▼
 0x6100 ┌──────────────┐  ... does not move THIS.
        │    Frame     │
        └──────────────┘
```

A `Box<Frame>` has no opinion about its own address. It is 8 bytes holding a number, like the `payload.ptr` of
lesson 1. Moving the box anywhere leaves the frame where it is. So `Box<Frame>: Unpin`, even though
`Frame: !Unpin`. The same holds for `&T`, `&mut T` and `Pin<Box<T>>` itself.

## D · The compiler's perspective

**What `Unpin` means.** `T: Unpin` says: *a `T` does not care about being moved, even after it has been
pinned.* The pinning contract of lesson 3 began with "for a value whose type is not `Unpin`". For an `Unpin`
type there is nothing to protect, so `Pin<Ptr>` gives everything back:

| Method | Available when |
|---|---|
| `Pin::new(ptr)` | the target is `Unpin` |
| `Pin::get_mut` | the target is `Unpin` |
| `DerefMut` for `Pin<Ptr>` | the target is `Unpin` |
| `Pin::into_inner` | the target is `Unpin` |

That answers one of the course's hard questions. *Why can `Unpin` types move even when pinned?* Because for
them `Pin` hands out `&mut T` in safe code, and `&mut T` is all a move needs.

**Who decides.** `Unpin` is an **auto trait**. Nobody writes `impl Unpin for String`. The compiler implements
it automatically for every type whose fields are all `Unpin`, the same way it does for `Send` and `Sync`.

That explains both frames:

- Lesson 2's `Frame` has a `u32`, a `String` and a `*const String`. All three are `Unpin`, so the frame is
  `Unpin`. The compiler has no idea that the raw pointer happens to point back into the struct. **A raw pointer
  is just a number to it.** Address-sensitivity is a fact about *our intent*, and the compiler cannot see it.
- Lesson 3's `Frame` has one extra field of type `PhantomPinned`, and `PhantomPinned` is not `Unpin`. One
  non-`Unpin` field is enough, so the whole struct is `!Unpin`.

**`!Unpin` is not a trait you can write.** It only means "does not implement `Unpin`". The direct way to say
it would be a negative implementation, and stable Rust refuses that
(`invalid/lesson04_negative_impl.rs`, not part of the build):

```text
error[E0658]: negative impls are experimental
11 | impl !Unpin for Frame {}
   |      ^^^^^^
   = help: use marker types for now
```

"Use marker types" is exactly what `PhantomPinned` is. Lesson 5.

**Where you actually meet `Unpin`: in somebody else's bounds.** Any function that wants to move a value out
of a pin, or to call `Pin::new`, has to ask for it (`invalid/lesson04_unpin_required.rs`):

```rust
fn take_back<T: Unpin>(pinned: Pin<Box<T>>) -> T {
    *Pin::into_inner(pinned)
}
```

```text
error[E0277]: `PhantomPinned` cannot be unpinned
22 |       let frame = take_back(Box::pin(Frame {
   |                   --------- required by a bound introduced by this call
   |  within `Frame`, the trait `Unpin` is not implemented for `PhantomPinned`
note: required because it appears within the type `Frame`
12 |     _address_sensitive: PhantomPinned,
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ required by this field
note: required by a bound in `take_back`
16 | fn take_back<T: Unpin>(pinned: Pin<Box<T>>) -> T {
   |                 ^^^^^ required by this bound in `take_back`
```

Learn to read this message from the bottom up. Who asked (`take_back`, the bound `T: Unpin`). Which type
failed (`Frame`). Which field is the reason (`_address_sensitive: PhantomPinned`). The headline "cannot be
unpinned" is the same error you will get from async code, where the non-`Unpin` thing is a future.

## E · The "solution": know which side a type is on

There is nothing to fix here. The lesson is a rule you can now apply to any type:

1. Look at the fields. All `Unpin` → the type is `Unpin`, and `Pin` around it changes nothing.
2. One field that is not `Unpin` → the type is `!Unpin`, and `Pin` around it means what lesson 3 said.
3. A pointer, reference or `Box` to anything → `Unpin`.

And one warning, block 5:

```rust
pub struct Reckless {
    pub id: u32,
    _address_sensitive: PhantomPinned,
}

impl Unpin for Reckless {}
```

Implementing `Unpin` by hand is **safe** code. No `unsafe` keyword is needed, and it overrides the automatic
answer. For `Reckless` that is harmless: it holds no pointer into itself. For lesson 3's `Frame` the same
one-line `impl` would hand `get_mut` back to everyone and make the `unsafe` read in `payload` unsound.

Why is that allowed to be safe? Because a type can only *depend* on being pinned by using `unsafe`
somewhere (ours does, twice). Whoever writes that `unsafe` code takes on the duty of keeping the type
`!Unpin`. The rule is not "you cannot write `impl Unpin`". The rule is: **if your type's `unsafe` code relies
on pinning, you must not.**

## F · The important lines

```rust
impl<T: ?Sized + Unpin> Probe<T> {
    pub const UNPIN: bool = true;
}
```
The probe. `Probe<T>` gets a constant named `UNPIN` from a trait for every `T` (`false`), and a second one
directly on the type, but only where `T: Unpin` (`true`). When both exist, the one defined directly on the type
wins. It is a measuring trick, not something to put in production code, and it only works for a concrete type
written at the call site.

```rust
let mut a = Pin::new(&mut first);
```
`Pin::new` is the safe constructor. It compiles here because `String: Unpin`. For lesson 3's frame it does
not (experiment 2 of lesson 3).

```rust
mem::swap(a.as_mut().get_mut(), b.as_mut().get_mut());
a.push_str(", and written through the pin");
```
`get_mut` exists, and so does `DerefMut` (that is what lets `push_str` be called on a `Pin<&mut String>`).
The variable `first` now holds what used to be in `second`: a pinned value was moved, legally.

```rust
pub struct Envelope<T> {
    pub id: u32,
    pub inner: T,
}
```
No `impl` anywhere, and `Envelope<String>` is `Unpin` while `Envelope<Frame>` is not. Auto traits are decided
per concrete type.

```rust
fn must_be_unpin<T: ?Sized + Unpin>() {}
```
In the tests. A function that only compiles for `Unpin` types: the compiler's own answer, next to the probe's.

## G · Experiment

1. **Give `Envelope` the marker.** Add a field `_pin: PhantomPinned` to `Envelope<T>`.
   *Prediction:* every `Envelope<_>` line flips to `!Unpin`, including `Envelope<String>` and
   `Envelope<Box<Frame>>`. One non-`Unpin` field is enough, whatever `T` is.
2. **Remove `impl Unpin for Reckless {}`.**
   *Prediction:* block 5 no longer compiles: `E0277`, "`PhantomPinned` cannot be unpinned", at `get_mut`.
3. **Try to say it directly.** `rustc --edition 2024 invalid/lesson04_negative_impl.rs`.
   *Measured:* `error[E0658]: negative impls are experimental`, with the hint "use marker types for now".

## H · Common mistakes

- **"`Unpin` means the value is not pinned."** It means the *type* does not care about pinning. A `String`
  inside a `Pin` is pinned and `Unpin` at the same time.
- **"`!Unpin` types cannot be moved."** They move freely until they are pinned. Lesson 3 moved an unlinked
  `Frame` into a `Box` before pinning it.
- **"Most types are `!Unpin`, so I need `Pin` a lot."** The reverse. Nearly every type you write is `Unpin`.
  The common `!Unpin` types are the futures that `async` generates, and types that contain one.
- **"`Box<T>` is `Unpin` only if `T` is."** `Box<T>` is always `Unpin`. This is why `Box::pin(x)` makes any
  future usable where an `Unpin` future is required: the *box* can be moved, and the future inside stays put.
- **"A raw pointer field makes a type `!Unpin`."** Raw pointers are `Unpin`. If your type is address-sensitive,
  you have to say so yourself.
- **"`impl Unpin` needs `unsafe`."** It does not. Which is why it deserves a second look in code review.

## I · Challenge

`Pin<Box<Frame>>` is `Unpin` (block 3). So this is all safe code:

```rust
let mut frame: Pin<Box<Frame>> = Frame::pinned(1, "hello");
let outer: Pin<&mut Pin<Box<Frame>>> = Pin::new(&mut frame);
let pointer: &mut Pin<Box<Frame>> = outer.get_mut();
let mut other = Frame::pinned(2, "world");
mem::swap(pointer, &mut other);
```

We called `get_mut` and `mem::swap` on something pinned, with an address-sensitive `Frame` involved. Before
you run it: is the pinning contract broken? Where is frame 1 afterwards, and does its cursor still work?

**Expected result:** the contract is intact. We pinned the *pointer*, and the pointer is `Unpin`, so we got
`&mut` to the pointer and swapped two pointers. Neither `Frame` moved. Frame 1 is now owned by `other`, at the
same address as before, and `other.as_ref().payload()` is `Some("hello")`. The test
`the_outer_pointer_can_move_and_the_frame_still_cannot` checks it.

That is the cleanest statement of what `Pin<Ptr>` pins: **the value `Ptr` points at. Never `Ptr` itself.**

The question for lesson 5: *`PhantomPinned` is a struct with no fields and no size. How can putting it in our
struct change which methods exist on `Pin`?*

## Documentation

- `Unpin`: https://doc.rust-lang.org/std/marker/trait.Unpin.html
- `std::pin`, "`Unpin`": https://doc.rust-lang.org/std/pin/#unpin
- Auto traits, the Rust Reference: https://doc.rust-lang.org/reference/special-types-and-traits.html#auto-traits
- `Pin::new`, `Pin::get_mut`, `Pin::into_inner`: https://doc.rust-lang.org/std/pin/struct.Pin.html
- `PhantomPinned`: https://doc.rust-lang.org/std/marker/struct.PhantomPinned.html
- Negative implementations (unstable): https://github.com/rust-lang/rust/issues/68318
