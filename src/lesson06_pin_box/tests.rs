use super::*;

fn must_be_send<T: Send>(_: &T) {}
fn must_be_unpin<T: Unpin>(_: &T) {}

#[test]
fn async_fn_futures_are_not_unpin() {
    let (read, ping_once, close_now) = (read_frame(1), ping(), close());
    assert!(!is_unpin_value!(read));
    assert!(!is_unpin_value!(ping_once));
    assert!(!is_unpin_value!(close_now));
}

#[test]
fn a_box_future_is_two_pointers_whatever_is_inside() {
    let two_words = 2 * mem::size_of::<usize>();
    assert_eq!(mem::size_of::<BoxFuture<String>>(), two_words);
    assert_eq!(mem::size_of::<BoxFuture<[u8; 4096]>>(), two_words);
}

#[test]
fn a_box_future_is_unpin_and_send() {
    let job: BoxFuture<'static, String> = Box::pin(read_frame(1));
    must_be_unpin(&job);
    must_be_send(&job);
}

#[test]
fn different_futures_fit_in_one_list_and_all_run() {
    let mut jobs: Vec<BoxFuture<'static, String>> =
        vec![Box::pin(read_frame(7)), Box::pin(ping()), Box::pin(close())];
    let answers: Vec<(String, u32)> =
        jobs.iter_mut().map(|job| turn(job.as_mut())).collect();
    assert_eq!(answers[0], ("frame 7, first byte 'G'".to_string(), 3));
    assert_eq!(answers[1], ("pong".to_string(), 2));
    assert_eq!(answers[2], ("closed".to_string(), 1));
}

#[test]
fn a_half_run_future_survives_its_box_being_moved() {
    let mut job: BoxFuture<'static, String> = Box::pin(read_frame(7));
    // Note the address BEFORE the first poll. A future that is parked in
    // the middle of its work is not something to take a `&` to casually:
    // lesson 8 shows what it holds at that moment.
    let at = inside(&job);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(job.as_mut().poll(&mut context).is_pending());
    // Parked. Now move the box: into a Vec, grow the Vec, take it out.
    let mut shelf = vec![job];
    shelf.reserve(4096);
    let mut job = shelf.pop().unwrap();
    let (answer, more_polls) = turn(job.as_mut());
    assert_eq!(answer, "frame 7, first byte 'G'");
    assert_eq!(more_polls, 2);
    assert_eq!(inside(&job), at, "the future itself never moved");
}

#[test]
fn a_plain_box_becomes_a_pinned_box_with_into_pin() {
    // The challenge at the end of the lesson.
    let plain: Box<dyn Future<Output = String> + Send> = Box::new(ping());
    let at = std::ptr::from_ref(&*plain).cast::<()>().addr();
    let mut pinned: BoxFuture<'static, String> = Box::into_pin(plain);
    assert_eq!(inside(&pinned), at, "same allocation, new type");
    assert_eq!(turn(pinned.as_mut()), ("pong".to_string(), 2));
}

#[test]
fn a_box_future_can_borrow() {
    let line = String::from("GET /health");
    let mut later = length_later(&line);
    assert_eq!(turn(later.as_mut()), (11, 2));
}

#[test]
fn recursion_through_boxes() {
    assert_eq!(turn(countdown(0).as_mut()), (0, 1));
    assert_eq!(turn(countdown(3).as_mut()), (3, 4));
}

#[test]
fn operations_work_as_trait_objects() {
    let operations: Vec<Box<dyn Operation>> =
        vec![Box::new(Read(9)), Box::new(Ping), Box::new(Close)];
    let names: Vec<_> = operations.iter().map(|o| o.name()).collect();
    assert_eq!(names, ["read", "ping", "close"]);
    let (answer, _) = turn(operations[0].start().as_mut());
    assert_eq!(answer, "frame 9, first byte 'G'");
}
