# Lesson 2 · A struct that points at itself

Part 1, Understand memory. Code: `src/lesson02_self_referential.rs`, tests in
`src/lesson02_self_referential/tests.rs`, and `examples/address_stability.rs`.

```console
$ cargo run -- 2
$ cargo run --example address_stability
$ cargo test lesson02
$ cargo +nightly miri test lesson02      # raw pointers are stored and compared, never read through
```

Lesson 1 ended on one sentence: *moving a normal struct is safe because nothing points into it.* This lesson
builds a struct where something does.

## A · The problem

Our `Frame` holds a line read from a connection. A parser wants to keep its place: a cursor that says "the
payload is right here", so that later code does not have to find it again. The natural thing to store is a
pointer to the payload. And the payload is a field of the same struct.

That is a **self-referential struct**: one of its fields holds the address of another of its fields.

## B · The naive implementation

**First try: a reference.** It is what Rust teaches us to reach for.

```rust
struct Frame<'a> {
    payload: String,
    cursor: &'a String,
}

fn build<'a>() -> Frame<'a> {
    let payload = String::from("GET /health");
    Frame { cursor: &payload, payload }
}
```

This is `invalid/lesson02_reference_to_self.rs`. It is not part of the build. Its real errors:

```text
error[E0515]: cannot return value referencing local variable `payload`
  --> invalid/lesson02_reference_to_self.rs:15:5
   |
15 |     Frame { cursor: &payload, payload }
   |     ^^^^^^^^^^^^^^^^--------^^^^^^^^^^^
   |     |               |
   |     |               `payload` is borrowed here
   |     returns a value referencing data owned by the current function

error[E0505]: cannot move out of `payload` because it is borrowed
  --> invalid/lesson02_reference_to_self.rs:15:31
   |
15 |     Frame { cursor: &payload, payload }
   |     --------------------------^^^^^^^--
   |     |               |         |
   |     |               |         move out of `payload` occurs here
   |     |               borrow of `payload` occurs here
   |     returning this value requires that `payload` is borrowed for `'a`
```

Read them as one sentence: *putting `payload` into the struct is a move, and you are holding a reference to the
place it is moving away from.* The borrow checker is not being fussy. It has seen the exact problem of this
lesson, and it has no way to express "this reference should follow the value".

**Second try: a raw pointer.** A raw pointer has no lifetime, so the borrow checker has nothing to say.

```rust
pub struct Frame {
    pub id: u32,
    pub payload: String,
    cursor: *const String,
}

pub fn link(&mut self) {
    self.cursor = ptr::from_ref(&self.payload);
}
```

It compiles. It needs no `unsafe`, because creating and storing a raw pointer is safe. Here is one real run:

```text
size of Frame: 40 bytes

1. linked in a local variable
  payload field is at  0x00016ce04c38
  cursor says          0x00016ce04c38
  points at itself:    true

2. moved into a Box
  payload field is at  0x00010368dca0
  cursor says          0x00016ce04c38
  points at itself:    false
  the cursor still holds the OLD address of the field

3. a cursor into the text, moved the same way
  before true, after true

4. a cursor that is a position, moved the same way
  payload: "/health"

5. boxed first, then linked
  payload field is at  0x00010368dd40
  cursor says          0x00010368dd40
  points at itself:    true
  after moving the box: true

6. the contents of two boxes swapped
  a points at itself: false
  b points at itself: false
  a's cursor points into b: true
```

Step 1 is fine. Step 2 is the bug: **the payload field is at one address, and the cursor names another.**
Nothing warned us. The compiler is silent, the program does not crash, and `Frame` has no way to notice.

## C · What happens in memory

The addresses below are **illustrative**, chosen to be readable.

After `link`, on the stack. The cursor holds the address of the field two lines above it:

```text
 STACK                                        HEAP
 0x7000  ┌───────────────────────────┐
   ┌───▶ │ payload.ptr  = 0x5000 ────┼──────▶ 0x5000  "GET /health"
   │     │ payload.cap  = 11         │
   │     │ payload.len  = 11         │
   │     │ cursor       = 0x7000 ──┐ │
   │     │ id           = 7        │ │
   │     └─────────────────────────┼─┘
   └───────────────────────────────┘     the frame points at itself
```

Now the frame is moved into a `Box`. Lesson 1 told us what a move is: the bytes are copied, all 40 of them, the
cursor included. The number `0x7000` arrives in the new place unchanged.

```text
 STACK                                        HEAP
 0x7000  ┌───────────────────────────┐
   ┌───▶ │ (dead bytes: the frame    │        0x5000  "GET /health"   ◀──┐
   │     │  that used to live here)  │                                   │
   │     └───────────────────────────┘        0x6000 ┌───────────────┐   │
   │                                                 │ ptr = 0x5000 ─┼───┘
   │                                                 │ cap = 11      │
   │                                                 │ len = 11      │
   └─────────────────────────────────────────────────┼─ cursor=0x7000│
                                                     │ id  = 7       │
                                                     └───────────────┘
                                                       the frame, now at 0x6000
```

Compare the two pointers inside the frame:

- `payload.ptr` holds `0x5000`, the text. The text did not move. **Still right.**
- `cursor` holds `0x7000`, where the payload field *used to be*. The field is now at `0x6000`. **Wrong.**

Both were copied in exactly the same way. The difference is only in what they point at: one points *out of* the
struct, to something that stayed. The other points *into* the struct, and the struct left.

That is what **address-sensitive** means: the value is only correct while it sits at one particular address.

And the stale cursor is not merely useless. `0x7000` is memory the program will use again for something else.
Reading a `String` through it would be reading whatever happens to be there, as if it were a pointer, a capacity
and a length. That is undefined behaviour, which is why this lesson only ever compares the two numbers.

## D · The compiler's perspective

- **References cannot express this.** `&'a String` promises the target stays put, and stays borrowed, for all
  of `'a`. A struct that holds a reference to its own field would be borrowed for as long as it exists, so it
  could never be moved or mutably used again. The language has no "relative" reference. E0505 and E0515 are that
  fact, reported.
- **Raw pointers switch the checks off.** `*const String` carries no lifetime and no borrow. Making one, copying
  it and comparing it are all safe operations. Only *dereferencing* it is `unsafe`, and that is exactly where
  the promise "this still points at a live `String`" would have to be made by us.
- **The type cannot defend itself.** There is no move constructor (lesson 1), so `Frame` is never told that it
  moved and cannot re-aim its cursor.
- **So the guarantee has to come from outside the type:** from a rule that says *this particular value will
  never be moved again*. Rust has no such rule for ordinary values. Any owner may move a value, and so may
  anyone holding `&mut` to it. Step 6 shows the second half of that.

## E · Solutions, and how far each one goes

**1. Do not point at yourself: store a position.**

```rust
pub struct OffsetFrame {
    pub line: String,
    payload_starts: usize,
}

pub fn payload(&self) -> &str {
    &self.line[self.payload_starts..]
}
```

"The payload starts at byte 4" is true wherever the frame is. This is the right answer for most real code, and
you should reach for it first. It costs one bounds-checked slice when you read.

**2. Point at something that does not move with you.** Step 3: a cursor to the *text* survives the move,
because the text is on the heap and a move of the struct does not touch it. This is fragile in a different way
(the experiment below), so it is not a general answer either.

**3. Put the frame where it will stay, then link it.** Step 5:

```rust
let mut first = Box::new(Frame::new(1, "first"));
first.link();
```

The frame is linked at its final address. Moving the box afterwards moves one pointer, and the frame stays
(the challenge of lesson 1). `examples/address_stability.rs` puts the cases side by side:

```text
the frame lives in     what we did                the frame
a local variable       passed it to a function    MOVED
a Vec<Frame>           the Vec grew               MOVED
a Box<Frame>           moved the box              stayed
a Vec<Box<Frame>>      the Vec grew               stayed
a Box<Frame>           swapped the contents       MOVED
```

**And the last row is why this is still not enough.** A `Box<Frame>` hands out `&mut Frame` to anyone who
asks, and `&mut Frame` is all `mem::swap` needs:

```rust
mem::swap(&mut *a, &mut *b);
```

Step 6: both cursors are now wrong, and worse than dangling. Each one names a live, valid payload field. It is
just the payload of the *other* frame. No crash, no error, wrong data.

So we know exactly what is missing. We need a pointer to a `Frame` that lets us use the frame, and that does
**not** hand out the one thing that makes a move possible: unrestricted `&mut Frame`. That pointer is `Pin`,
and it is lesson 3.

## F · The important lines

```rust
cursor: *const String,
```
A raw pointer to the `String` *field*, the three words inside the frame. Not to the text. That choice is what
makes the struct address-sensitive.

```rust
Frame { id, payload: payload.to_string(), cursor: ptr::null() }
```
The cursor starts null on purpose. We cannot aim it yet: the frame we are building here is about to be moved
out of `new` to wherever the caller puts it. A self-referential value can only be linked *after* it has reached
the place where it will stay. Every sound design for this problem has that two-step shape.

```rust
pub fn link(&mut self) {
    self.cursor = ptr::from_ref(&self.payload);
}
```
`&self.payload` is the field's address at this instant. `ptr::from_ref` turns the reference into a raw pointer,
which ends the borrow: nothing ties the stored number to the frame any more.

```rust
pub fn points_at_itself(&self) -> bool {
    self.cursor_says() == self.payload_is_at()
}
```
Two addresses as `usize`, compared. This is the whole measuring instrument of the lesson, and it is safe.

```rust
mem::swap(&mut *a, &mut *b);
```
`&mut *a` reborrows the frame inside the box. `swap` exchanges the two frames' bytes, cursors included. The
boxes themselves do not move.

## G · Experiment

1. **Grow the text.** In step 3, make the box mutable and push 4,096 characters onto `text.payload` before the
   check. *Prediction:* the String needs a bigger block, so the text moves, and the cursor into the text is now
   stale. *Measured on a plain `String`:* capacity 11 → 4107, text moved: true. A cursor into the heap survives
   a move of the struct, and breaks on a reallocation of the buffer. Different trigger, same disease.
2. **Link twice.** After the move in step 2, call `boxed.link()` again (make it `mut`). *Prediction:* it points
   at itself again. *Why it is not a fix:* someone has to remember to call it after every move, and moves are
   invisible (lesson 1, the growing `Vec`).
3. **Run the tests under Miri.** `cargo +nightly miri test lesson02`. *Prediction:* clean. *Measured:* 15 tests
   pass, no error. Storing and comparing a stale raw pointer is not undefined behaviour. Reading through it
   would be, and Miri would say so.

## H · Common mistakes

- **"I'll link it in `new`."** `new` returns the frame by value. That return is a move, so the cursor would be
  stale before the caller ever sees it.
- **"A raw pointer compiled without `unsafe`, so it's fine."** Safe to *create*. The danger is deferred to the
  first dereference.
- **"`Box` fixes it."** `Box` gives a stable address only while nobody uses `&mut` to move the contents.
  `mem::swap`, `mem::replace`, `mem::take` and `*boxed = other` all do.
- **"A stale pointer is a null pointer."** It is neither null nor obviously bad. After the swap it points at a
  perfectly valid field of another frame.
- **"Self-referential means it holds a pointer to its heap data."** A `String` or a `Vec` holds a pointer to
  its own heap buffer and moves freely. Address-sensitive means pointing into the *struct's own bytes*.
- **"`Rc`, lifetimes or `'static` can express it."** They cannot make the field stop moving with the struct.

## I · Challenge

`mem::swap` is not the only way out through `&mut`. Link a boxed frame, then replace it:

```rust
let mut boxed = Box::new(Frame::new(1, "first"));
boxed.link();
let taken = mem::replace(&mut *boxed, Frame::new(2, "second"));
```

Before you run it, answer three questions. Does `taken` point at itself? What address does `taken`'s cursor
hold? And what lives at that address now?

**Expected result:** `taken.points_at_itself()` is `false`. Its cursor still holds the address of the payload
field inside the box, and that slot now belongs to frame 2: `taken.cursor_says() == boxed.payload_is_at()`. The
test `replace_moves_a_frame_out_through_a_mutable_reference` checks all three.

Then the real question, to carry into lesson 3: *what is the smallest change to the type system that would have
made both `swap` and `replace` refuse to compile here, and still allowed them for the `Frame` of lesson 1?*

## Documentation

- `std::pin`, "Address-sensitive values, AKA when we need pinning":
  https://doc.rust-lang.org/std/pin/#address-sensitive-values-aka-when-we-need-pinning
- Raw pointers, `*const T` and `*mut T`: https://doc.rust-lang.org/std/primitive.pointer.html
- `std::ptr::null`: https://doc.rust-lang.org/std/ptr/fn.null.html
- `std::mem::swap`: https://doc.rust-lang.org/std/mem/fn.swap.html and `std::mem::replace`:
  https://doc.rust-lang.org/std/mem/fn.replace.html
- The compiler's explanations: `rustc --explain E0505`, `rustc --explain E0515`
- The Rustonomicon, "Meet Safe and Unsafe": https://doc.rust-lang.org/nomicon/meet-safe-and-unsafe.html
