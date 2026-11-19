# Lesson 8 · The async state machine

Part 3, Build a Future. Code: `src/lesson08_async_state_machine.rs`, tests in
`src/lesson08_async_state_machine/tests.rs`, and `examples/parked_future_alias.rs`.

```console
$ cargo run -- 8
$ cargo test lesson08
$ cargo run --example parked_future_alias
$ cargo +nightly miri run --example parked_future_alias     # reports an error, on purpose: see section D
```

**A rule for this lesson.** We never read the compiler's generated code and never claim to know its layout.
Everything said about "what is inside an async future" comes from one of two places: a *measurement* from the
outside (a size, an `Unpin` answer, a behaviour, a Miri report), or a *model* we write by hand to compare.
Each statement below is marked as one or the other.

## A · The problem

Lesson 7 ended on a contrast. Our hand-written `ReadFrame` is `Unpin`. Every future that came out of an
`async fn` in lesson 6 was `!Unpin`, even the one-byte future of `close()`.

What does the compiler put in an `async` future that we did not put in ours? And why does it make the future
address-sensitive?

## B · The working example first

Five small `async fn`s that differ in one thing only: what is still alive when they reach their `.await`.
Then one `async fn`, `read_two`, next to the same logic written by hand as an `enum`. One real run:

```text
1. the size of a future, measured
  keeps_nothing                   24 bytes  !Unpin
  buffer_before_the_await         32 bytes  !Unpin
  buffer_across_the_await       1048 bytes  !Unpin
  borrows_its_own_local           40 bytes  !Unpin
  outer (awaits the third)      1064 bytes  !Unpin

2. read_two: the compiler's future and ours
  async fn read_two               56 bytes  !Unpin
  enum ReadTwo                    32 bytes  Unpin
  async fn: Report { output: Some("HEADER | BODY"), polls: 3, wakes: 2 }
  by hand:  Report { output: Some("HEADER | BODY"), polls: 3, wakes: 2 }
  same answer, same polls, same wakes: true

3. moved four times BEFORE its first poll, then run
  Report { output: Some(5), polls: 2, wakes: 1 }
```

Read block 1 as an experiment with one variable:

- `buffer_before_the_await` uses a 1,024-byte buffer and is done with it before the `.await`: **32 bytes**.
- `buffer_across_the_await` uses the same buffer *after* the `.await`: **1,048 bytes**.

The buffer moved into the future. Nothing else changed.

- `outer` only awaits `buffer_across_the_await`: **1,064 bytes**. The inner future is stored inside the outer.
- Every one of them is `!Unpin`.

Block 2: the compiler's future and our hand-written `enum` give the same answer after the same number of
polls and wakes. Ours is `Unpin`.

Block 3: a future was moved four times before its first poll (a move, a `Box`, a `Vec` that grew, back out)
and then ran correctly.

## C · What happens in memory

### The model

An `async fn` cannot keep its local variables on the stack, because it *returns* every time it has to wait
(lesson 7: `poll` returns `Pending`). When it is polled again it must find its variables where it left them.
So they have to live in the future itself.

Here is `read_two`, and our hand-written version of what has to be remembered at each place where it can be
parked. **This is a model. It is how we would write it; it is not the compiler's output.**

```rust
pub async fn read_two(inbox: &Inbox) -> String {
    let header = inbox.read_frame().await;      // can park here  (1)
    let body = inbox.read_frame().await;        // can park here  (2)
    format!("{header} | {body}")
}

pub enum ReadTwo<'a> {
    Start { inbox: &'a Inbox },                                  // not polled yet
    WaitingForHeader { inbox: &'a Inbox, future: ReadFrame },    // parked at (1)
    WaitingForBody { header: String, future: ReadFrame },        // parked at (2)
    Done,                                                        // returned
}
```

```text
            poll                    poll, header arrived            poll, body arrived
  Start ──────────▶ WaitingForHeader ──────────────────▶ WaitingForBody ─────────────────▶ Done
  holds:            holds:                               holds:                            returns
   inbox             inbox                                header   ◀── needed after (2)     "HEADER | BODY"
                     the ReadFrame future                 the ReadFrame future
                        │        ▲                           │        ▲
                        └─Pending┘ (parked, woken later)     └─Pending┘
```

Each state holds exactly what is still needed from that point on. `header` appears in `WaitingForBody`
because the function uses it after the second `.await`. That is the rule block 1 measured with the buffer:
**what is alive across an `.await` is stored in the future.**

### Where the self-reference comes from

Now the function that explains `!Unpin`:

```rust
pub async fn borrows_its_own_local(inbox: &Inbox) -> usize {
    let mut frames_seen = 0;
    let counter = &mut frames_seen;
    let frame = inbox.read_frame().await;
    *counter += 1;
    frame.len() + *counter
}
```

Both `frames_seen` and `counter` are alive across the `.await`, so both are stored in the future. And
`counter` is a reference to `frames_seen`. Illustrative addresses, as a model of the parked future (40 bytes,
measured; the arrangement inside is our drawing, not a known layout):

```text
 the parked future, pinned at 0x6000
 ┌──────────────────────────────────────────┐
 │ which .await am I parked at?             │
 │ frames_seen = 0            ◀────────┐    │   0x6008 (say)
 │ counter     = 0x6008  ──────────────┘    │   a pointer INTO THIS FUTURE
 │ the ReadFrame future being awaited       │
 └──────────────────────────────────────────┘
```

That is the `Frame` of lesson 2, generated by the compiler: a struct with a field that holds the address of
another of its own fields. Move this future to `0x9000` and `counter` still says `0x6008`.

We could not write this struct ourselves in safe Rust (lesson 2, E0505 and E0515). The compiler can, because
it gets something we did not have: `poll` takes `Pin<&mut Self>`, so the future is guaranteed not to move
once polling has begun.

## D · The compiler's perspective, with evidence

**1. State is stored in the future. (Measured.)** 32 bytes against 1,048, for the same buffer before and
after the `.await`. The test `what_lives_across_an_await_is_stored_in_the_future` asserts the two bounds.

**2. An awaited future lives inside the one that awaits it. (Measured.)** `outer` is at least as big as the
future it awaits (1,064 ≥ 1,048). So `.await` does not allocate, and it does not need `Box`. The inner future
is a field of the outer one, and it is pinned *because the outer one is*. The only future that anyone has to
pin explicitly is the outermost one, and the executor does that (lesson 7: one `pin!` in `block_on_for`).

That answers two of the course's hard questions. *Does every async function need `Pin<Box<_>>`?* No: awaiting
is free of boxes. *Why are async functions sometimes `!Unpin`?* See the next two points.

**3. Every `async` future we measure is `!Unpin`. (Measured.)** Including `keeps_nothing`, including an empty
`async {}` (the test `every_async_future_here_is_not_unpin`). The compiler does not inspect each function to
decide whether *this particular* future holds a self-reference. `async` futures are simply not `Unpin`. The
ones that do not need the protection pay nothing for it at run time (lesson 4: `Unpin` is not in memory).

**4. A parked future really does hold a pointer into itself. (Measured, with a caveat.)**
`examples/parked_future_alias.rs` polls `borrows_its_own_local` once, takes a shared reference to the whole
parked future, and polls again. Every line is safe Rust and the program prints the right answer. Under Miri:

```text
error: Undefined Behavior: attempting a read access using <814> at alloc391[0x8], but that tag does not
       exist in the borrow stack for this location
  --> src/lesson08_async_state_machine.rs:48:5
48 |     *counter += 1;
   |     ^^^^^^^^^^^^^ this error occurs as part of an access at alloc391[0x8..0x10]
   = help: ... the Stacked Borrows rules it violated are still experimental
help: <814> was created by a Unique retag at offsets [0x8..0x10]
  --> src/lesson08_async_state_machine.rs:46:19
46 |     let counter = &mut frames_seen;
help: <814> was later invalidated at offsets [0x0..0x28] by a SharedReadOnly retag
  --> examples/parked_future_alias.rs:33:18
33 |     let whole = &*future;
```

Read it as a description of the inside of the future:

- `created ... at offsets [0x8..0x10]` by `let counter = &mut frames_seen`: there is a unique reference to
  bytes 8 to 16 **of the future's own allocation**. That is `counter`, pointing at `frames_seen`.
- `invalidated at offsets [0x0..0x28]` by `&*future`: our shared reference covered all 40 bytes of the
  future, including those 8.
- The error is raised at `*counter += 1`, when the future uses its inner reference again.

**The caveat, stated exactly.** This is a report from Stacked Borrows, an *experimental* model of Rust's
aliasing rules, about a program that contains no `unsafe` at all. Miri's other model accepts the same program:

```console
$ MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri run --example parked_future_alias
the parked future is 40 bytes
answer: 5
```

So this is not a bug in our program, and we do not claim it is undefined behaviour in Rust. What it gives us
is something we could not get any other way from the outside: a tool that tracks every reference telling us,
with byte offsets, that the parked future contains a reference into itself. How references *inside* a
self-referential future should interact with references *to* it is a known open area of Rust's aliasing
rules.

**5. Before its first poll, a future is an ordinary value. (Measured.)** Block 3 moved one four times and
then ran it. An `async fn` does not run any of its body when it is called; it only packages its arguments.
No local exists yet, so no reference to a local exists yet. The address starts to matter at the first `poll`,
which is exactly when `Pin` is required.

That answers the last hard question for this lesson. *Why does Tokio accept futures without asking you to pin
them?* Because functions like `spawn` take the future **by value**, before it has been polled, when moving it
is harmless. They move it once into its final place and pin it there themselves. Lesson 10 does this for real.

**6. What you see when this goes wrong.** `invalid/lesson08_async_future_needs_pinning.rs` (not part of the
build) tries to use an `async` future through a plain `&mut`, two everyday ways:

```text
error[E0277]: `{async fn body of work()}` cannot be unpinned
18 |     Pin::new(&mut future).poll(&mut context)
   |     -------- ^^^^^^^^^^^ within `impl Future<Output = u32>`, the trait `Unpin` is not implemented for
   |                           `{async fn body of work()}`

error[E0277]: `{async fn body of work()}` cannot be unpinned
24 |     (&mut future).await
   = note: required for `&mut impl Future<Output = u32>` to implement `Future`
   = note: consider using the `pin!` macro
```

`{async fn body of work()}` is the compiler's name for the state machine. The fix is in the note:
`let mut future = pin!(work());` and then `future.as_mut().await` or `future.as_mut().poll(…)`.

## E · The hand-written machine, as a solution you can run

```rust
impl Future for ReadTwo<'_> {
    type Output = String;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<String> {
        let this = self.get_mut();
        loop {
            match mem::replace(this, ReadTwo::Done) {
                ReadTwo::Start { inbox } => {
                    let future = inbox.read_frame();
                    *this = ReadTwo::WaitingForHeader { inbox, future };
                }
                ReadTwo::WaitingForHeader { inbox, mut future } => {
                    match Pin::new(&mut future).poll(cx) {
                        Poll::Ready(header) => { /* go to WaitingForBody */ }
                        Poll::Pending => { /* put the state back, return Pending */ }
                    }
                }
                // WaitingForBody: the same shape. Done: panic.
            }
        }
    }
}
```

Block 2 ran it against the real `async fn`: same output, three polls, two wakes.

And notice what this version is allowed to do that the compiler's is not. It calls `self.get_mut()` and
`mem::replace(this, …)`: it takes its own state **out by value** and puts a new one in. That is a move of
every field, on every poll. It is legal only because nothing in `ReadTwo` points into `ReadTwo`: all fields
are `Unpin`, so the `enum` is (lesson 4). The moment a state had to hold `counter = &mut frames_seen`, this
technique would be impossible and we would be back to lesson 2.

That is the real trade the language made. Hand-written state machines are `Unpin` and cannot borrow across
their own suspension points. `async` lets you write `let x = &mut local; something.await; *x += 1;` as if
it were ordinary code, and the price is that the future must be pinned before it is polled.

## F · The important lines

```rust
let checksum = {
    let buffer = [1u8; 1024];
    buffer.iter().map(|byte| *byte as usize).sum::<usize>()
};
inbox.read_frame().await.len() + checksum
```
The inner block ends before the `.await`, so `buffer` is gone by then and only the 8-byte `checksum` crosses.
Scoping a large value so that it dies before an `.await` is a real technique for keeping futures small.

```rust
let counter = &mut frames_seen;
let frame = inbox.read_frame().await;
*counter += 1;
```
Three lines of ordinary-looking Rust. A borrow of a local, held across an `.await`. This is what makes the
future self-referential.

```rust
match mem::replace(this, ReadTwo::Done) {
```
The hand-written machine's way of getting ownership of the current state's fields: swap in a placeholder,
take the old value apart, write the next state. If we `return Poll::Pending` we must first put a real state
back, which both `Pending` arms do.

```rust
match Pin::new(&mut future).poll(cx) {
```
Polling the inner future. `Pin::new` is enough because `ReadFrame` is `Unpin`.

```rust
ReadTwo::Done => panic!("ReadTwo polled after completion"),
```
Lesson 7's contract: a future must not be polled after it returned `Ready`. Ours says so loudly, and a test
checks it.

## G · Experiment

1. **Grow the buffer.** Change `1024` to `65536` in `buffer_across_the_await`.
   *Prediction:* that future and `outer` both grow by about 64 KB; `buffer_before_the_await` (change it there
   too) does not grow at all.
2. **Shorten a lifetime.** In `borrows_its_own_local`, move `*counter += 1;` *above* the `.await`.
   *Prediction:* the result is the same and the future is still `!Unpin` (point 3 of section D). Whether its
   size changes is something to measure, not to assume.
3. **Run the experiment yourself.** `cargo +nightly miri run --example parked_future_alias`, then again with
   `MIRIFLAGS=-Zmiri-tree-borrows`. *Measured:* an error at `*counter += 1` under the default model; the
   correct answer, 5, under Tree Borrows.

## H · Common mistakes

- **"An `async fn` runs when I call it."** It returns a future and runs nothing. Block 3 depends on that.
- **"Awaiting allocates."** It does not. The awaited future is stored inside the awaiting one.
- **"Only futures with borrows across `.await` are `!Unpin`."** All `async` futures are, here, including an
  empty one. Do not write code that assumes otherwise.
- **"My future is 50 KB, async is bloated."** Something large is alive across an `.await`. Scope it, or put
  *that value* in a `Box`.
- **"The state machine has one state per `.await` and I can rely on its layout."** The model is a way of
  thinking. Sizes and layouts are the compiler's business and change between versions.
- **Holding a `MutexGuard` (or any borrow you did not mean to keep) across an `.await`.** It becomes part of
  the future's state and stays locked while the future is parked.
- **"I must pin every future before awaiting it."** `.await` on a future you own does it for you. You pin by
  hand only when you poll by hand or await through `&mut`.

## I · Challenge

Two futures from the same function:

```rust
let inbox = Inbox::new();
let first = borrows_its_own_local(&inbox);
let second = borrows_its_own_local(&inbox);
```

Neither has been polled. Is `mem::swap(&mut first, &mut second)` allowed (make them `mut`)? Does it compile?
Is it sound? And what changes, exactly, after `let mut first = pin!(first);` and one poll?

**Expected result:** it compiles and it is sound. `mem::swap` needs `&mut T`, and you have that for a value
you own, whether or not `T` is `Unpin`. Unpolled, the two futures hold only their argument, so there is
nothing to break. After `pin!` you hold `Pin<&mut _>` and the original binding is out of reach, so there is
no `&mut` to give to `swap`: the same call no longer compiles (E0277 if you try `get_mut`). And after one
poll, `counter` exists and points into the future, which is the first moment a move would have done damage.
`!Unpin` never stopped a move by itself. Pinning did, and it arrived exactly when it was needed.

The question for lesson 9: *our `Anchored` future in lesson 7 used `unsafe { self.get_unchecked_mut() }` to
reach its own fields. Real futures wrap other futures. How do you get from `Pin<&mut Wrapper>` to
`Pin<&mut Inner>` for a field, and when is that sound?*

## Documentation

- The Rust Reference, async blocks and async functions:
  https://doc.rust-lang.org/reference/expressions/block-expr.html#async-blocks and
  https://doc.rust-lang.org/reference/items/functions.html#async-functions
- `std::pin`, "Address-sensitive values" and the note on `async` futures:
  https://doc.rust-lang.org/std/pin/#address-sensitive-values-aka-when-we-need-pinning
- `std::mem::size_of_val`: https://doc.rust-lang.org/std/mem/fn.size_of_val.html
- `std::mem::replace`: https://doc.rust-lang.org/std/mem/fn.replace.html
- Miri and its aliasing models: https://github.com/rust-lang/miri
- Stacked Borrows (experimental): https://github.com/rust-lang/unsafe-code-guidelines/blob/master/wip/stacked-borrows.md
- Tree Borrows: https://perso.crans.org/vanille/treebor/
