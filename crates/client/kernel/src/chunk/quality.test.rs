// pxl-kernel/chunk/quality.test.rs

use super::*;

/// Coarser at once to the feasible step; finer one step at a time after the
/// wait, which restarts after each step and whenever the room goes.
#[test]
fn coarser_at_once_and_finer_a_step_at_a_time_after_the_wait() {
    let mut s = Smooth::default();
    let wait = 1_000.0;
    assert_eq!(s.follow(4, 4, wait, 0.0), 4);
    assert_eq!(s.follow(1, 1, wait, 10.0), 4);
    assert_eq!(s.follow(1, 1, wait, 900.0), 4);
    assert_eq!(s.follow(1, 1, wait, 1_010.0), 3);
    assert_eq!(s.follow(1, 1, wait, 1_500.0), 3);
    assert_eq!(s.follow(1, 1, wait, 2_010.0), 3);
    assert_eq!(s.follow(1, 1, wait, 2_500.0), 2);
    assert_eq!(s.follow(1, 2, wait, 2_600.0), 2);
    assert_eq!(s.follow(1, 1, wait, 3_000.0), 2);
    assert_eq!(s.follow(1, 1, wait, 3_999.0), 2);
    assert_eq!(s.follow(1, 1, wait, 4_000.0), 1);
    assert_eq!(s.follow(0, 0, wait, 4_500.0), 1);
    assert_eq!(s.follow(5, 5, wait, 4_600.0), 5);
    let mut t = 4_600.0;
    for _ in 0..20 {
        t += 1_000.0;
        s.follow(0, 0, wait, t);
    }
    assert_eq!(s.at(), 0);
}

#[test]
fn one_instant_is_one_step() {
    let mut s = Smooth::default();
    s.follow(3, 3, 1_000.0, 0.0);
    assert_eq!(s.follow(0, 0, 1_000.0, 0.0), 3);
    assert_eq!(s.follow(0, 0, 1_000.0, 1_000.0), 2);
    assert_eq!(s.follow(0, 0, 1_000.0, 1_000.0), 2);
}
