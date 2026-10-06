// pxl-kernel/world/quality.test.rs

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

/// The reference view asks `SPLIT` itself, a screen half as tall a cell
/// twice as wide; finer is held at `SPLIT`.
#[test]
fn the_target_is_the_screens_cell_and_never_finer_than_the_criterion() {
    let _g = crate::serial();
    let v = view();
    let kept = (v.height, v.fov);
    v.fov = 50.0;
    let at = |height: f64| {
        view().height = height;
        target().scale()
    };
    let span = |height: f64| 2.0 * libm::atan(pxl_spec::space::lod::target_cell_px() * libm::tan(25f64.to_radians()) / height) * SPLIT;
    assert!((span(1080.0) - 1.0).abs() < 0.01, "the reference view's cell is not the criterion's: {}", span(1080.0));
    assert_eq!(at(1080.0), 1.0);
    assert!((at(540.0) - span(540.0)).abs() < 1e-12, "not the angle half the height's cell spans: {}", at(540.0));
    assert!((at(540.0) / span(1080.0) - 2.0).abs() < 1e-4, "half the height, not about twice the cell: {}", at(540.0));
    assert_eq!(at(2160.0), 1.0, "a logical screen finer than the criterion asked past it");
    (v.height, v.fov) = kept;
}
