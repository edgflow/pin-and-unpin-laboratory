# Lesson 5 · `PhantomPinned`

Part 2, Understand Pin. Code: `src/lesson05_phantom_pinned.rs`, tests in `src/lesson05_phantom_pinned/tests.rs`.

```console
$ cargo run -- 5
$ cargo test lesson05
$ cargo +nightly miri test lesson05      # one unsafe line in a Drop impl: Miri checks it
```

## A · The problem

Lesson 4 gave us the rule: a type is `Unpin` unless one of its fields is not. And it showed that we cannot
write `impl !Unpin for Frame {}` on stable Rust. So how does an ordinary library author say "my type's address
matters"? In lesson 3 we did it with one field and promised to explain it:

```rust
_address_sensitive: PhantomPinned,
```

Three questions. What does that field cost? What exactly does it do? And what does it *not* do, that people
assume it does?

## B · The working example first

One real run:

```text
1. what it costs
  size of PhantomPinned:  0
  size of Bare:           40
  size of Marked:         40

2. what it does
  Bare                           Unpin
  Marked                         !Unpin
  PhantomData<String>            Unpin
  PhantomData<PhantomPinned>     !Unpin
  ViaPhantomData                 !Unpin

3. a Session that is not pinned yet
  moved into a Box: true

4 and 5. where each Session was pinned, and where it died
  before         never pinned
  on the heap    same address
  on the stack   same address
```

Block 1: the marker has size 0, and the struct is 40 bytes with it and 40 bytes without it.
Block 2: with it, the struct is `!Unpin`. That is the whole effect.
Block 3: a `Session` that carries the marker and is **not pinned yet** moves into a `Box` like anything else.
Blocks 4 and 5: once pinned, each `Session` was dropped at the same address it was pinned at, even though the
`Pin<Box<_>>` holding one of them travelled into a `Vec` that reallocated.

## C · What happens in memory

Illustrative addresses. The two structs of block 1, side by side:

```text
 Bare  (40 bytes)                          Marked  (40 bytes)

 0x6000 ┌───────────────────────┐          0x6100 ┌───────────────────────┐
        │ payload.ptr           │                 │ payload.ptr           │
        │ payload.cap           │                 │ payload.cap           │
        │ payload.len           │                 │ payload.len           │
        │ cursor                │                 │ cursor                │
        │ id                    │                 │ id                    │
        └───────────────────────┘                 │ _address_sensitive    │  ← 0 bytes
                                                  └───────────────────────┘
```

The marker occupies nothing. There is no byte in a `Marked` that a `Bare` does not have. Whatever
`PhantomPinned` does, it does not do it at run time.

"Phantom" is the standard library's word for this: a field that exists only for the type checker.
`PhantomData<T>` is the general tool ("pretend I hold a `T`"). `PhantomPinned` is the special case for one
question: *am I `Unpin`?*

And the timeline that blocks 3 to 5 measured, for one `Session`:

```text
   created ──────────── moved freely ───────────┐
   (anywhere)           (stack → Box, fine)     │
                                                ▼
                                         pinned at 0x6200
                                                │   from here on: the same address,
                                                │   and the memory is not reused ...
                                                ▼
                                         dropped at 0x6200
                                                    ... until its destructor has run.
```

The marker does not *start* that promise. `Box::pin` or `pin!` does. The marker is what makes the promise
*binding* once it has started.

## D · The compiler's perspective

**What `PhantomPinned` is.** In the standard library's source it is two lines (read from the library source
shipped with the toolchain):

```rust
pub struct PhantomPinned;

impl !Unpin for PhantomPinned {}
```

A unit struct, and a negative implementation of `Unpin`. The standard library is allowed to use that unstable
feature; we are not (lesson 4, E0658). So `PhantomPinned` is the one stable door to the same result.

**Why putting it in a struct works.** No new rule. It is the auto-trait rule of lesson 4:

> a type is `Unpin` when all of its fields are `Unpin`.

`PhantomPinned` is not, so no struct that contains one is, automatically. That is the complete answer to
*"why does `PhantomPinned` prevent automatic `Unpin`?"* Block 2 also shows it passing through a
`PhantomData<PhantomPinned>`: `PhantomData<T>` is `Unpin` exactly when `T` is.

**What it does not do.** Each line is a test in `src/lesson05_phantom_pinned/tests.rs`:

| Belief | Reality |
|---|---|
| It makes the value immovable | A marked value moves freely until it is pinned (block 3) |
| It pins the value | Only `Box::pin`, `pin!` or `Pin::new_unchecked` pin anything |
| It costs something | Size 0, alignment 1, no code |
| It affects `Send`, `Sync`, `Copy` | It is `Send`, `Sync` and `Copy` itself and changes none of them |
| It cannot be overridden | `impl Unpin for MyType {}` overrides it, in safe code (lesson 4, `Reckless`) |

**The drop guarantee, measured.** The second half of the pinning contract says the memory of a pinned value is
not reused until its destructor has run. `Session` makes that visible. It notes its address when it is pinned
and again inside `drop`. For both pinned sessions the two addresses are equal.

**`Drop` and pinned values: a real trap.** Look at the signature every destructor has:

```rust
fn drop(&mut self)
```

`&mut self`. Not `Pin<&mut Self>`. The destructor of a pinned value is handed exactly the reference `Pin`
exists to withhold. `Drop` is older than `Pin` and its signature could not be changed. So inside `drop` the
compiler will happily let you `mem::take` a field of a value that is, by contract, still pinned. The standard
library's advice is to re-pin immediately and do the work in a function that takes the pinned form:

```rust
impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: the value is being dropped and is never used again, so
        // nothing can move it after this point. And `finish` only reads.
        let this = unsafe { Pin::new_unchecked(self) };
        Session::finish(this);
    }
}
```

From there on the type checker is on your side again: `finish` receives `Pin<&mut Self>` and cannot move
anything out by accident.

## E · The solution: how to mark a type, in practice

```rust
pub struct Session {
    name: String,
    pinned_at: Cell<usize>,
    log: Log,
    _address_sensitive: PhantomPinned,
}
```

1. Add one private field of type `PhantomPinned`. The usual name is `_pin` or `_pinned`; this course spells
   out `_address_sensitive` so that the purpose is in the name.
2. Initialize it with `PhantomPinned`.
3. Give the methods that rely on a stable address a pinned receiver: `self: Pin<&Self>` or
   `self: Pin<&mut Self>`.
4. Never write `impl Unpin` for the type.
5. If the type has a `Drop` impl, treat `self` as pinned inside it.

`Session` holds no pointer into itself at all, and it contains no `unsafe` apart from the one re-pinning line
in `drop`. The marker is not about raw pointers. It is how *any* type says: "once you have pinned me, keep me
here."

## F · The important lines

```rust
pub fn remember_where_i_am(self: Pin<&Self>) {
    self.pinned_at.set(self.address());
}
```
A pinned receiver, shared this time. The method cannot be called on an unpinned `Session`, so the address it
records is one the value will keep. `Cell` allows the write through a shared reference.

```rust
let boxed = Box::new(session);
let second = boxed.address();
```
Block 3. `Box::new`, not `Box::pin`: an ordinary box, an ordinary move, and no complaint from the compiler
although `Session` is `!Unpin`. `!Unpin` restricts nothing by itself.

```rust
let session = Box::pin(Session::new("on the heap", &log));
session.as_ref().remember_where_i_am();
```
`Box::pin` is where the promise starts. `as_ref()` turns `&Pin<Box<Session>>` into `Pin<&Session>`, the
receiver the method asks for.

```rust
{
    let session = pin!(Session::new("on the stack", &log));
    session.as_ref().remember_where_i_am();
}
```
The same promise for the length of one scope. The value sits in a hidden local of this block and is dropped
at the closing brace, at the address where it was pinned.

```rust
let this = unsafe { Pin::new_unchecked(self) };
```
`Pin::new_unchecked` is the unsafe constructor: "trust me, this pointee will not be moved again." Here that
is easy to justify, because the value is in the middle of being destroyed.

## G · Experiment

1. **Remove the marker from `Session`.** Delete the field and its initializer.
   *Prediction:* the lesson still compiles and prints the same thing. Nothing here *needs* the protection,
   because `Session` holds no self-pointer. What you lose is the guarantee: `get_mut` becomes available on
   `Pin<&mut Session>`, so a caller could `mem::swap` two pinned sessions and the "same address" lines would
   stop being a promise and become a coincidence.
2. **Pin too late.** In block 4, call `Box::new` first, record the address through a temporary `Pin`, and
   only then convert with `Box::into_pin`. *Prediction:* `remember_where_i_am` is not callable before the
   conversion. The pinned receiver stops you from recording an address the value is not committed to.
3. **Ask Miri about the destructor.** `cargo +nightly miri test lesson05`.
   *Measured:* 7 tests pass, no error.

## H · Common mistakes

- **"I added `PhantomPinned`, so my struct is pinned now."** It is merely *pinnable in a meaningful way*.
- **"I don't need the marker, my struct has a raw pointer in it."** Raw pointers are `Unpin` (lesson 4).
  Without the marker, `Pin` gives your callers `get_mut`.
- **Making the marker field public and letting callers build the struct already "linked".** Keep the fields
  that carry the invariant private, and construct through a function that pins first.
- **Moving fields out in `Drop`.** `drop` hands you `&mut self` even when the value is pinned. Re-pin first.
- **Adding `#[derive(Clone)]` to an address-sensitive type.** The clone would copy the internal pointer, and
  the copy points into the *original*. If such a type needs `Clone`, write it by hand and re-link.
- **Putting `PhantomPinned` in a type "to be safe".** It forces every user to pin. Use it when an address
  really matters, and only then.

## I · Challenge

`Pin::set` again, this time with a type that tells us when it dies:

```rust
let mut slot = Box::pin(Session::new("old", &log));
slot.as_ref().remember_where_i_am();
slot.set(Session::new("new", &log));
```

Before you run it: at the moment `set` returns, how many farewells are in the log? At which address was
"old" dropped? Where does "new" live, and what does its `pinned_at` say?

**Expected result:** one farewell, for "old", dropped at the very address it was pinned at. `set` ran its
destructor in place first, which is the drop guarantee being honoured. "new" now lives at that same address
and its `pinned_at` is still 0, because nobody has called `remember_where_i_am` on it. The test
`set_drops_the_old_value_in_place_before_writing_the_new_one` checks each of those.

The question for lesson 6: *we now have a value that must stay put behind a `Pin<Box<_>>`. The most common
such value in real Rust is a future. Why is a `Box<dyn Future>` not enough to poll it?*

## Documentation

- `PhantomPinned`: https://doc.rust-lang.org/std/marker/struct.PhantomPinned.html
- `PhantomData`: https://doc.rust-lang.org/std/marker/struct.PhantomData.html
- `std::pin`, the drop guarantee: https://doc.rust-lang.org/std/pin/#drop-guarantee
- `std::pin`, "Implementing `Drop` for types with address-sensitive states":
  https://doc.rust-lang.org/std/pin/#implementing-drop-for-types-with-address-sensitive-states
- `Pin::new_unchecked`: https://doc.rust-lang.org/std/pin/struct.Pin.html#method.new_unchecked
- `Box::pin` and `Box::into_pin`: https://doc.rust-lang.org/std/boxed/struct.Box.html#method.into_pin
