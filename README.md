# pin-and-unpin-laboratory

`Pin` and `Unpin`, from a struct moving through memory to async internals.
One type, a `Frame` read from a connection, is followed through the whole
course: first as an ordinary struct, then with a pointer to itself, then
pinned, then as a hand-written `Future`, and finally in a real asynchronous
read.

Every lesson is a module with a `run` function, a write-up in `lessons/`, and
tests. Code that is meant NOT to compile lives in `invalid/`, outside the
build, with its real compiler error quoted in the write-up.

```console
$ cargo run            # lists the lessons
$ cargo run -- 1       # runs lesson 1
$ cargo test           # every lesson's tests
$ cargo run --example address_stability
$ cargo run --example boxed_future
$ cargo run --example custom_future
$ cargo run --example parked_future_alias
```

| # | Lesson | Write-up |
|---|---|---|
| 1 | What a move really is | [lessons/01-moves.md](lessons/01-moves.md) |
| 2 | A struct that points at itself | [lessons/02-self-referential.md](lessons/02-self-referential.md) |
| 3 | `Pin<P>`: a promise about the pointee | [lessons/03-pin.md](lessons/03-pin.md) |
| 4 | `Unpin` and `!Unpin` | [lessons/04-unpin.md](lessons/04-unpin.md) |
| 5 | `PhantomPinned` | [lessons/05-phantom-pinned.md](lessons/05-phantom-pinned.md) |
| 6 | `Pin<Box<T>>` and `BoxFuture` | [lessons/06-pin-box.md](lessons/06-pin-box.md) |
| 7 | `Future::poll`, `Pending`, `Ready`, `Waker` | [lessons/07-future-poll.md](lessons/07-future-poll.md) |
| 8 | The async state machine | [lessons/08-async-state-machine.md](lessons/08-async-state-machine.md) |

More lessons are added one at a time.

Rust 1.100.0 (pinned in `rust-toolchain.toml`). Lessons with unsafe code are
also checked under Miri: `cargo +nightly miri test`.
