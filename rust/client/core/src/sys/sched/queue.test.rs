use super::Queue;

const PACED_MS: f64 = 16.0;

fn busy(busy_ms: f64, encode_ms: f64) -> Queue {
    let mut q = Queue::default();
    q.measure(busy_ms);
    q.submit(0.0, encode_ms);
    q
}

#[test]
fn an_empty_gpu_takes_a_frame_whatever_was_foreseen() {
    assert!(busy(220.0, 4.0).due(0.0, 0, PACED_MS));
}

#[test]
fn a_frame_waits_while_the_gpu_outlasts_the_next_turn() {
    assert!(!busy(54.0, 4.0).due(16.0, 1, PACED_MS));
}

#[test]
fn a_frame_begins_at_the_last_turn_that_reaches_the_gpu_in_time() {
    let q = busy(54.0, 4.0);
    assert!(!q.due(33.0, 1, PACED_MS), "the next turn still reaches it");
    assert!(q.due(34.0, 1, PACED_MS), "the next turn would be late");
}

#[test]
fn a_gpu_quicker_than_two_turns_is_never_made_to_wait() {
    assert!(busy(20.0, 4.0).due(0.0, 1, PACED_MS));
}

#[test]
fn with_no_measure_nothing_waits() {
    let mut q = Queue::default();
    q.submit(0.0, 4.0);
    assert!(q.due(0.0, 1, PACED_MS));
}

#[test]
fn a_frame_handed_over_queues_behind_the_one_running() {
    let mut q = busy(54.0, 4.0);
    q.submit(10.0, 4.0);
    assert!(
        !q.due(87.0, 1, PACED_MS),
        "108 less 21 is more than a turn off"
    );
    assert!(q.due(88.0, 1, PACED_MS));
}
