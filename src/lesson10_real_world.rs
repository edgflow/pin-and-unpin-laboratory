//! Lesson 10 · A real asynchronous read.
//!
//! A real socket on the loopback interface, driven by Tokio. Every piece
//! from the nine lessons before is in here once, and at the end we audit
//! which of them needed pinning and who did it.
//!
//! Run it:  cargo run -- 10
use crate::lesson03_pin::Frame;
use crate::lesson06_pin_box::BoxFuture;
use crate::lesson09_pin_projection::Timed;
use pin_project::pin_project;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWriteExt, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{Sleep, sleep};

// ------------------------------------------- 1. a hand-written future

/// "Read one line from this reader": lesson 7's `ReadFrame`, for real.
/// It holds a borrow of the reader and the bytes collected so far.
pub struct ReadLine<'a, R: ?Sized> {
    reader: &'a mut R,
    line: Vec<u8>,
}

pub fn read_line<R: AsyncRead + Unpin + ?Sized>(
    reader: &mut R,
) -> ReadLine<'_, R> {
    ReadLine {
        reader,
        line: Vec::new(),
    }
}

impl<R: AsyncRead + Unpin + ?Sized> Future for ReadLine<'_, R> {
    type Output = io::Result<String>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // A `&mut R` and a `Vec<u8>`: both `Unpin`, so we are `Unpin`,
        // and plain `&mut Self` is available without `unsafe`.
        let this = self.get_mut();
        loop {
            // One byte at a time, so that we never read past the line
            // end and swallow the start of the next line. (Real code
            // puts a buffer in the READER instead: `BufReader`.)
            let mut byte = [0u8; 1];
            let mut space = ReadBuf::new(&mut byte);
            // The reader registers `cx`'s waker with Tokio's reactor
            // when it has nothing for us. We only pass `Pending` on.
            ready!(Pin::new(&mut *this.reader).poll_read(cx, &mut space))?;
            match space.filled() {
                [] => {
                    return Poll::Ready(Err(
                        io::ErrorKind::UnexpectedEof.into()
                    ));
                }
                [b'\n'] => {
                    let line = String::from_utf8_lossy(&this.line).into_owned();
                    return Poll::Ready(Ok(line));
                }
                bytes => this.line.extend_from_slice(bytes),
            }
        }
    }
}

// --------------------------- 2. a wrapper with two pinned fields

/// The inner future did not finish in time.
#[derive(Debug, PartialEq)]
pub struct TimedOut;

/// Runs `future` until it finishes or the timer does. Both fields are
/// structurally pinned: `F` may be an `async` future, and Tokio's
/// `Sleep` is `!Unpin` as well.
#[pin_project]
pub struct WithDeadline<F> {
    #[pin]
    future: F,
    #[pin]
    timer: Sleep,
}

pub fn with_deadline<F: Future>(future: F, after: Duration) -> WithDeadline<F> {
    WithDeadline {
        future,
        timer: sleep(after),
    }
}

impl<F: Future> Future for WithDeadline<F> {
    type Output = Result<F::Output, TimedOut>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        if let Poll::Ready(output) = this.future.poll(cx) {
            return Poll::Ready(Ok(output));
        }
        ready!(this.timer.poll(cx));
        Poll::Ready(Err(TimedOut))
    }
}

// --------------------------------- 3. many boxed futures, one future

/// Runs a list of boxed futures side by side and collects their outputs
/// in the original order.
pub struct JoinAll<'a, T> {
    jobs: Vec<Option<BoxFuture<'a, T>>>,
    outputs: Vec<Option<T>>,
}

pub fn join_all<'a, T>(jobs: Vec<BoxFuture<'a, T>>) -> JoinAll<'a, T> {
    let outputs = jobs.iter().map(|_| None).collect();
    JoinAll {
        jobs: jobs.into_iter().map(Some).collect(),
        outputs,
    }
}

// Nothing in `JoinAll` is pinned in place: the futures are behind boxes
// and the outputs are moved out at the end. It is `Unpin` for any `T`.
impl<T> Unpin for JoinAll<'_, T> {}

impl<T> Future for JoinAll<'_, T> {
    type Output = Vec<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Vec<T>> {
        let this = self.get_mut();
        for (slot, output) in this.jobs.iter_mut().zip(&mut this.outputs) {
            let Some(job) = slot else { continue };
            // `as_mut()` on the box: the future inside does not move.
            if let Poll::Ready(value) = job.as_mut().poll(cx) {
                *output = Some(value);
                *slot = None; // finished: drop the box
            }
        }
        if this.jobs.iter().any(Option::is_some) {
            return Poll::Pending;
        }
        Poll::Ready(this.outputs.drain(..).flatten().collect())
    }
}

// ----------------------------------------------- a small test server

/// What the server sends on every connection, with a pause between
/// the lines. After that it stays silent until the client hangs up.
pub const SCRIPT: [&str; 4] = ["HEADER", "BODY", "TICK 1", "TICK 2"];
const GAP: Duration = Duration::from_millis(20);

async fn serve_one(mut stream: TcpStream) -> io::Result<()> {
    for line in SCRIPT {
        stream.write_all(format!("{line}\n").as_bytes()).await?;
        sleep(GAP).await;
    }
    // Silent now. Wait for the client to close the connection.
    let mut rest = [0u8; 64];
    while tokio::io::AsyncReadExt::read(&mut stream, &mut rest).await? > 0 {}
    Ok(())
}

async fn serve(listener: TcpListener) {
    while let Ok((stream, _)) = listener.accept().await {
        // `spawn` takes the future BY VALUE. No `Pin` in sight.
        tokio::spawn(serve_one(stream));
    }
}

/// Opens a new connection and returns the first line the server sends.
async fn first_line(server: SocketAddr) -> io::Result<String> {
    let mut stream = TcpStream::connect(server).await?;
    read_line(&mut stream).await
}

// ------------------------------------------------------ the session

/// Everything that happened in one session, for printing and testing.
#[derive(Debug, PartialEq)]
pub struct Session {
    pub header: String,
    pub header_polls: u32,
    pub body: Result<String, String>,
    pub ticks: Vec<String>,
    pub silence: Result<String, String>,
    pub greetings: Vec<String>,
    pub frames: Vec<String>,
}

fn flatten(
    read: Result<io::Result<String>, TimedOut>,
) -> Result<String, String> {
    match read {
        Ok(Ok(line)) => Ok(line),
        Ok(Err(error)) => Err(error.to_string()),
        Err(TimedOut) => Err("timed out".to_string()),
    }
}

pub async fn session() -> io::Result<Session> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let server = listener.local_addr()?;
    let serving = tokio::spawn(serve(listener));
    let mut stream = TcpStream::connect(server).await?;
    let unread = || Err("not read yet".to_string());
    let mut log = Session {
        header: String::new(),
        header_polls: 0,
        body: unread(),
        ticks: Vec::new(),
        silence: unread(),
        greetings: Vec::new(),
        frames: Vec::new(),
    };

    // A. Our hand-written future, inside lesson 9's poll counter.
    let (header, polls) = Timed::new(read_line(&mut stream)).await;
    (log.header, log.header_polls) = (header?, polls);

    // B. The same read, under a generous deadline.
    let within = Duration::from_secs(2);
    log.body = flatten(with_deadline(read_line(&mut stream), within).await);

    // C. One timer for the whole loop. It is used by `&mut` in every
    //    turn, so it has to be pinned by us: the one explicit pin here.
    let quiet = sleep(Duration::from_millis(300));
    tokio::pin!(quiet);
    loop {
        tokio::select! {
            line = read_line(&mut stream) => log.ticks.push(line?),
            () = &mut quiet => break,
        }
    }

    // D. The server is silent now. A short deadline fires.
    let short = Duration::from_millis(100);
    log.silence = flatten(with_deadline(read_line(&mut stream), short).await);

    // E. Three more connections at once, as boxed futures in one list.
    let jobs: Vec<BoxFuture<'_, io::Result<String>>> =
        (0..3).map(|_| Box::pin(first_line(server)) as _).collect();
    for greeting in join_all(jobs).await {
        log.greetings.push(greeting?);
    }

    // F. Keep what we read as lesson 3's address-sensitive frames, and
    //    read each payload back THROUGH its self-pointer.
    let lines = [&log.header, log.body.as_ref().map_or(&log.header, |b| b)];
    let frames: Vec<Pin<Box<Frame>>> = (1..)
        .zip(lines)
        .map(|(id, line)| Frame::pinned(id, line))
        .collect();
    for frame in &frames {
        let payload = frame.as_ref().payload().unwrap_or("?");
        log.frames.push(format!("frame {}: {payload}", frame.id));
    }

    serving.abort();
    Ok(log)
}

pub fn run() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a Tokio runtime");
    // `block_on` is where the outermost future is pinned, once.
    let log = runtime.block_on(session()).expect("the loopback session");

    println!("A. header:   {:?}", log.header);
    println!("   polled more than once: {}", log.header_polls > 1);
    println!("B. body:     {:?}", log.body);
    println!("C. ticks:    {:?}", log.ticks);
    println!("D. silence:  {:?}", log.silence);
    println!("E. three more connections: {:?}", log.greetings);
    println!("F. frames, read through their own cursors:");
    for frame in &log.frames {
        println!("   {frame}");
    }
}

#[cfg(test)]
mod tests;
