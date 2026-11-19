# Lesson 1 · What a move really is

Part 1, Understand memory. Code: `src/lesson01_moves.rs`, tests in `src/lesson01_moves/tests.rs`.

```console
$ cargo run -- 1
$ cargo test lesson01
$ cargo +nightly miri run -- 1        # optional: no unsafe here, Miri agrees
```

`Pin` exists to stop one thing: a value being moved. So before `Pin` can make sense, we need to know exactly what
a move is, and why Rust is normally free to do it whenever it likes.

## A · The problem

You write `let b = a;`, or pass a value to a function, or push it into a `Vec`. Rust calls each of these a
*move*. What actually happened to the bytes? Did anything get copied? Is the value still "the same value"
afterwards? And why is the compiler allowed to do this without asking you?

Our subject is the type this whole course is built around, one frame of data read from a connection:

```rust
pub struct Frame {
    pub id: u32,
    pub payload: String,
}
```

## B · The naive implementation

There is nothing naive to fix yet. This lesson only looks. `locate` reads four addresses out of a `Frame`, as
plain numbers:

```rust
pub fn locate(frame: &Frame) -> Location {
    Location {
        frame: std::ptr::from_ref(frame).addr(),
        payload_field: std::ptr::from_ref(&frame.payload).addr(),
        id_field: std::ptr::from_ref(&frame.id).addr(),
        text: frame.payload.as_ptr().addr(),
    }
}
```

Then `run` moves one `Frame` five ways and calls `locate` after each. The real output of one run (the addresses
differ on every run and every machine; the pattern does not):

```text
size of Frame: 32 bytes
size of String: 24 bytes

0. created in a local variable
  struct         0x00016d988b40  same
  payload field  0x00016d988b40  same
  id field       0x00016d988b58  same
  text           0x000102d05e10  same
  payload is 0 bytes into the struct, id is 24

1. moved into a function
  struct         0x00016d988c50  MOVED
  payload field  0x00016d988c50  MOVED
  id field       0x00016d988c68  MOVED
  text           0x000102d05e10  same

2. moved into a Box
  struct         0x000b61000940  MOVED
  payload field  0x000b61000940  MOVED
  id field       0x000b61000958  MOVED
  text           0x000102d05e10  same

3. moved into a Vec
  struct         0x000102d05c60  MOVED
  payload field  0x000102d05c60  MOVED
  id field       0x000102d05c78  MOVED
  text           0x000102d05e10  same

4. the Vec grew
  struct         0x000b60c00000  MOVED
  payload field  0x000b60c00000  MOVED
  id field       0x000b60c00018  MOVED
  text           0x000102d05e10  same

5. swapped with its neighbour
  struct         0x000b60c00020  MOVED
  payload field  0x000b60c00020  MOVED
  id field       0x000b60c00038  MOVED
  text           0x000102d05e10  same

five moves, and the text never moved: true
frame 7 still reads "GET /health"
```

Three things to notice:

1. **The struct's address changed every time.** Five moves, five new places.
2. **The fields moved with it**, and kept their distance from the start of the struct (0 and 24 bytes here).
3. **The text never moved.** Its address is the same in every block.

## C · What happens in memory

The addresses below are **illustrative**: round numbers chosen to be readable, not taken from a run.

A `Frame` is 32 bytes on this machine. A `String` is three machine words (a pointer to the text, a capacity, a
length), and those three words are *inside* the `Frame`. The text itself is somewhere else, on the heap.

```text
 STACK                                        HEAP
 0x7000  ┌───────────────────────────┐
         │ payload.ptr  = 0x5000 ────┼──────▶ 0x5000  "GET /health"
 0x7008  │ payload.cap  = 11         │
 0x7010  │ payload.len  = 11         │
 0x7018  │ id           = 7          │
         └───────────────────────────┘
           Frame, 32 bytes, at 0x7000
```

Now the frame is moved, for example into a `Box`. A move copies those 32 bytes to the new place. Nothing else.

```text
 STACK                                        HEAP
 0x7000  ┌───────────────────────────┐
         │ (old bytes, now dead:     │        0x5000  "GET /health"      ◀──┐
         │  the compiler will not    │                                      │
         │  let anyone read them)    │        0x6000 ┌──────────────────┐   │
         └───────────────────────────┘               │ ptr  = 0x5000 ───┼───┘
                                                     │ cap  = 11        │
 0x7020  ┌───────────────────────────┐               │ len  = 11        │
         │ Box pointer = 0x6000 ─────┼─────────────▶ │ id   = 7         │
         └───────────────────────────┘               └──────────────────┘
                                                       Frame, now at 0x6000
```

What moved: the 32 bytes of the struct (0x7000 → 0x6000), and therefore both fields.
What stayed: the text at 0x5000. The copied pointer still holds the number 0x5000, and 0x5000 is still where the
text is. The pointer is as good as it was.

That last sentence is the whole lesson. **A pointer is only a number. Copying the number somewhere else does not
make it wrong, as long as the thing it names has not gone anywhere.**

Step 4 deserves a second look, because we never wrote a move there:

```rust
frames.reserve(1024);
```

The `Vec` ran out of room, asked for a bigger block, and copied every element into it. Our frame moved
"by itself", inside a library call. In Rust a value can be moved at any time by any code that owns it or holds
`&mut` to it. Nothing in the language announces it.

## D · The compiler's perspective

- **A move is a bitwise copy of the value, plus a rule.** The copy is what you saw: the bytes land in a new
  place. The rule is that the old place may not be used again. That rule is checked at compile time, and it is
  why two `String`s never end up owning the same text.
- **Rust types carry no "moved" hook.** There is no move constructor to run. So a type cannot notice that it
  moved, and cannot fix itself up afterwards. Whatever was true of the bytes must still be true after they are
  copied somewhere else.
- **For an ordinary type, that is automatically so.** A `Frame` holds a number and a `String`. Neither contains
  the address of the `Frame`. Nothing inside it depends on where it is.
- **Whether an address actually changes is up to the compiler.** In step 1 the function received its own copy at
  a different address, in both a debug and a release build of this program. An optimizer is allowed to skip a
  copy when nobody can tell. Never write code that relies on a move changing an address, or on it not changing.
- **Field order is the compiler's choice too.** We declared `id` first, and measured `payload` at offset 0 and
  `id` at offset 24. The default representation does not promise declaration order.

The intentionally invalid file `invalid/lesson01_use_after_move.rs` shows the rule being enforced. It is not
part of the build. Its real error:

```text
error[E0382]: borrow of moved value: `frame`
  --> invalid/lesson01_use_after_move.rs:14:33
   |
12 |     let frame = Frame { id: 7, payload: String::from("GET /health") };
   |         ----- move occurs because `frame` has type `Frame`, which does not implement the `Copy` trait
13 |     let moved = frame;
   |                 ----- value moved here
14 |     println!("{} {}", frame.id, frame.payload);
   |                                 ^^^^^^^^^^^^^ value borrowed here after move
```

## E · The solution

There is no problem to solve in this lesson, and that is the point. Moving a normal struct is safe because
**nothing points into it**. Keep that sentence. Lesson 2 breaks exactly that condition, and everything `Pin` does
follows from it.

## F · The important lines

```rust
frame: std::ptr::from_ref(frame).addr(),
```
`ptr::from_ref` turns `&Frame` into `*const Frame` without a cast, and `.addr()` gives the address as a `usize`.
Both are safe: we look at the number and never read memory through it. The whole course keeps this habit, so
that we can talk about stale pointers without ever touching one.

```rust
text: frame.payload.as_ptr().addr(),
```
`String::as_ptr` is the address of the first byte of the text on the heap. It is a different address from
`&frame.payload`, which is where the String's three words are. Keeping those two apart is the key to lesson 2.

```rust
fn into_function(frame: Frame) -> (Frame, Location) {
```
Taking `Frame` by value is a move in, and returning it is a move out. We measure in between.

```rust
frames.reserve(1024);
```
No assignment, no function taking a `Frame`. A reallocation, and every element is somewhere else.

```rust
let (first, second) = frames.split_at_mut(1);
mem::swap(&mut first[0], &mut second[0]);
```
`mem::swap` needs two `&mut Frame` at once, so the slice is split in two. Then it exchanges the bytes of the two
frames. Hold on to this: **anyone with `&mut T` can move a `T`**, with `swap`, `replace` or `take`. Lesson 3
starts there.

## G · Experiment

1. **Release mode.** Run `cargo run --release -- 1`.
   *Prediction:* the same pattern. *Measured:* yes, every struct line still says MOVED on this machine. But
   read section D again: that is an observation about this build, not a promise.
2. **Change the layout.** Put `#[repr(C)]` above `struct Frame`.
   *Prediction:* the fields now follow declaration order. *Measured:* `id` at offset 0, `payload` at offset 8,
   size still 32. The moves behave exactly as before.
3. **An empty payload.** Build the frame with `Frame::new(7, "")`.
   *Prediction:* there is no text, so there is no heap block. *Measured:* the text address prints as `0x1` and
   the capacity is 0. An empty `String` does not allocate; its pointer is a placeholder that is never read. It
   still says "same" after every move, because the placeholder is copied like any other number.

## H · Common mistakes

- **"Moving a `String` copies the text."** It copies three words. The text stays. (`clone` copies the text.)
- **"After a move the old variable is cleared."** Nothing is cleared. The bytes are left where they were. What
  changes is that the compiler stops letting you name them.
- **"`&frame.payload` is where the text is."** It is where the String's pointer, capacity and length are, inside
  the struct. The text is at `frame.payload.as_ptr()`.
- **"A value only moves when I write a move."** A `Vec` that grows moves its elements. So do `sort`, `swap`,
  `insert`, `mem::replace`, and returning from a function.
- **"If the address did not change, it was not a move."** A move is a matter of ownership. Whether bytes were
  copied is an implementation detail.

## I · Challenge

Put a `Frame` in a `Box`. Then move the *box* twice: first into another variable, then into an array.

```rust
let boxed = Box::new(Frame::new(1, "hello"));
let before = locate(&boxed);
let moved_box = boxed;
let shelf = [moved_box];
let after = locate(&shelf[0]);
```

Before you run it: which of the four addresses in `after` differ from `before`?

**Expected result:** none. `after == before`. A `Box` is one pointer. Moving the box copies that pointer; the
`Frame` it points at is never touched. The test `moving_a_box_moves_the_pointer_not_the_frame` checks it.

This is the first hint of how the course ends up: if a value must not move, put it behind a pointer and move the
pointer.

## Documentation

- The Rust Reference, *Expressions*, moved and copied types:
  https://doc.rust-lang.org/reference/expressions.html#moved-and-copied-types
- `std::mem::swap`: https://doc.rust-lang.org/std/mem/fn.swap.html
- `std::ptr::from_ref`: https://doc.rust-lang.org/std/ptr/fn.from_ref.html and `pointer::addr`:
  https://doc.rust-lang.org/std/primitive.pointer.html#method.addr
- `String::as_ptr`: https://doc.rust-lang.org/std/string/struct.String.html#method.as_ptr
- The Rust Reference, *Type layout*, the default representation:
  https://doc.rust-lang.org/reference/type-layout.html#the-rust-representation
- `std::pin`, the module documentation this course keeps returning to: https://doc.rust-lang.org/std/pin/
