// pxl-kernel/world/quality.test.rs

use super::*;

fn counts() -> Vec<Slots> {
    vec![[300, 1_900, 0], [250, 1_400, 0], [180, 900, 0], [90, 500, 0], [30, 200, 0], [0, 60, 0]]
}

/// The finest step that fits: what `around` answers asking only some.
fn feasible(frontier: &[Slots], arena: Slots, headroom: Slots, spare: Slots) -> Option<usize> {
    (0..frontier.len()).find(|&k| fits(frontier[k], arena, headroom, spare))
}

#[test]
fn with_room_for_the_target_the_target_is_in_force() {
    let mut s = Smooth::default();
    let c = counts();
    let (star, roomy) = (
        feasible(&c, [10_000, 10_000, 10_000], [40; POOLS], [0; POOLS]).unwrap(),
        feasible(&c, [10_000, 10_000, 10_000], [40; POOLS], [500, 500, 0]).unwrap(),
    );
    for t in 0..10 {
        assert_eq!(s.follow(star, roomy, 1_000.0, t as f64 * 500.0), 0);
    }
}

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

/// Asking only the steps about the one in force moves the smoothing as the
/// whole ladder would.
#[test]
fn asking_about_the_step_in_force_moves_as_the_whole_ladder_does() {
    let c = counts();
    let spare = [20, 100, 0];
    for arena in [
        [40, 300, 10_000],
        [100, 2_000, 10_000],
        [200, 600, 10_000],
        [260, 1_500, 10_000],
        [400, 2_000, 10_000],
        [1_000, 5_000, 10_000],
        [5, 50, 10_000],
    ] {
        let last = c.len() - 1;
        let star = feasible(&c, arena, [10; POOLS], [0; POOLS]).unwrap_or(last);
        let roomy = feasible(&c, arena, [10; POOLS], spare).unwrap_or(last);
        for at in 0..c.len() {
            let mut asked = Vec::new();
            let local = around(at, c.len(), |k, room| {
                asked.push(k);
                fits(c[k], arena, [10; POOLS], if room { spare } else { [0; POOLS] })
            });
            let (mut whole, mut near) = (Smooth::default(), Smooth::default());
            for s in [&mut whole, &mut near] {
                s.follow(at, at, 1_000.0, 0.0);
            }
            for t in [1.0, 2_000.0] {
                assert_eq!(whole.follow(star, roomy, 1_000.0, t), near.follow(local.0, local.1, 1_000.0, t), "{arena:?} from {at}");
            }
            let fit_at = fits(c[at], arena, [10; POOLS], [0; POOLS]);
            let most = if fit_at { 3 } else { star.max(at) - at + 1 };
            assert!(asked.len() <= most, "{arena:?} from {at}: asked {asked:?}");
        }
    }
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
