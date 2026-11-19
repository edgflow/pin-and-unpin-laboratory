//! Lesson 7 · `Future::poll`, `Pending`, `Ready` and the `Waker`.
//!
//! We write the future behind "read one frame from the connection" by
//! hand, and the smallest honest executor to run it: one that sleeps
//! until it is told there is something to do.
//!
//! Run it:  cargo run -- 7
use std::future::Future;
use std::marker::PhantomPinned;
use std::pin::{Pin, pin};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------- inbox

/// What the reading side and the network side share.
#[derive(Default)]
struct Shared {
    /// The frame, once it has arrived.
    frame: Option<String>,
    /// How to tell the waiting task that it has.
    waker: Option<Waker>,
    /// How many times a future has asked.
    asked: u32,
}

/// One end of a connection: a place where a frame will turn up.
#[derive(Clone, Default)]
pub struct Inbox {
    shared: Arc<Mutex<Shared>>,
}

impl Inbox {
    pub fn new() -> Inbox {
        Inbox::default()
    }

    /// The network side: put the frame in, then wake whoever is waiting.
    pub fn deliver(&self, frame: &str) {
        let waker = {
            let mut shared = self.shared.lock().unwrap();
            shared.frame = Some(frame.to_string());
            shared.waker.take()
        };
        // Outside the lock: the woken task may want the lock at once.
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    /// Has a future asked at least once?
    pub fn was_asked(&self) -> bool {
        self.shared.lock().unwrap().asked > 0
    }

    /// Is a frame lying here that nobody has collected?
    pub fn has_frame(&self) -> bool {
        self.shared.lock().unwrap().frame.is_some()
    }

    pub fn read_frame(&self) -> ReadFrame {
        ReadFrame {
            inbox: self.clone(),
        }
    }

    pub fn read_frame_forgetfully(&self) -> ForgetfulReadFrame {
        ForgetfulReadFrame {
            inbox: self.clone(),
        }
    }

    /// A network that answers a moment AFTER we started waiting.
    pub fn deliver_later(&self, frame: &'static str) -> thread::JoinHandle<()> {
        let inbox = self.clone();
        thread::spawn(move || {
            while !inbox.was_asked() {
                thread::yield_now();
            }
            inbox.deliver(frame);
        })
    }
}

// ---------------------------------------------------------- the future

/// "Give me the next frame": a future written by hand.
pub struct ReadFrame {
    inbox: Inbox,
}

impl Future for ReadFrame {
    type Output = String;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<String> {
        let mut shared = self.inbox.shared.lock().unwrap();
        shared.asked += 1;
        // Is it there? Then we are done, and we hand the value over.
        if let Some(frame) = shared.frame.take() {
            return Poll::Ready(frame);
        }
        // It is not. Before saying "not yet", leave our phone number:
        // the waker of the task that is polling us right now.
        match &mut shared.waker {
            Some(known) if known.will_wake(cx.waker()) => {}
            slot => *slot = Some(cx.waker().clone()),
        }
        Poll::Pending
    }
}

/// The same future with one line missing. It says "not yet" and leaves
/// no way to be told. Nothing will ever poll it again.
pub struct ForgetfulReadFrame {
    inbox: Inbox,
}

impl Future for ForgetfulReadFrame {
    type Output = String;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<String> {
        let mut shared = self.inbox.shared.lock().unwrap();
        shared.asked += 1;
        match shared.frame.take() {
            Some(frame) => Poll::Ready(frame),
            None => Poll::Pending,
        }
    }
}

/// A future that cares where it is. It notes its address on the first
/// poll and checks it on every later one. `PhantomPinned` makes it
/// `!Unpin`, so the only way to poll it is to pin it for good.
pub struct Anchored {
    polls_left: u32,
    first_seen_at: Option<usize>,
    _address_sensitive: PhantomPinned,
}

impl Anchored {
    pub fn new(polls: u32) -> Anchored {
        Anchored {
            polls_left: polls,
            first_seen_at: None,
            _address_sensitive: PhantomPinned,
        }
    }
}

impl Future for Anchored {
    /// True when every poll found the future at the same address.
    type Output = bool;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<bool> {
        // SAFETY: we only write two plain fields in place. Nothing is
        // moved out of the pinned future.
        let this = unsafe { self.get_unchecked_mut() };
        let here = std::ptr::from_ref(&*this).addr();
        let first = *this.first_seen_at.get_or_insert(here);
        if first != here {
            return Poll::Ready(false);
        }
        if this.polls_left == 0 {
            return Poll::Ready(true);
        }
        this.polls_left -= 1;
        // Not done, and nobody else will wake us: ask to be polled again.
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

// ------------------------------------------------------- the executor

/// What a waker does in our executor: set a flag, wake the thread.
struct ThreadWaker {
    thread: Thread,
    notified: AtomicBool,
    wakes: AtomicU32,
}

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.wakes.fetch_add(1, Ordering::Relaxed);
        self.notified.store(true, Ordering::Release);
        self.thread.unpark();
    }
}

/// What happened while a future was run.
#[derive(Debug, Clone, PartialEq)]
pub struct Report<T> {
    /// The future's output, or `None` if we gave up waiting.
    pub output: Option<T>,
    pub polls: u32,
    pub wakes: u32,
}

/// Runs one future on this thread. It polls, and when the future says
/// `Pending` it goes to sleep until the waker is called, or until
/// `patience` runs out.
pub fn block_on_for<F: Future>(
    future: F,
    patience: Duration,
) -> Report<F::Output> {
    // The future is pinned HERE, once, for its whole life.
    let mut future = pin!(future);
    let state = Arc::new(ThreadWaker {
        thread: thread::current(),
        notified: AtomicBool::new(false),
        wakes: AtomicU32::new(0),
    });
    let waker = Waker::from(Arc::clone(&state));
    let mut context = Context::from_waker(&waker);
    let deadline = Instant::now() + patience;
    let mut polls = 0;
    let report = |output, polls| Report {
        output,
        polls,
        wakes: state.wakes.load(Ordering::Relaxed),
    };
    loop {
        polls += 1;
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return report(Some(output), polls);
        }
        // Sleep until notified. A thread can wake for no reason, so the
        // flag, not the waking, is what counts.
        while !state.notified.swap(false, Ordering::Acquire) {
            let now = Instant::now();
            if now >= deadline {
                return report(None, polls);
            }
            thread::park_timeout(deadline - now);
        }
    }
}

/// Runs a future to completion, however long it takes.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let forever = Duration::from_secs(60 * 60 * 24 * 365);
    block_on_for(future, forever)
        .output
        .expect("a year is enough")
}

// -------------------------------------------------------------- run

pub fn run() {
    // 1. Ready at once: the frame is already there.
    let inbox = Inbox::new();
    inbox.deliver("already here");
    let report = block_on_for(inbox.read_frame(), Duration::from_secs(5));
    println!("1. the frame was already there");
    println!("  {report:?}");

    // 2. Pending, then woken, then Ready.
    let inbox = Inbox::new();
    let network = inbox.deliver_later("GET /health");
    let report = block_on_for(inbox.read_frame(), Duration::from_secs(5));
    network.join().unwrap();
    println!("\n2. the frame arrived while we were asleep");
    println!("  {report:?}");

    // 3. The same, with a future that forgets to leave its waker.
    let inbox = Inbox::new();
    let network = inbox.deliver_later("GET /health");
    let patience = Duration::from_millis(300);
    let report = block_on_for(inbox.read_frame_forgetfully(), patience);
    network.join().unwrap();
    println!("\n3. the forgetful future, given 300 ms");
    println!("  {report:?}");
    println!("  the frame is lying in the inbox: {}", inbox.has_frame());

    // 4. A future that checks its own address on every poll.
    let report = block_on_for(Anchored::new(4), Duration::from_secs(5));
    println!("\n4. an address-sensitive future, polled five times");
    println!("  {report:?}");

    // 5. An async fn that awaits our hand-written future.
    let inbox = Inbox::new();
    let network = inbox.deliver_later("PING");
    let answer = block_on(async {
        let frame = inbox.read_frame().await;
        format!("got {frame:?}, {} bytes", frame.len())
    });
    network.join().unwrap();
    println!("\n5. awaited from an async block: {answer}");
}

#[cfg(test)]
mod tests;
