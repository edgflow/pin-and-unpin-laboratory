# Lesson 3 · `Pin<P>`: a promise about the thing a pointer points at

Part 2, Understand Pin. Code: `src/lesson03_pin.rs`, tests in `src/lesson03_pin/tests.rs`.

```console
$ cargo run -- 3
$ cargo test lesson03
$ cargo +nightly miri test lesson03      # this lesson reads through a raw pointer: Miri checks it
```

## A · The problem

Lesson 2 left us with a precise need. A `Box<Frame>` keeps the frame at one address, and it also hands out
`&mut Frame` to anyone who asks. Every way of moving a value out from behind a pointer goes through that
`&mut`: `mem::swap`, `mem::replace`, `mem::take`, plain assignment, moving out by dereference.

So we want a pointer that still lets us *use* the frame, and never gives out unrestricted `&mut Frame`.

## B · The working example first

```rust
let frame = Frame::pinned(7, "GET /health");
```

`Frame::pinned` returns a `Pin<Box<Frame>>`. One real run:

```text
size of Box<Frame>:      8
size of Pin<Box<Frame>>: 8

2. Frame::pinned
  points at itself: true
  read through the cursor: Some("GET /health")
  read a field directly: id 7

3. the Pin<Box<Frame>> moved into a Vec, the Vec grew
  the frame stayed: true
  points at itself: true
  read through the cursor: Some("GET /health")

4. pinned in a local with pin!
  before link: None
  after link:  Some("GET /items")
  points at itself: true

5. lesson 2's frame, pinned the same way, then swapped
  a points at itself: false
  b points at itself: false
  that swap COMPILED. Pin protected nothing here. Why?
```

Steps 2 to 4 are what we wanted. The frame points at itself, we **read the payload through the cursor** (the
read that lesson 2 could not risk), and it keeps working after the `Pin<Box<Frame>>` was pushed into a `Vec`
and the `Vec` reallocated.

Step 5 is there to keep us honest, and it is the hook for the next two lessons: lesson 2's frame, put into the
very same `Pin<Box<_>>`, was swapped, and it **compiled**. `Pin` alone protected nothing there.

## C · What happens in memory

Illustrative addresses.

`Pin<Box<Frame>>` is a `Box<Frame>` with a different type around it. Step 1 measured it: 8 bytes, the same as
the bare pointer. There is no flag in memory, no lock, nothing that happens at run time.

```text
 STACK                                        HEAP
 0x7020 ┌──────────────────────┐              0x6000 ┌────────────────────┐
        │ Pin<Box<Frame>>      │                 ┌─▶ │ payload.ptr ───────┼──▶ "GET /health"
        │   pointer = 0x6000 ──┼─────────────────┤   │ payload.cap  = 11  │
        └──────────────────────┘                 │   │ payload.len  = 11  │
          8 bytes. Moves freely.                 └───┼─ cursor = 0x6000   │
                                                     │ id           = 7   │
                                                     └────────────────────┘
                                                       the PINNED frame: stays at 0x6000
                                                       until it is dropped
```

Move the `Pin<Box<Frame>>` into a `Vec` (step 3):

```text
 HEAP (the Vec's buffer)                      HEAP
 0x9000 ┌──────────────────────┐              0x6000 ┌────────────────────┐
        │ pointer = 0x6000 ────┼────────────────────▶│ ... same frame ... │
        └──────────────────────┘                     │ cursor = 0x6000    │  still right
                                                     └────────────────────┘
```

The 8-byte pointer moved from `0x7020` to `0x9000`. The frame did not move, so the cursor is still correct.
This is the challenge of lesson 1 again. What is new is the type: with a plain `Box` we were *hoping* nobody
would reach inside and swap. With `Pin<Box<_>>` the compiler refuses.

The two pinning pointers of this lesson, side by side:

```text
 Pin<Box<Frame>>                              Pin<&mut Frame>   (from pin!)

 owner ──▶ HEAP  [ Frame ]                    STACK  [ Frame ]  ◀── Pin<&mut Frame>
                                                     a hidden local that nobody can name

 owns the frame                               borrows the frame
 the frame lives on the heap                  the frame lives in the enclosing function's stack frame
 can be returned, stored, sent                cannot outlive that function
 pinned for as long as the Box lives          pinned for as long as the hidden local lives
```

## D · The compiler's perspective

**`Pin<Ptr>` wraps a pointer and removes methods.** That is all it is. The pinned thing is the *pointee*. The
`Pin` itself is an ordinary value.

What a `Pin<Ptr>` to a type that is **not `Unpin`** lets you do, and what it does not:

| You want | Available? | How |
|---|---|---|
| Read the pointee | yes | `Deref`: `frame.id`, `&*frame` |
| A pinned shared reference | yes | `frame.as_ref()` → `Pin<&Frame>` |
| A pinned mutable reference | yes | `frame.as_mut()` → `Pin<&mut Frame>` |
| Call a method that takes `self: Pin<&mut Self>` | yes | `frame.as_mut().link()` |
| Replace the value in place | yes | `frame.set(new_frame)` |
| `&mut Frame` | **no** | `get_mut` requires `Unpin`; there is no `DerefMut` |
| The frame, by value | **no** | `Pin::into_inner` requires `Unpin` |
| `&mut Frame` anyway | only in `unsafe` | `get_unchecked_mut`, and you take over the promise |

`invalid/lesson03_moving_out_of_pin.rs` and `invalid/lesson03_writing_through_pin.rs` try the forbidden rows.
They are not part of the build. Their real errors, shortened to the telling lines:

```text
error[E0277]: `PhantomPinned` cannot be unpinned
26 |     mem::swap(a.as_mut().get_mut(), b.as_mut().get_mut());
   |                          ^^^^^^^ within `Frame`, the trait `Unpin` is not implemented for `PhantomPinned`
note: required by a bound in `Pin::<&'a mut T>::get_mut`

error[E0277]: `PhantomPinned` cannot be unpinned
   |     let loose: Box<Frame> = Pin::into_inner(a);
note: required by a bound in `Pin::<Ptr>::into_inner`

error[E0594]: cannot assign to data in dereference of `Pin<Box<Frame>>`
   |     a.id = 9;
   = help: trait `DerefMut` is required to modify through a dereference, but it is not implemented for `Pin<Box<Frame>>`

error[E0507]: cannot move out of dereference of `Pin<Box<Frame>>`
   |     let moved: Frame = *b;
```

**The pinning contract, stated carefully.** For a value whose type is not `Unpin`: from the moment it is pinned
until it is dropped,

1. it stays at the same address, and
2. its memory is not freed, overwritten or reused for something else unless its destructor has run first.

Safe code cannot break either point, because safe code cannot get `&mut` to the pointee or get it back by
value. Code that uses `unsafe` (`get_unchecked_mut`, `Pin::new_unchecked`) takes on the duty of keeping both.

**What `Pin` does not promise** (each of these is a common belief):

- *"Nothing can move."* The `Pin<Box<Frame>>` itself moves like any pointer (step 3).
- *"The value is immutable."* It can be read, mutated through pinned methods, and replaced with `set`.
- *"Pinning does something at run time."* It is the same 8 bytes.
- *"Every type is protected once it is in a `Pin`."* Step 5. The restrictions apply only to types that are not
  `Unpin`. That is lessons 4 and 5.

## E · The solution

```rust
pub struct Frame {
    pub id: u32,
    payload: String,
    cursor: *const String,
    _address_sensitive: PhantomPinned,
}

pub fn unlinked(id: u32, payload: &str) -> Frame { /* cursor: null */ }

pub fn link(self: Pin<&mut Self>) {
    // SAFETY: we write one field in place and move nothing out.
    let this = unsafe { self.get_unchecked_mut() };
    this.cursor = ptr::from_ref(&this.payload);
}

pub fn pinned(id: u32, payload: &str) -> Pin<Box<Frame>> {
    let mut frame = Box::pin(Frame::unlinked(id, payload));
    frame.as_mut().link();
    frame
}

pub fn payload(self: Pin<&Self>) -> Option<&str> {
    if self.cursor.is_null() {
        return None;
    }
    // SAFETY: a non-null cursor was set by `link` on this frame while it
    // was pinned, so it still points at this frame's own, live payload.
    let payload: &String = unsafe { &*self.cursor };
    Some(payload.as_str())
}
```

The whole design is one invariant, written above the struct: **the cursor is null, or it points at the payload
of this very frame, and this frame is pinned.** Three facts hold it up:

1. The only code that sets the cursor is `link`.
2. `link` takes `Pin<&mut Self>`, so it can only be called on a frame that is already pinned.
3. A pinned `Frame` cannot be moved by safe code, because of the one marker field. (Lesson 5 is about that field.)

A caller writes no `unsafe` at all. There are exactly two `unsafe` blocks in the module, each with its reason
written next to it.

## F · The important lines

```rust
pub fn link(self: Pin<&mut Self>)
```
A method can name its receiver's type. `Pin<&mut Self>` means: "you may call this only if you hold a pinned
mutable reference to me." It turns "please don't move this afterwards" from a comment into a type.

```rust
let this = unsafe { self.get_unchecked_mut() };
```
The one escape hatch. It returns `&mut Frame`, which is exactly the thing `Pin` exists to withhold, so it is
`unsafe`: we now hold the power to move the frame and we promise not to use it. We use it for one field write.

```rust
this.cursor = ptr::from_ref(&this.payload);
```
The same line as lesson 2. The difference is not in the line. It is in what we know about where `this` is.

```rust
let mut frame = Box::pin(Frame::unlinked(id, payload));
frame.as_mut().link();
```
The two-step shape lesson 2 predicted: reach the final place first, link second. `Box::pin` moves the unlinked
frame onto the heap (harmless: its cursor is null) and returns `Pin<Box<Frame>>`. `as_mut()` reborrows it as
`Pin<&mut Frame>` without giving up ownership.

```rust
pub fn payload(self: Pin<&Self>) -> Option<&str>
```
The read through the raw pointer takes a *pinned* receiver too, because pinning is what makes it sound. And it
returns `Option`, because an unlinked frame honestly has nothing to read.

```rust
let mut local = pin!(Frame::unlinked(8, "GET /items"));
```
`pin!` moves the value into a hidden local variable and gives back `Pin<&mut Frame>` to it. Nobody can name
that local, so nobody can move it. No heap allocation.

## G · Experiment

1. **Take the marker out.** Delete the `_address_sensitive` field (and its initializer).
   *Prediction:* everything still compiles, and the type is no longer protected: `get_mut` becomes available.
   *Evidence:* that is exactly step 5, where lesson 2's frame (no marker) is swapped inside `Pin<Box<_>>`.
   Put the field back.
2. **`Pin::new`.** Write `let mut f = Frame::unlinked(1, "x"); let p = Pin::new(&mut f);`.
   *Prediction:* refused. *Measured:* `error[E0277]: PhantomPinned cannot be unpinned`. `Pin::new` is the safe
   constructor and only accepts `Unpin` targets. For anything else you pin with `Box::pin` or `pin!`, which
   make sure nobody else can still reach the value.
3. **Both of Miri's aliasing models.** `cargo +nightly miri test lesson03`, then again with
   `MIRIFLAGS=-Zmiri-tree-borrows`. *Measured:* 9 tests pass, no error, under both.

## H · Common mistakes

- **"`Pin` pins the pointer."** It pins the pointee. `Pin<Box<T>>` moves; the `T` does not.
- **"I'll just call `get_unchecked_mut` whenever I need `&mut`."** Each call is a promise not to move the value
  and not to invalidate it. `mem::swap` on the result compiles, and breaks the contract for everyone who relied
  on it.
- **"Linking in `new` is fine if I return `Pin<Box<Self>>`."** Only if you link *after* `Box::pin`. Link first
  and the cursor points at the stack slot the frame was built in.
- **"The cursor-reading method can take `&self`."** Then it could be called on a frame that was never pinned.
  Here that would still be safe only because an unpinned frame always has a null cursor. Making the receiver
  `Pin<&Self>` states the requirement instead of relying on that argument.
- **"`pin!` gives me something I can return."** It borrows a local of the current function. Use `Box::pin`
  when the pinned value has to outlive the function.
- **"A pinned value cannot be dropped early."** It can be dropped at any time. What cannot happen is its memory
  being reused *without* the drop.

## I · Challenge

`Pin::set` is in the "available" half of the table:

```rust
let mut frame = Frame::pinned(1, "old");
frame.set(Frame::unlinked(9, "new"));
```

It compiles, in safe code, for our address-sensitive type. Before you run it: does this break the pinning
contract? What does `frame.as_ref().payload()` return afterwards, and at which address is the frame?

**Expected result:** the contract holds. `set` runs the old frame's destructor where it stands, then writes
the new frame into the same place. Nothing was moved *out*, and the memory was only reused *after* the drop:
both points of the contract are respected. The address is unchanged, `id` is 9, and `payload()` is `None`
because the new frame was never linked. One `frame.as_mut().link()` later it returns `Some("new")`. The test
`set_replaces_the_value_without_moving_anything_out` checks every step.

The question for lesson 4: *step 5 compiled. Which exact property of lesson 2's `Frame` made `get_mut`
available, and who decided that it has that property?*

## Documentation

- `std::pin`, the module: https://doc.rust-lang.org/std/pin/
- "What is pinning?" and the drop guarantee:
  https://doc.rust-lang.org/std/pin/#what-is-pinning and
  https://doc.rust-lang.org/std/pin/#drop-guarantee
- `Pin`: https://doc.rust-lang.org/std/pin/struct.Pin.html
  (`as_mut`, `as_ref`, `set`, `get_mut`, `get_unchecked_mut`, `into_inner`, `new`, `new_unchecked`)
- `Box::pin`: https://doc.rust-lang.org/std/boxed/struct.Box.html#method.pin
- `std::pin::pin!`: https://doc.rust-lang.org/std/pin/macro.pin.html
- `PhantomPinned`: https://doc.rust-lang.org/std/marker/struct.PhantomPinned.html
- Miri: https://github.com/rust-lang/miri
