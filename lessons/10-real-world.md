# Lesson 10 · A real asynchronous read

Part 4, Real-world application. Code: `src/lesson10_real_world.rs`, tests in
`src/lesson10_real_world/tests.rs`.

```console
$ cargo run -- 10
$ cargo test lesson10
$ cargo +nightly miri test lesson10      # 5 of the 8 tests; the three with sockets and timers are skipped
```

This is the final project. It adds one dependency, `tokio` 1.54.1 with five features: `rt` (the runtime), `net`
(TCP), `time` (timers), `io-util` (`write_all`, the in-memory `duplex` pipe) and `macros` (`select!`, `pin!`).
Everything runs on the loopback interface of your own machine: the program starts its own small server on
`127.0.0.1` with a port chosen by the system, and talks only to that.

## A · The problem

Nine lessons built the theory. Now a real program: connect to a server, read lines from a socket, give up when
the server is too slow, read from several connections at once. Two questions hang over it, and they are the two
questions people actually ask about `Pin`:

1. **Does every async function need `Pin<Box<_>>`?**
2. **Why does Tokio accept my futures without me pinning anything?**

We answer both by building the program, and then auditing it line by line: what is address-sensitive, and who
pinned it.

## B · The naive implementation

The everyday version of this code needs a timer that runs across a whole loop: "stop when the connection has
been quiet for 300 ms in total". The natural first attempt creates the timer once and uses it by `&mut` in every
turn:

```rust
let mut quiet = sleep(Duration::from_millis(300));
loop {
    tokio::select! {
        () = sleep(Duration::from_millis(50)) => turns += 1,
        () = &mut quiet => break,
    }
}
```

It does not compile. The file is `invalid/lesson10_select_without_pin.rs`; it is not part of the build. Its real
error (the compiler prints it three times, for three places inside the macro; this is the first):

```text
error[E0277]: `PhantomPinned` cannot be unpinned
  --> invalid/lesson10_select_without_pin.rs:14:9
   |
14 | /         tokio::select! {
15 | |             () = sleep(Duration::from_millis(50)) => turns += 1,
16 | |             () = &mut quiet => break,
17 | |         }
   | |_________within `(PhantomData<&()>, PhantomPinned)`, the trait `Unpin` is not implemented for `PhantomPinned`
   |
   = note: consider using the `pin!` macro
           consider using `Box::pin` if you need to access the pinned value outside of the current scope
   = note: required because it appears within the type `(PhantomData<&()>, PhantomPinned)`
   = note: required for `Sleep` to implement `Unpin`
   = note: required for `&mut Sleep` to implement `Future`
   = note: required for `&mut Sleep` to implement `IntoFuture`
```

Read the notes from the bottom up, with what the course has taught:

- `select!` needs a future in every branch. We gave it `&mut Sleep`.
- `&mut F` is a future only when `F: Unpin` (lesson 4: with `Unpin`, a plain `&mut` is as good as a pinned one).
- `Sleep` is not `Unpin`, because it contains a `PhantomPinned` (lesson 5: the marker, in a real library).

Every word in that error is now something we have built ourselves.

## The working example

```console
$ cargo run -- 10
A. header:   "HEADER"
   polled more than once: true
B. body:     Ok("BODY")
C. ticks:    ["TICK 1", "TICK 2"]
D. silence:  Err("timed out")
E. three more connections: ["HEADER", "HEADER", "HEADER"]
F. frames, read through their own cursors:
   frame 1: HEADER
   frame 2: BODY
```

The server sends four lines with 20 ms between them, then stays silent. The session does six things with it:

| Step | What it does | What it uses |
|---|---|---|
| A | Reads the first line | Our hand-written future `ReadLine`, wrapped in lesson 9's `Timed` |
| B | Reads the second line under a 2 s deadline | `WithDeadline`, a wrapper with two pinned fields |
| C | Reads lines until 300 ms have passed | `select!` in a loop, with one timer pinned by us |
| D | Reads once more under a 100 ms deadline. The server is silent | The deadline fires |
| E | Opens three more connections at once | Three `BoxFuture`s (lesson 6) in one `Vec`, run by `JoinAll` |
| F | Stores the lines as lesson 3's `Frame`s | `Pin<Box<Frame>>`, read through the self-pointer |

Measured on this machine: the header read was polled 2 times in each of three runs (once before the bytes had
arrived, once after the wake). The program prints only "more than once", because the exact count depends on
timing.

## C · What happens in memory

The addresses below are **illustrative**. The sizes are measured (`size_of`), on this machine with this
toolchain. What is *inside* Tokio's types is Tokio's business; the boxes marked "Tokio" are a model.

```text
 STACK of the thread that called block_on            HEAP
 ┌──────────────────────────────────────────┐
 │ the `session()` future, 568 bytes         │       0x5000 ┌───────────────────────────┐
 │ pinned HERE by block_on, never moved      │              │ task for `serve(listener)`│
 │ ┌──────────────────────────────────────┐ │              │ its future lives in here, │
 │ │ stream: TcpStream, 32 bytes  (Unpin) │ │              │ placed by `spawn`  (Tokio)│
 │ │ log: Session                         │ │              └───────────────────────────┘
 │ │ quiet: Sleep, 96 bytes     (!Unpin)  │◀┼─── the runtime's timer keeps
 │ │   the one we pinned with tokio::pin! │ │    track of this entry (Tokio)
 │ │ whatever is being awaited right now: │ │
 │ │   ReadLine { &mut stream, Vec<u8> }──┼─┼──┐ points at `stream`, 10 lines up,
 │ │                                      │ │◀─┘ in the SAME 568 bytes
 │ │ frames: Vec<Pin<Box<Frame>>> ────────┼─┼────▶ 0x6000 ┌──────────────────────┐
 │ └──────────────────────────────────────┘ │              │ Frame 1              │
 └──────────────────────────────────────────┘              │  payload: "HEADER"   │◀─┐
                                                           │  cursor ─────────────┼──┘
                                                           └──────────────────────┘
```

Three pointers in this picture point *into* something that must therefore stay where it is:

1. `ReadLine` holds `&mut stream`, and both live inside the `session()` future. That is lesson 8's
   self-reference: a local borrowed across an `.await`. It is why the `session()` future is `!Unpin`.
2. Tokio's timer knows about `quiet`. That is why `Sleep` is `!Unpin`.
3. A `Frame`'s cursor points at its own payload. That is lesson 3.

And one thing is conspicuously *not* pinned: `stream`. A `TcpStream` is 32 bytes and `Unpin`. The operating
system's socket does not care where our handle to it lives.

## D · The compiler's perspective

**1. `Future::poll` takes `Pin<&mut Self>`, so somebody must pin every future before its first poll.** That is
the whole rule. The question is only *who*.

**2. `.await` pins for you.** When `session` writes `read_line(&mut stream).await`, the `ReadLine` value becomes
part of the `session()` state machine, and is polled in place there. If the outer future does not move, neither
does the inner one. Pinning the outermost future pins everything awaited inside it, all the way down. This is
structural pinning (lesson 9), generated by the compiler.

**3. The runtime pins the outermost future, once.** There are exactly two entry points in this program:

```rust
runtime.block_on(session())          // takes the future BY VALUE
tokio::spawn(serve(listener))        // takes the future BY VALUE
```

Neither signature mentions `Pin`. Both take ownership, and that is the trick: once the runtime owns the future,
no one else can move it. `block_on` keeps it in place while it polls it; `spawn` moves it into a task that the
runtime allocates and polls it there. (Lesson 7's `block_on_for` does the same with `pin!`, in one line.) That
is the answer to "why does Tokio accept futures without manual pinning?": **it takes them by value and pins
them itself, exactly once, at the top.**

**4. You pin by hand only when you keep a future and poll it through a reference.** In the whole session there
is one such place, step C. `quiet` is created once and polled in every turn of the loop through `&mut quiet`.
We keep the future, so we have to promise it will not move:

```rust
let quiet = sleep(Duration::from_millis(300));
tokio::pin!(quiet);                  // quiet is now Pin<&mut Sleep>
```

After this, `&mut quiet` is `&mut Pin<&mut Sleep>`, and `Pin<&mut Sleep>` is `Unpin` (lesson 4: a pinning
pointer is itself an ordinary value). The error from section B is gone.

**5. `Box::pin` is for when the type or the place must change, not for async in general.** Step E uses it,
because three futures must become *one type* to share a `Vec` (lesson 6). No other `.await` in the file has a
box. That is the answer to "does every async function need `Pin<Box<_>>`?": **no. Awaiting needs no box.
Boxing is for type erasure, recursion, or storing a future somewhere that moves.**

## E · The solution: the pinning audit

Every component of the final project, whether it is address-sensitive, and who pins it. The `Unpin` column is
measured with the lesson 4 probes (the test `who_needs_pinning`, and one extra run for the rows marked †).

| Component | `Unpin`? | Does it genuinely need pinning? | Who pins it |
|---|---|---|---|
| `TcpStream`, `TcpListener`† | yes | No. A handle to a socket; nothing points into it | nobody |
| `ReadLine<'_, TcpStream>` (ours) | yes | No. A `&mut` and a `Vec<u8>`. `poll` uses `get_mut`, no `unsafe` | nobody; `.await` still pins it, harmlessly |
| `Timed<ReadLine<..>>`† (lesson 9) | yes | No: `Unpin` because its inner future is | nobody |
| `Sleep` (Tokio) | **no** | **Yes.** Contains `PhantomPinned` (the error in B says so) | whoever awaits or holds it |
| `quiet` in step C | **no** | **Yes**, and we poll it by `&mut` | **us**, with `tokio::pin!`. The one explicit pin |
| `WithDeadline<F>` (ours) | **no** | **Yes**: it contains a `Sleep`, and `F` may be anything | `.await`; inside, `pin-project` hands the pin on to both fields |
| `JoinAll<'_, T>` (ours) | yes | No. Its futures are already behind `Pin<Box<..>>` | nobody |
| the three jobs in step E | **no** (each `async` future) | **Yes** | **us**, with `Box::pin`, because they must share a `Vec` |
| `Pin<Box<Sleep>>`, `Pin<&mut Sleep>`† | yes | The pointer does not. Its pointee does | – |
| the `session()` future | **no** | **Yes**: holds `ReadLine`'s borrow of its own `stream` across awaits | `block_on` |
| the `serve` and `serve_one` futures | **no** | **Yes** | `spawn` |
| `JoinHandle<()>`† | yes | No | nobody |
| `Frame` (lesson 3) | **no** | **Yes**: a pointer to its own field | `Frame::pinned`, with `Box::pin` |
| `Pin<Box<Frame>>`† | yes | No, which is why a `Vec` of them can grow freely | – |

Count the rows where *we* wrote a pin: **three**. One `tokio::pin!` (a future polled by reference in a loop),
one `Box::pin` (futures that must be one type), and the `Box::pin` inside `Frame::pinned` (our own
address-sensitive struct). Everything else was pinned by `.await`, by `block_on`, or by `spawn`, or never
needed it.

That is the design of `Pin` seen from the far end. It is in every `poll` signature, so that the *few* types
that are address-sensitive can be written safely. And it is almost invisible in everyday async code, because
`Unpin` switches it off for ordinary types (lesson 4) and the runtime handles the top.

## F · The important lines

```rust
let this = self.get_mut();
```
`ReadLine::poll`. `Pin::get_mut` exists only for `Unpin` targets. `ReadLine` holds a reference and a `Vec`, both
`Unpin`, so it is `Unpin` automatically, and we get `&mut Self` with no `unsafe` (lesson 4).

```rust
ready!(Pin::new(&mut *this.reader).poll_read(cx, &mut space))?;
```
Four things in one line. `poll_read` wants `Pin<&mut R>`; the reader is `Unpin` (the bound on `read_line`), so
`Pin::new` is safe. We pass *our* `cx` down: the reader hands its waker to Tokio's reactor when no bytes are
there, which is the half of the contract that lesson 7's `Inbox` did by hand. `ready!` returns `Pending` from
our `poll` if the reader said `Pending`. `?` returns an I/O error as `Ready(Err(..))`.

```rust
let mut byte = [0u8; 1];
```
One byte per call, so we never read past the end of the line and lose the start of the next one. It is simple
and correct, and slow for real traffic. Production code puts a buffer in the reader (`tokio::io::BufReader`)
and keeps what is left over.

```rust
#[pin_project]
pub struct WithDeadline<F> {
    #[pin] future: F,
    #[pin] timer: Sleep,
}
```
Both fields are structurally pinned. `future` because `F` may be an `async` future; `timer` because `Sleep` is
`!Unpin` whatever `F` is. So `WithDeadline<F>` is `!Unpin` for every `F`, even an `Unpin` one. Compare lesson
9's `Timed<F>`, which is `Unpin` whenever `F` is: there the other field was a `u32`.

```rust
let this = self.project();
if let Poll::Ready(output) = this.future.poll(cx) { .. }
ready!(this.timer.poll(cx));
```
`project` gives `Pin<&mut F>` and `Pin<&mut Sleep>`. Both are polled with the same `cx`, so *either* can wake
us: the socket when bytes arrive, the timer when time is up. The future is asked first, so a result that is
ready wins over a deadline that has also passed.

```rust
impl<T> Unpin for JoinAll<'_, T> {}
```
A safe `impl Unpin`, with a reason. Without it, `JoinAll<T>` would be `Unpin` only when `T` is, because it
stores `Option<T>` outputs. But `JoinAll` never pins an output: it moves them in and drains them out. It makes
no pinning promise about any field, so it may say so (lesson 4's rule: `impl Unpin` is safe to write, and
correct exactly when no code relies on the fields staying put).

```rust
if let Poll::Ready(value) = job.as_mut().poll(cx) {
```
`job` is `&mut Pin<Box<dyn Future>>`. `as_mut` re-borrows it as a `Pin<&mut dyn Future>` for this one poll
(lesson 3), so the box can be polled again next time.

```rust
tokio::select! {
    line = read_line(&mut stream) => log.ticks.push(line?),
    () = &mut quiet => break,
}
```
The first branch creates a new `ReadLine` in every turn; `select!` pins it on the spot for us. If the timer
wins, that `ReadLine` is dropped half-way. Here that loses nothing, because the server is silent by then. In
general, dropping a half-finished read can lose bytes it had already collected: this is called cancellation
safety, and it is a real concern with `select!` that is separate from pinning.

```rust
let jobs: Vec<BoxFuture<'_, io::Result<String>>> =
    (0..3).map(|_| Box::pin(first_line(server)) as _).collect();
```
`as _` asks the compiler to turn each `Pin<Box<{async future}>>` into the `Pin<Box<dyn Future ..>>` that the
`Vec` is declared to hold.

## G · Experiment

All measured with a temporary example on this machine.

1. **Delete `tokio::pin!(quiet);`** (and make `quiet` `mut`).
   *Prediction:* the error of section B. *Measured:* E0277, "`PhantomPinned` cannot be unpinned", "required for
   `&mut Sleep` to implement `Future`".
2. **Use the standard library instead:** `let mut quiet = std::pin::pin!(sleep(..));`
   *Prediction:* the same behaviour; `tokio::pin!` is not special. *Measured:* compiles, and a loop whose other
   branch fires every 50 ms ends after 5 turns and 300 ms. `Box::pin(sleep(..))` gives the same 5 turns and
   300 ms, at the cost of one allocation.
3. **Avoid the pin by creating the timer inside the loop:** `() = sleep(Duration::from_millis(300)) => break`.
   *Prediction:* it compiles, because `select!` pins a fresh future itself. But is it the same program?
   *Measured:* no. With another branch winning every 50 ms, the loop ran all 20 turns we allowed it, about one
   second, and the 300 ms timer never fired: every turn drops it and starts a new one. This is why the pinned
   version exists. Pinning is what lets one future **survive across** turns.
4. **Run our future on our own executor.** The test `read_line_runs_on_our_own_executor_from_lesson_7` drives
   `ReadLine` over Tokio's in-memory `duplex` pipe with lesson 7's `block_on_for`. No Tokio runtime exists in
   that test.
   *Prediction:* it works, because `Future`, `Pin` and `Waker` are the standard library's, and are the entire
   contract between a future and whoever polls it. *Measured:* passes, under Miri too. (A `TcpStream` or a
   `Sleep` would not work there: those need Tokio's reactor and timer to be running. The pipe does not.)

## H · Common mistakes

- **"Async needs `Box::pin` everywhere."** One `Box::pin` for futures in this file, for a `Vec` of mixed
  futures. An unnecessary box is an allocation and a virtual call per poll.
- **"`tokio::spawn(Box::pin(fut))`."** `spawn` takes the future by value and places it itself. The box adds a
  second allocation and nothing else.
- **"I get an `Unpin` error, so I add `+ Unpin` to my function."** That pushes the problem to every caller, who
  can no longer pass an `async` block. Usually the right fix is inside: `pin!` the value you hold.
- **"`&mut future` in `select!` should just work."** Only for `Unpin` futures. Pin it first, then `&mut` the
  pin.
- **"A new `sleep` in every turn is the same as one pinned `sleep`."** Experiment 3: it is a different program.
- **"A socket must be pinned, it is I/O."** `TcpStream` is `Unpin`. Being asynchronous and being
  address-sensitive are unrelated properties.
- **"My hand-written future must be `!Unpin` to be a proper future."** `ReadLine` and `JoinAll` are `Unpin`,
  need no `unsafe`, and are easier to use for it. Add `PhantomPinned` only when something really points into
  the value.
- **"A `Pin` that compiles means the code is right."** `Pin` says nothing about dropping a future half-way
  (`select!`), or about a future that forgets its waker (lesson 7).

## I · Challenge

`with_deadline` is given a future that is ready immediately **and** a deadline of zero: both are "done" at the
first poll.

```rust
let both_ready = with_deadline(async { 7 }, Duration::ZERO).await;
```

Before you run it: `Ok(7)` or `Err(TimedOut)`? Then: change `WithDeadline::poll` so that the deadline wins, and
say which of the two behaviours you would ship.

**Expected result:** `Ok(7)`. `poll` asks `future` first and returns as soon as it is `Ready`; the timer is
never polled at all. The test `a_ready_future_beats_a_deadline_that_has_already_passed` checks it. Swapping the
two polls makes the deadline win. Asking the future first is the usual choice: a result that already exists should not be thrown away because a clock was also done.

Notice what the change did *not* require: any thought about pinning. `project()` gave two pinned fields, and
the order of two `poll` calls is ordinary logic. That is what a correct projection buys.

## The twelve questions, once more

| Question | Lesson |
|---|---|
| Why is moving a normal struct safe? | 1 |
| Why can self-referential structures be address-sensitive? | 2 |
| Does `Pin` prevent all movement? Can `Pin<Box<T>>` itself move? | 3 |
| Why can `Unpin` types move even when pinned? | 4 |
| Why does `PhantomPinned` prevent automatic `Unpin`? | 5 |
| `Pin<&mut T>` versus `Pin<Box<T>>` | 3, 6 |
| Why does `Future::poll` take `Pin<&mut Self>`? | 7 |
| Why are async functions sometimes `!Unpin`? | 8 |
| Does every async function need `Pin<Box<_>>`? Why does Tokio take futures unpinned? | 8, and section D here |
| What exactly does the pinning contract guarantee? | 3, restated in 9, and visible in the audit in section E here |

## Documentation

- `std::pin`: https://doc.rust-lang.org/std/pin/
- `Future`, with its implementation for `&mut F where F: Future + Unpin`:
  https://doc.rust-lang.org/std/future/trait.Future.html
- `std::pin::pin!`: https://doc.rust-lang.org/std/pin/macro.pin.html
- `tokio::pin!`: https://docs.rs/tokio/1.54.1/tokio/macro.pin.html
- `tokio::select!`, including its section on cancellation safety:
  https://docs.rs/tokio/1.54.1/tokio/macro.select.html
- `tokio::time::Sleep`: https://docs.rs/tokio/1.54.1/tokio/time/struct.Sleep.html
- `tokio::time::timeout`: https://docs.rs/tokio/1.54.1/tokio/time/fn.timeout.html
- `tokio::spawn`: https://docs.rs/tokio/1.54.1/tokio/task/fn.spawn.html
- `tokio::io::AsyncRead`: https://docs.rs/tokio/1.54.1/tokio/io/trait.AsyncRead.html
- `pin-project`: https://docs.rs/pin-project/
- Asynchronous Programming in Rust, *Pinning*: https://rust-lang.github.io/async-book/
