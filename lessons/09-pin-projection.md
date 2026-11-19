# Lesson 9 · Pin projection

Part 4, Real-world application. Code: `src/lesson09_pin_projection.rs`, tests in
`src/lesson09_pin_projection/tests.rs`.

```console
$ cargo run -- 9
$ cargo test lesson09
$ cargo +nightly miri test lesson09      # this lesson has hand-written unsafe projections: Miri checks them
```

This lesson adds the course's first dependency for a concept: `pin-project` 1.1.13, a small macro crate that
many async libraries build on.

## A · The problem

Real futures wrap other futures: a timeout around a read, a retry around a request, a metric around
anything. Our wrapper counts how often its inner future is polled:

```rust
struct Timed<F> {
    inner: F,
    polls: u32,
}
```

In its `poll` we are given `Pin<&mut Timed<F>>`, and we need two different things out of it:

- for `inner`: a **`Pin<&mut F>`**, because that is what `F::poll` takes;
- for `polls`: a plain **`&mut u32`**, to add one.

Going from a pinned struct to a reference to one of its fields is called **projection**. `Pin` does not give
it to us, and for a good reason.

## B · The naive implementation

`invalid/lesson09_fields_through_a_pin.rs`, not part of the build:

```rust
fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    self.polls += 1;
    self.inner.poll(cx)
}
```

```text
error[E0599]: no method named `poll` found for type parameter `F` in the current scope
21 |         self.inner.poll(cx)
   |                    ^^^^ method not found in `F`
help: consider pinning the expression
21 ~         let mut pinned = std::pin::pin!(self.inner);
```

`self.inner` goes through `Deref` and is a plain `F` place, and `poll` is not a method on `F`: it is a method
on `Pin<&mut F>`. (The compiler's hint does not fit this case. `pin!(self.inner)` would have to move the
future *out* of our struct, which is the opposite of what we want.) And once that line is dealt with,
`self.polls += 1` fails the way lesson 3's assignment did: E0594, no `DerefMut` for a `Pin` around a type that
may not be `Unpin`.

So we have a pinned struct and no way in. The lesson is four ways in.

## The working example

One real run of all four:

```text
1. four wrappers around read_two's future
  the future itself                56 bytes  !Unpin
  TimedUnpin (boxed by caller)     16 bytes  Unpin
  TimedBoxed                       16 bytes  Unpin
  TimedByHand                      64 bytes  !Unpin
  Timed (pin-project)              64 bytes  !Unpin

2. run each one
  TimedUnpin   Some(("HEADER | BODY", 3))
  TimedBoxed   Some(("HEADER | BODY", 3))
  TimedByHand  Some(("HEADER | BODY", 3))
  Timed        Some(("HEADER | BODY", 3))

3. a self-referential future inside TimedByHand
  Some((5, 2))

4. polls so far, read through Pin<&Self>: 0

5. Fused, polled to the end and then again
  first:  Some(Some("LAST"))
  second: Some(None)
  the inner future is gone: true
```

## C · What happens in memory

Illustrative addresses. A `Timed<F>` pinned at `0x6000`, with the inner future stored inline:

```text
 Pin<&mut Timed<F>> ──▶ 0x6000 ┌──────────────────────────────────────┐
                               │ inner: F   (56 bytes)                │ ◀── Pin<&mut F>   = 0x6000
                               │   a parked async future. It may hold │     STRUCTURALLY PINNED:
                               │   pointers into these very bytes.    │     pinned because Timed is
                               ├──────────────────────────────────────┤
                               │ polls: u32                           │ ◀── &mut u32      = 0x6038
                               └──────────────────────────────────────┘     NOT structurally pinned:
                                 one allocation, 64 bytes                   a plain number
```

Projection does not move or copy anything. It is pointer arithmetic: "the struct is at `0x6000`, so `inner` is
at `0x6000` and `polls` is at `0x6038`". The only question is which *type* each of those two pointers gets.

- If `Timed` will not move (it is pinned), then the bytes of `inner` will not move either, since they are part
  of it. So it is *possible* to hand out `Pin<&mut F>` truthfully. We say `inner` is **structurally pinned**:
  the pinning of the struct extends to this field.
- For `polls` we want the opposite: a plain `&mut u32`, with which the number can be overwritten, swapped,
  taken. That is fine for a `u32`. We say `polls` is **not structurally pinned**.

This is a decision the author of the type makes, **per field**. The compiler cannot make it for us, because
both choices are sometimes right, and that is why there is no automatic projection in the language.

The alternative layout, approach 2 below, moves the future out of the struct altogether:

```text
 0x6000 ┌────────────────────────┐          0x7000 ┌─────────────────────────────┐
        │ inner: Pin<Box<F>> ────┼────────────────▶│ F  (56 bytes), pinned here  │
        │ polls: u32             │                 └─────────────────────────────┘
        └────────────────────────┘
          16 bytes, and Unpin: this struct may move freely. The future will not.
```

## D · The compiler's perspective

**The methods of `Pin`, sorted by whether they need `unsafe`.** This is the reference table for the whole
course.

| Method | From → to | Safe? | Why |
|---|---|---|---|
| `as_mut()` | `&mut Pin<P>` → `Pin<&mut T>` | safe | a reborrow; still pinned |
| `as_ref()` | `&Pin<P>` → `Pin<&T>` | safe | a shared reborrow; still pinned |
| `get_ref()` | `Pin<&T>` → `&T` | safe | nothing can be moved through a shared reference |
| `set(value)` | replaces the pointee in place | safe | the old value is dropped where it stands |
| `get_mut()` | `Pin<&mut T>` → `&mut T` | safe, **only if `T: Unpin`** | lesson 4 |
| `into_inner()` | `Pin<P>` → `P` | safe, **only if the target is `Unpin`** | lesson 4 |
| `Option::as_pin_mut()` | `Pin<&mut Option<T>>` → `Option<Pin<&mut T>>` | safe | the library made the structural choice for `Option` |
| `get_unchecked_mut()` | `Pin<&mut T>` → `&mut T` | **unsafe** | you must not move the value or leave it invalid |
| `map_unchecked_mut(f)` | `Pin<&mut T>` → `Pin<&mut U>` | **unsafe** | you assert that `U` is structurally pinned in `T` |
| `map_unchecked(f)` | `Pin<&T>` → `Pin<&U>` | **unsafe** | the same assertion, shared |
| `Pin::new_unchecked(p)` | `P` → `Pin<P>` | **unsafe** | you assert the pointee will not move again |

Reading is always safe (`get_ref`). Getting `&mut` or a pinned reference to a *part* is where the promises
are made.

**What you owe for declaring a field structurally pinned.** From the standard library's documentation of
`std::pin`. These four are written above `TimedByHand` in the code, because that is where they have to be
kept true:

1. **`Unpin`.** The struct may be `Unpin` only if every structurally pinned field is. The automatic
   implementation already does exactly that. So: no hand-written `impl Unpin`.
2. **`Drop`.** The destructor receives `&mut self` (lesson 5). It must not move a structurally pinned field
   out.
3. **No `#[repr(packed)]`.** A packed struct may have to move a field to drop it, and its fields are not at
   aligned addresses.
4. **No other way out.** No method that, given `Pin<&mut Self>`, moves the field out or overwrites it
   without running its destructor.

And for a field that is *not* structurally pinned: never create a `Pin<&mut Field>` to it. One field, one
choice.

**What `pin-project` does about that list.** `invalid/lesson09_pin_project_refuses.rs` breaks items 2 and 3
on purpose. Written by hand, both compile. With the macro:

```text
error: #[pin_project] attribute may not be used on #[repr(packed)] types
24 | #[repr(packed)]

error[E0119]: conflicting implementations of trait `WithDropMustNotImplDrop` for type `WithDrop<_>`
12 | #[pin_project]
```

The second message is the macro's trick showing through: it generates an impl of a private trait that
conflicts with any `Drop` impl you write. (If you need a destructor, the crate has `#[pinned_drop]`, which
hands you `Pin<&mut Self>`.) It also generates the conditional `Unpin` impl of item 1.

**A finding from this lesson's own code.** The first version of `TimedByHand::poll` read the counter like
this:

```rust
let polls = self.as_ref().polls_so_far();      // Pin<&Self> → &Self → .polls
```

Safe code, correct result. Miri's default model rejected it, with the same report as lesson 8, pointing at
that line: a shared reference to the *whole* struct covers the parked inner future as well, including the
reference it holds into itself. The fix is the habit this lesson is about: **project to the field you need,
and do not take a reference to the whole pinned struct while a part of it is parked.** The code now reads
the counter through the `&mut u32` projection. (As in lesson 8: Tree Borrows accepted both versions, and no
`unsafe` was involved in the rejected line.)

## E · Four solutions

**1. Require `F: Unpin`.** `TimedUnpin<F>`.

```rust
let this = self.get_mut();
this.polls += 1;
Pin::new(&mut this.inner).poll(cx)
```
No `unsafe`. The cost is pushed to the caller: an `async` future is not `Unpin`, so the caller has to write
`TimedUnpin::new(Box::pin(read_two(&inbox)))`. Many early async APIs looked like this.

**2. Box the inner future.** `TimedBoxed<F>`, with `inner: Pin<Box<F>>`.

```rust
let this = self.get_mut();
this.polls += 1;
this.inner.as_mut().poll(cx)
```
No `unsafe`, any `F`. `Pin<Box<F>>` is `Unpin` whatever `F` is (lesson 4), so the wrapper is too. The cost is
one heap allocation per wrapper. The run measured it: 16 bytes inline against 64.

**3. Project by hand.** `TimedByHand<F>`.

```rust
fn inner(self: Pin<&mut Self>) -> Pin<&mut F> {
    // SAFETY: `inner` is structurally pinned (see the type's notes).
    unsafe { self.map_unchecked_mut(|this| &mut this.inner) }
}

fn polls(self: Pin<&mut Self>) -> &mut u32 {
    // SAFETY: `polls` is never pinned, and through this `&mut Self`
    // we touch only that field. Nothing is moved out of `inner`.
    unsafe { &mut self.get_unchecked_mut().polls }
}
```
No allocation, any `F`, and two `unsafe` blocks whose correctness depends on the four-item list staying true
for as long as the type exists, across every future edit.

**4. Let a macro write number 3.** `Timed<F>`.

```rust
#[pin_project]
pub struct Timed<F> {
    #[pin]
    inner: F,
    polls: u32,
}

let this = self.project();
*this.polls += 1;
this.inner.poll(cx)
```
`#[pin]` marks the structurally pinned field. `project()` returns a struct whose `inner` is `Pin<&mut F>`
and whose `polls` is `&mut u32`. Same layout and cost as number 3, no `unsafe` in your code, and the list is
enforced by the compiler.

| | Any `F`? | `unsafe` you write | Allocation | Wrapper is `Unpin` |
|---|---|---|---|---|
| 1 `TimedUnpin` | only `Unpin` | none | none (the caller may box) | always |
| 2 `TimedBoxed` | yes | none | one per wrapper | always |
| 3 `TimedByHand` | yes | two blocks | none | when `F` is |
| 4 `Timed` | yes | none | none | when `F` is |

**Which to use.** Number 4 for library code that wraps futures. Number 2 when an allocation does not matter
and you want the wrapper to be `Unpin`. Number 3 when you cannot take a dependency, and then with Miri in
your test suite. Number 1 rarely.

Block 3 of the run is the test that matters for numbers 3 and 4: the inner future is
`borrows_its_own_local`, the self-referential one from lesson 8. It finishes with the right answer, and Miri
is clean on both.

## F · The important lines

```rust
fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let counter = self.as_mut().polls();
```
`polls` takes `self: Pin<&mut Self>` by value, which would use `self` up. `as_mut()` makes a fresh, shorter
`Pin<&mut Self>` from it, so `self` is still there for `self.inner()` two lines later. This reborrow is the
most common line in hand-written `poll` functions.

```rust
unsafe { self.map_unchecked_mut(|this| &mut this.inner) }
```
"Give me a pinned reference to a part of what you point at." The closure must return a reference *into* the
value, never to something else, and never to a field you also hand out unpinned.

```rust
let Some(future) = this.inner.as_mut().as_pin_mut() else {
    return Poll::Ready(None);
};
```
In `Fused`. `this.inner` is `Pin<&mut Option<F>>`. `as_pin_mut` turns it into `Option<Pin<&mut F>>`: a
projection *through an enum*, provided by the standard library and safe.

```rust
let output = std::task::ready!(future.poll(cx));
this.inner.set(None);
```
`ready!` returns `Pending` early if the inner future is not done. `Pin::set(None)` then drops the finished
future in place (lesson 3's challenge) and leaves `None`. No field was moved out.

```rust
self.get_ref().polls
```
In `polls_so_far`. Reading through `Pin<&Self>` is safe. See the finding in section D for when not to do it.

## G · Experiment

1. **Break rule 4 on purpose, and let Miri tell you.** Add this to `TimedByHand`:
   `fn take_inner(self: Pin<&mut Self>) -> F where F: Default { mem::take(unsafe { &mut self.get_unchecked_mut().inner }) }`.
   It compiles. It moves a pinned future out. *Prediction:* nothing fails until someone polls first and takes
   afterwards with a self-referential `F`; `async` futures are not `Default`, so try it with a hand-written
   `!Unpin` future. This is how such bugs look in review: one innocent method. Remove it again.
2. **Mark the wrong field.** In `Timed`, move `#[pin]` from `inner` to `polls`.
   *Prediction:* the `poll` body no longer type-checks: `this.inner` is now `&mut F` and has no `poll`, and
   `this.polls` is `Pin<&mut u32>`. The macro makes the choice visible in the types.
3. **Both aliasing models.** `cargo +nightly miri test lesson09`, then with `MIRIFLAGS=-Zmiri-tree-borrows`.
   *Measured:* 8 tests pass under both, after the fix described in section D.

## H · Common mistakes

- **Treating one field both ways.** A method returning `Pin<&mut F>` and another returning `&mut F` for the
  same field. The second lets a caller `mem::swap` what the first promised would stay.
- **`impl<F> Unpin for Timed<F> {}`** "to make it easier to use". It makes every pinned inner future
  movable. Rule 1.
- **A `Drop` impl that takes a pinned field** (`mem::take`, `Option::take`, a move into another thread).
- **Calling `get_unchecked_mut` and then using the whole `&mut Self` freely.** Touch the one field you
  justified in the comment.
- **Taking `&Self` over a pinned struct with a parked future inside** to read some unrelated field. Project
  to the field.
- **Writing the projection by hand when `pin-project` or `pin-project-lite` is already in the dependency
  tree.** Almost every async project has one of them through its runtime.
- **Thinking projection moves data.** It never does. It only changes the type of a pointer to a part.

## I · Challenge

Lesson 7's contract says a future must not be polled after it returned `Ready`. Write `Fused<F>`: a wrapper
that makes that safe. It polls `F` until it is done, **drops `F` at that moment**, and answers `None` to every
later poll. Store the future as `Option<F>`. You may not use `unsafe`, and you may not require `F: Unpin`.

Before you look at the solution: which field is structurally pinned? How do you get from
`Pin<&mut Option<F>>` to the future inside? And how do you get rid of a pinned future without moving it?

**Expected result:** the `Fused` in this lesson's code. `#[pin] inner: Option<F>`; `as_pin_mut()` for the way
in; `Pin::set(None)` for the way out. Block 5 of the run shows `Some(Some("LAST"))` and then `Some(None)`.
The test `fused_drops_its_future_at_once_and_then_answers_none` uses a future that reports its own drop and
checks that it happens on the completing poll, not later.

The question for lesson 10: *we now have every piece. A real program reads from a real socket with Tokio.
How many times does* our *code have to mention `Pin` to do that, and which of the pieces from these nine
lessons are at work underneath?*

## Documentation

- `std::pin`, "Projections and structural pinning":
  https://doc.rust-lang.org/std/pin/#projections-and-structural-pinning
- `Pin::map_unchecked_mut`: https://doc.rust-lang.org/std/pin/struct.Pin.html#method.map_unchecked_mut
- `Pin::get_ref`, `Pin::get_mut`, `Pin::as_mut`, `Pin::set`: https://doc.rust-lang.org/std/pin/struct.Pin.html
- `Option::as_pin_mut`: https://doc.rust-lang.org/std/option/enum.Option.html#method.as_pin_mut
- `std::task::ready!`: https://doc.rust-lang.org/std/task/macro.ready.html
- `pin-project`: https://docs.rs/pin-project/1.1.13/pin_project/
  (`#[pin_project]`, `#[pin]`, `#[pinned_drop]`)
- `pin-project-lite`, the same idea without a procedural macro: https://docs.rs/pin-project-lite/
