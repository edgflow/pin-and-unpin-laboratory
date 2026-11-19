# Lesson 7 · `Future::poll`, `Pending`, `Ready` and the `Waker`

Part 3, Build a Future. Code: `src/lesson07_future_poll.rs`, tests in `src/lesson07_future_poll/tests.rs`, and
`examples/custom_future.rs`.

```console
$ cargo run -- 7
$ cargo run --example custom_future
$ cargo test lesson07
$ cargo +nightly miri test lesson07      # threads, a waker, and one unsafe line: Miri checks them
```

Lesson 6 used futures as black boxes and a crude helper that asked "are you done?" in a loop. This lesson
opens the box. We write a future by hand, and the smallest honest executor to run it.

## A · The problem

"Read one frame from the connection." The frame is not there yet. It will arrive later, from another thread
(the network). We need a value that stands for *the frame that will exist*, and a way to run it that does not
burn a CPU core asking over and over.

The whole of async Rust rests on one trait with one method:

```rust
pub trait Future {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}
```

Three things in that signature need explaining: `Poll`, `Context`, and the receiver `Pin<&mut Self>`.

## B · The working example first

```rust
impl Future for ReadFrame {
    type Output = String;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<String> {
        let mut shared = self.inbox.shared.lock().unwrap();
        shared.asked += 1;
        if let Some(frame) = shared.frame.take() {
            return Poll::Ready(frame);
        }
        match &mut shared.waker {
            Some(known) if known.will_wake(cx.waker()) => {}
            slot => *slot = Some(cx.waker().clone()),
        }
        Poll::Pending
    }
}
```

One real run:

```text
1. the frame was already there
  Report { output: Some("already here"), polls: 1, wakes: 0 }

2. the frame arrived while we were asleep
  Report { output: Some("GET /health"), polls: 2, wakes: 1 }

3. the forgetful future, given 300 ms
  Report { output: None, polls: 1, wakes: 0 }
  the frame is lying in the inbox: true

4. an address-sensitive future, polled five times
  Report { output: Some(true), polls: 5, wakes: 4 }

5. awaited from an async block: got "PING", 4 bytes
```

- **Block 1:** the frame was already there. One poll, `Ready`, no wake.
- **Block 2:** the frame arrived later. **Two polls and one wake.** Not two thousand polls: the executor slept
  in between.
- **Block 3:** the same future with the waker line removed. One poll, `Pending`, and then nothing, for the
  whole 300 ms. The frame *did* arrive (last line). Nobody was told.
- **Block 4:** a future that records its own address on the first poll and checks it on every later one:
  five polls, the same address every time.
- **Block 5:** an `async` block awaits our hand-written future like any other.

## C · What happens in memory, and in time

A `Poll<T>` is a two-way answer: `Ready(value)` or `Pending`. There is no third answer and no "wait".
`poll` must return quickly either way.

Block 2 as a timeline, with two threads:

```text
 EXECUTOR THREAD                                   NETWORK THREAD
 ───────────────                                   ──────────────
 poll #1 ──▶ ReadFrame
             lock the inbox: no frame
             store the waker  ───────────┐
             return Pending              │
 ◀───────────────────────────────────────┘
 sleep (thread::park)                              the frame arrives
      .                                            lock the inbox: put the frame in,
      .                                                            take the waker out
      .                                            waker.wake() ──┐
      .                                                           │
 wake up  ◀───────────────────────────────────────────────────────┘
 poll #2 ──▶ ReadFrame
             lock the inbox: frame is there, take it
             return Ready("GET /health")
```

And what is where, between poll #1 and poll #2 (illustrative addresses):

```text
 EXECUTOR'S STACK                         HEAP (shared by both threads)

 0x7000 ┌─────────────────────┐           0x6000 ┌──────────────────────────────┐
        │ ReadFrame           │                  │ Mutex<Shared>                │
        │   inbox: Arc ───────┼────────────────▶ │   frame: None                │
        └─────────────────────┘        ┌───────▶ │   waker: Some(──┐)           │
          pinned here by pin!          │         │   asked: 1      │            │
                                       │         └─────────────────┼────────────┘
 NETWORK'S STACK                       │                           ▼
 0x8000 ┌─────────────────────┐        │         0x6100 ┌──────────────────────────┐
        │ inbox: Arc ─────────┼────────┘                │ ThreadWaker              │
        └─────────────────────┘                         │   thread: the executor   │
                                                        │   notified: false        │
                                                        └──────────────────────────┘
```

The waker is the piece that crosses the gap. The future does not know what an executor is. It only holds a
`Waker`, and a `Waker` is "call me when it is worth asking again". The network thread does not know what a
future is. It only calls `wake()`.

## D · The compiler's perspective

**The contract of `poll`.** Three duties, in this order. The comments in `examples/custom_future.rs` are
numbered the same way.

1. **Check.** Is the result available now? Then return `Ready(value)`. After that, the future must not be
   polled again (what happens if it is, is up to the future: it may panic).
2. **Register.** If not, make sure the waker from *this* call's `Context` will be woken when it is worth
   asking again. Always the latest one: a future can be polled by a different task next time.
3. **Report.** Return `Pending`.

Returning `Pending` without having arranged a wake is the bug of block 3. It is not a crash and not a compile
error. The task simply never runs again.

**Why checking and registering happen under one lock.** Suppose `poll` looked (no frame), released the lock,
and only then stored the waker. The frame could arrive in between: the network would find no waker to call,
and the future would then go to sleep on a frame that is already there. A *lost wakeup*. Doing both under the
same lock closes that window: `deliver` either runs before the check (and we see the frame) or after the
waker is stored (and we are woken).

**Why `poll` takes `Pin<&mut Self>`.** This is the question the last four lessons were building up to.

- `poll` is not called once. It is called again and again on the *same* future, and between calls the future
  keeps its state.
- Lesson 8 will show that the futures `async` generates can hold pointers into their own state. They are
  address-sensitive in exactly the sense of lesson 2.
- So the trait needs the *caller* to make a promise: "between this poll and the next, and until I drop you, I
  will not move you." A `&mut Self` receiver cannot carry that promise. Anyone holding `&mut` can `mem::swap`
  (lesson 2). A `Pin<&mut Self>` receiver can: the only way to have one for a `!Unpin` future is to have
  pinned it for good.
- A trait method has one receiver type for every implementor. So *every* future gets `Pin<&mut Self>`,
  including those with nothing to protect.

For a future like our `ReadFrame`, which holds only an `Arc`, the pin costs nothing and restricts nothing: it
is `Unpin` (lesson 4), so the test `our_read_frame_is_unpin_and_may_move_between_polls` polls it, moves it into
a `Box`, and polls it again, legally.

For a future like `Anchored`, which is `!Unpin`, the same signature is a guarantee. Block 4 measures it: five
polls, one address.

**Who pins?** Not the future, and usually not you. The executor does, once:

```rust
let mut future = pin!(future);
```

That one line in `block_on_for` is the pin for the future's whole life. Every later `future.as_mut()` only
reborrows it.

**`Context` and `Waker`.** A `Context` is, today, a wrapper around a `&Waker`. A `Waker` is a handle that is
`Send + Sync + Clone`: it can be stored, copied, and called from any thread. What calling it *does* is up to
the executor that made it. Ours is three lines:

```rust
impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.wakes.fetch_add(1, Ordering::Relaxed);
        self.notified.store(true, Ordering::Release);
        self.thread.unpark();
    }
}
```

`std::task::Wake` lets us build a `Waker` from an `Arc` in safe code. (Underneath, a waker is a data pointer
plus a table of four functions, `RawWaker`. Building one by hand is `unsafe` and rarely needed.)

## E · The solution: a future and an executor, complete

The future is in section B. The executor, without the bookkeeping:

```rust
let mut future = pin!(future);
let waker = Waker::from(Arc::clone(&state));
let mut context = Context::from_waker(&waker);
loop {
    if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
        return output;
    }
    while !state.notified.swap(false, Ordering::Acquire) {
        thread::park();
    }
}
```

Pin once. Poll. If `Pending`, sleep until the flag says we were woken. Repeat. Tokio and every other runtime
are this loop with many tasks, a queue of the woken ones, and an I/O reactor that plays the part of our
network thread.

## F · The important lines

```rust
if let Some(frame) = shared.frame.take() {
    return Poll::Ready(frame);
}
```
Duty 1. `take` moves the frame out and leaves `None`, so the value is handed over exactly once.

```rust
match &mut shared.waker {
    Some(known) if known.will_wake(cx.waker()) => {}
    slot => *slot = Some(cx.waker().clone()),
}
```
Duty 2. If we already hold a waker for the same task, keep it and skip a clone. Otherwise store the new one,
replacing any old one. The test `a_newer_waker_replaces_the_stored_one` polls from two "tasks" and checks that
only the second is woken.

```rust
let waker = { /* lock, set the frame, take the waker */ };
if let Some(waker) = waker { waker.wake(); }
```
In `deliver`. The waker is called *after* the lock is released, because the woken task will want that lock
at once.

```rust
let this = unsafe { self.get_unchecked_mut() };
```
In `Anchored::poll`, the lesson's one `unsafe` line. The future is `!Unpin`, so there is no safe `&mut`.
We take it to write two plain fields in place and move nothing out. (Lesson 9 replaces this kind of line with
pin projection.)

```rust
cx.waker().wake_by_ref();
Poll::Pending
```
Also `Anchored`. A future may wake *itself*: "I am not done, and nothing outside will tell you. Poll me
again." It is the correct way to yield.

```rust
while !state.notified.swap(false, Ordering::Acquire) {
    thread::park_timeout(deadline - now);
}
```
A parked thread may wake without anyone having called `unpark`. So the executor trusts the flag, never the
waking.

## G · Experiment

1. **The busy loop.** Run block 2's future with lesson 6's `crank::turn` instead of `block_on_for`.
   *Prediction:* it still finishes, and the poll count is no longer 2 but some large number that changes on
   every run: the crank never sleeps. That is the difference between an executor and a loop.
2. **Lose a wakeup on purpose.** In `ReadFrame::poll`, drop the lock after the check and take it again before
   storing the waker. *Prediction:* nearly every run still passes, and that is what makes this bug dangerous.
   The window is a few nanoseconds wide. It will be found in production.
3. **The timer.** `cargo run --example custom_future`. *Measured:* waited about 150 ms, polls: 2, wakes: 1.
4. **Repeat the tests.** They involve two threads. *Measured:* 6 runs out of 6 pass, and Miri passes them too,
   because the network thread waits until the future has asked once before delivering.

## H · Common mistakes

- **Returning `Pending` without registering the waker.** Block 3. The task sleeps for ever.
- **Storing the waker only on the first poll.** The next poll may come from a different task. Refresh it
  every time, with `will_wake` to skip the unnecessary clone.
- **Blocking inside `poll`.** `thread::sleep` or a blocking read in `poll` stalls the executor's thread and
  every other task on it. `poll` answers now, or says `Pending`.
- **Calling `wake` while holding the lock the woken task needs.**
- **Polling again after `Ready`.** Not allowed by the contract. Our `ReadFrame` would return `Pending` with a
  fresh waker; other futures panic.
- **"I implemented `poll` with `Pin<&mut Self>`, so my future is pinned."** The signature is a requirement on
  the caller. If your future is `Unpin`, the pin protects nothing, and that is fine.
- **Reaching for `get_unchecked_mut` in every `poll`.** For an `Unpin` future, `self.get_mut()` or plain
  field access works and is safe. See `Delay` in the example: `mut self: Pin<&mut Self>`, then `self.until`.

## I · Challenge

One future, polled by two different tasks in turn (each with its own waker), then the frame is delivered:

```rust
let mut future = pin!(inbox.read_frame());
// poll with waker A  → Pending
// poll with waker B  → Pending
inbox.deliver("frame");
```

Before you run it: which wakers are called? What would happen with a future that kept only the *first* waker?

**Expected result:** only B. The future replaced A with B on the second poll, because `will_wake` said they
were different tasks. A future that kept the first waker would wake A, which no longer cares, and the task
that is actually waiting would sleep for ever: the bug of block 3, in disguise. The test
`a_newer_waker_replaces_the_stored_one` checks both flags.

The question for lesson 8: *we wrote `ReadFrame` by hand, and it is `Unpin`. The futures in lesson 6 came
from `async fn`, and every one of them was `!Unpin`. What does the compiler put in them that we did not?*

## Documentation

- `Future` and `Future::poll`: https://doc.rust-lang.org/std/future/trait.Future.html#tymethod.poll
- `Poll`: https://doc.rust-lang.org/std/task/enum.Poll.html
- `Context`: https://doc.rust-lang.org/std/task/struct.Context.html
- `Waker` (`wake`, `wake_by_ref`, `will_wake`, `noop`): https://doc.rust-lang.org/std/task/struct.Waker.html
- `std::task::Wake`: https://doc.rust-lang.org/std/task/trait.Wake.html
- `RawWaker`: https://doc.rust-lang.org/std/task/struct.RawWaker.html
- `thread::park` and spurious wakeups: https://doc.rust-lang.org/std/thread/fn.park.html
- `std::pin::pin!`: https://doc.rust-lang.org/std/pin/macro.pin.html
