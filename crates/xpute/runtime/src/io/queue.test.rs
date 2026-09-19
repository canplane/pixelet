// xpute-runtime/io/queue.test.rs
//
// The outside runs nothing here: a demand starting is its key in `started`
// after the pump, and finishing is `settle`.

use super::*;

const CREDITS: u32 = 6;

fn d(key: u64, category: u32, priority: f64) -> Demand {
    Demand { key, category, priority }
}

/// The turn's end: the pump, and the keys it started.
fn turn<const N: usize>(q: &mut IoQueue<N>) -> Vec<u64> {
    q.pump(CREDITS);
    let keys = q.started().iter().map(|d| d.key).collect();
    q.clear_started();
    keys
}

#[test]
fn io_runs_at_most_the_credits_granted_at_once_and_admits_the_rest_as_slots_free() {
    let mut q = IoQueue::<32>::new();
    let h: Vec<Demand> = (0..9).map(|i| d(i, 0, i as f64)).collect();
    q.sync(0, &h);
    assert_eq!(turn(&mut q), [0, 1, 2, 3, 4, 5]);
    assert_eq!(q.pending_count(0), 9);
    q.settle(0);
    assert_eq!(turn(&mut q), [6]);
    for x in &h {
        q.settle(x.key);
    }
    assert_eq!(turn(&mut q), [7, 8]);
    q.settle(7);
    q.settle(8);
    assert_eq!(q.pending_count(0), 0);
}

#[test]
fn io_the_lowest_priority_number_runs_first() {
    let mut q = IoQueue::<32>::new();
    let fill: Vec<Demand> = (100..106).map(|k| d(k, 1, -1.0)).collect();
    q.sync(1, &fill);
    assert_eq!(turn(&mut q).len(), 6);
    q.sync(0, &[d(1, 0, 10.0), d(2, 0, 1.0), d(3, 0, 5.0)]);
    assert!(turn(&mut q).is_empty());
    q.settle(100);
    assert_eq!(turn(&mut q), [2]);
    q.settle(101);
    assert_eq!(turn(&mut q), [3]);
    q.settle(102);
    assert_eq!(turn(&mut q), [1]);
}

#[test]
fn io_a_later_category_submitting_in_the_same_turn_still_wins_on_priority() {
    let mut q = IoQueue::<32>::new();
    let low: Vec<Demand> = (10..16).map(|k| d(k, 1, 5.0)).collect();
    q.sync(1, &low);
    q.sync(2, &[d(20, 2, 0.0)]);
    let s = turn(&mut q);
    assert!(s.contains(&20));
    assert_eq!(s.iter().filter(|&&k| (10..16).contains(&k)).count(), 5);
}

#[test]
fn io_sync_aborts_an_in_flight_demand_no_longer_wanted_and_says_so_once() {
    let mut q = IoQueue::<32>::new();
    let (keep, drop) = (d(1, 0, 0.0), d(2, 0, 1.0));
    q.sync(0, &[keep, drop]);
    assert_eq!(turn(&mut q).len(), 2);
    q.sync(0, &[keep]);
    assert_eq!(q.aborted(), [2]);
    q.clear_aborted();
    q.sync(0, &[keep]);
    assert!(q.aborted().is_empty());
    assert!(q.settle(2).is_some());
    assert!(q.settle(1).is_some());
    assert_eq!(q.pending_count(0), 0);
}

#[test]
fn io_running_answers_only_for_keys_in_flight() {
    let mut q = IoQueue::<32>::new();
    assert!(!q.running(1));
    q.sync(0, &[d(1, 0, 0.0)]);
    assert!(!q.running(1), "queued, not yet pumped");
    turn(&mut q);
    assert!(q.running(1));
    q.settle(1);
    assert!(!q.running(1));
}

#[test]
fn io_sync_drops_a_queued_demand_no_longer_wanted() {
    let mut q = IoQueue::<32>::new();
    let fill: Vec<Demand> = (100..106).map(|k| d(k, 1, -1.0)).collect();
    q.sync(1, &fill);
    turn(&mut q);
    q.sync(0, &[d(50, 0, 0.0)]);
    assert!(turn(&mut q).is_empty());
    assert_eq!(q.pending_count(0), 1);
    q.sync(0, &[]);
    assert_eq!(q.pending_count(0), 0);
    for x in &fill {
        q.settle(x.key);
    }
    assert!(!turn(&mut q).contains(&50), "freeing slots must not resurrect it");
}

#[test]
fn io_re_submitting_a_running_key_does_not_start_a_second_run() {
    let mut q = IoQueue::<32>::new();
    q.sync(0, &[d(1, 0, 0.0)]);
    assert_eq!(turn(&mut q), [1]);
    q.sync(0, &[d(1, 0, 0.0)]);
    assert!(turn(&mut q).is_empty());
    assert_eq!(q.pending_count(0), 1);
    q.settle(1);
    assert_eq!(q.pending_count(0), 0);
}

#[test]
fn io_a_full_queue_refuses_what_it_cannot_hold() {
    let mut q = IoQueue::<4>::new();
    let h: Vec<Demand> = (0..6).map(|i| d(i, 0, i as f64)).collect();
    assert_eq!(q.sync(0, &h), 2);
    assert_eq!(q.pending_count(0), 4);
}

#[test]
fn io_fewer_credits_than_are_running_start_nothing_more() {
    let mut q = IoQueue::<32>::new();
    let h: Vec<Demand> = (0..7).map(|i| d(i, 0, i as f64)).collect();
    q.sync(0, &h);
    assert_eq!(turn(&mut q).len(), 6);
    q.pump(3);
    assert!(q.started().is_empty());
}

#[test]
fn io_settle_hands_back_the_demand_and_when_it_was_enqueued_started_and_ended() {
    let mut q = IoQueue::<32>::new();
    let x = d(1, 3, 2.0);
    q.sync(3, &[x]);
    turn(&mut q);
    let s = q.settle(1).expect("in flight");
    assert_eq!(s.demand, x);
    assert!(s.enqueue_ms <= s.start_ms && s.start_ms <= s.end_ms);
    assert_eq!(q.settle(1), None);
}
