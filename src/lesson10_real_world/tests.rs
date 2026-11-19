use super::*;
use crate::lesson06_pin_box::{close, ping, read_frame};
use crate::lesson07_future_poll::block_on_for;
use crate::{is_unpin, is_unpin_value};
use std::pin::pin;
use std::task::Waker;

const PATIENCE: Duration = Duration::from_secs(10);

fn tokio_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn who_needs_pinning() {
    assert!(is_unpin!(TcpStream), "a socket is an ordinary value");
    assert!(is_unpin!(ReadLine<'static, TcpStream>));
    assert!(is_unpin!(JoinAll<'static, String>));
    assert!(!is_unpin!(Sleep), "Tokio's timer is address-sensitive");
    assert!(!is_unpin!(WithDeadline<ReadLine<'static, TcpStream>>));
    assert!(!is_unpin_value!(session()), "and so is every async fn");
}

#[test]
fn read_line_runs_on_our_own_executor_from_lesson_7() {
    // An in-memory pipe from Tokio, driven by OUR block_on. No Tokio
    // runtime exists in this test. The `Future` trait is the contract.
    let (mut near, mut far) = tokio::io::duplex(64);
    let wrote = block_on_for(far.write_all(b"HEADER\nBODY\n"), PATIENCE);
    assert!(wrote.output.is_some());
    let first = block_on_for(read_line(&mut near), PATIENCE);
    let second = block_on_for(read_line(&mut near), PATIENCE);
    assert_eq!(first.output.unwrap().unwrap(), "HEADER");
    assert_eq!(second.output.unwrap().unwrap(), "BODY");
}

#[test]
fn read_line_is_pending_until_the_line_is_complete() {
    let (mut near, mut far) = tokio::io::duplex(64);
    let mut reading = pin!(read_line(&mut near));
    let mut context = Context::from_waker(Waker::noop());
    assert!(reading.as_mut().poll(&mut context).is_pending());
    block_on_for(far.write_all(b"HEA"), PATIENCE);
    assert!(
        reading.as_mut().poll(&mut context).is_pending(),
        "half a line"
    );
    block_on_for(far.write_all(b"DER\n"), PATIENCE);
    match reading.as_mut().poll(&mut context) {
        Poll::Ready(Ok(line)) => assert_eq!(line, "HEADER"),
        other => panic!("expected the line, got {other:?}"),
    }
}

#[test]
fn read_line_reports_a_closed_connection() {
    let (mut near, far) = tokio::io::duplex(64);
    drop(far);
    let report = block_on_for(read_line(&mut near), PATIENCE);
    let error = report.output.unwrap().unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
}

#[test]
fn join_all_keeps_the_order_of_the_jobs() {
    // Lesson 6's three operations finish after 3, 2 and 1 polls.
    let jobs: Vec<BoxFuture<'static, String>> =
        vec![Box::pin(read_frame(7)), Box::pin(ping()), Box::pin(close())];
    let (answers, polls) = crate::crank::turn(pin!(join_all(jobs)));
    assert_eq!(answers, ["frame 7, first byte 'G'", "pong", "closed"]);
    assert_eq!(polls, 3, "as many polls as the slowest job needs");
}

#[test]
#[cfg_attr(miri, ignore = "needs Tokio's timer")]
fn a_deadline_lets_a_fast_future_through_and_stops_a_slow_one() {
    tokio_runtime().block_on(async {
        let fast = with_deadline(async { 7 }, Duration::from_secs(5)).await;
        assert_eq!(fast, Ok(7));
        let never = std::future::pending::<u32>();
        let slow = with_deadline(never, Duration::from_millis(30)).await;
        assert_eq!(slow, Err(TimedOut));
    });
}

#[test]
#[cfg_attr(miri, ignore = "needs real sockets")]
fn the_whole_session_over_loopback_tcp() {
    let log = tokio_runtime().block_on(session()).unwrap();
    assert_eq!(log.header, "HEADER");
    assert!(log.header_polls >= 1);
    assert_eq!(log.body, Ok("BODY".to_string()));
    assert_eq!(log.ticks, ["TICK 1", "TICK 2"]);
    assert_eq!(log.silence, Err("timed out".to_string()));
    assert_eq!(log.greetings, ["HEADER", "HEADER", "HEADER"]);
    assert_eq!(log.frames, ["frame 1: HEADER", "frame 2: BODY"]);
}

#[test]
#[cfg_attr(miri, ignore = "needs Tokio's timer")]
fn a_ready_future_beats_a_deadline_that_has_already_passed() {
    // The challenge of this lesson: `poll` asks the future first.
    tokio_runtime().block_on(async {
        let both_ready = with_deadline(async { 7 }, Duration::ZERO).await;
        assert_eq!(both_ready, Ok(7));
    });
}
