// pxl-kernel/world/quality.test.rs

use super::*;

/// A ladder's counts, finest first, never less at a finer step.
fn counts() -> Vec<Slots> {
    vec![[300, 1_900], [250, 1_400], [180, 900], [90, 500], [30, 200], [0, 60]]
}

/// The feasible step is the finest whose count fits beside the headroom, in
/// every pool at once.
#[test]
fn the_feasible_step_is_the_finest_that_fits_every_pool() {
    let c = counts();
    assert_eq!(feasible(&c, [400, 2_000], 10, [0; POOLS]), Some(0));
    assert_eq!(feasible(&c, [400, 1_000], 10, [0; POOLS]), Some(2));
    assert_eq!(feasible(&c, [100, 2_000], 10, [0; POOLS]), Some(3));
    assert_eq!(feasible(&c, [100, 2_000], 10, [20, 0]), Some(4));
    assert_eq!(feasible(&c, [5, 50], 10, [0; POOLS]), None);
    for arena in [[40, 300], [200, 600], [260, 1_500], [1_000, 5_000]] {
        let Some(k) = feasible(&c, arena, 10, [0; POOLS]) else { continue };
        let fits = |k: usize| (0..POOLS).all(|p| c[k][p] + 10 <= arena[p]);
        assert!((k..c.len()).all(fits), "{arena:?}: a step coarser than the feasible one does not fit");
        assert!(k == 0 || !fits(k - 1), "{arena:?}: a finer step fits");
    }
}

/// With an arena that holds the target's count, the target is what is in
/// force, from the first step and at rest.
#[test]
fn with_room_for_the_target_the_target_is_in_force() {
    let mut s = Smooth::default();
    let c = counts();
    let (star, roomy) = (feasible(&c, [10_000, 10_000], 40, [0; POOLS]).unwrap(), feasible(&c, [10_000, 10_000], 40, [500, 500]).unwrap());
    for t in 0..10 {
        assert_eq!(s.follow(star, roomy, 1_000.0, t as f64 * 500.0), 0);
    }
}

/// Coarser is taken at once, to the feasible step and not a step at a time;
/// finer a step at a time, each only once the step has been roomy for the
/// wait, and the wait begins again after each step and whenever the room
/// goes.
#[test]
fn coarser_at_once_and_finer_a_step_at_a_time_after_the_wait() {
    let mut s = Smooth::default();
    let wait = 1_000.0;
    // The view needs step 4 now: taken in the same instant.
    assert_eq!(s.follow(4, 4, wait, 0.0), 4);
    // Room for step 1 comes back: nothing moves before the wait.
    assert_eq!(s.follow(1, 1, wait, 10.0), 4);
    assert_eq!(s.follow(1, 1, wait, 900.0), 4);
    // Then one step, and the wait again for the next, from the first time
    // it is seen roomy after the step.
    assert_eq!(s.follow(1, 1, wait, 1_010.0), 3);
    assert_eq!(s.follow(1, 1, wait, 1_500.0), 3);
    assert_eq!(s.follow(1, 1, wait, 2_010.0), 3);
    assert_eq!(s.follow(1, 1, wait, 2_500.0), 2);
    // Feasible but not roomy: the step finer waits, and its wait starts over.
    assert_eq!(s.follow(1, 2, wait, 2_600.0), 2);
    assert_eq!(s.follow(1, 1, wait, 3_000.0), 2);
    assert_eq!(s.follow(1, 1, wait, 3_999.0), 2);
    assert_eq!(s.follow(1, 1, wait, 4_000.0), 1);
    // Coarser again at once, in the middle of a wait.
    assert_eq!(s.follow(0, 0, wait, 4_500.0), 1);
    assert_eq!(s.follow(5, 5, wait, 4_600.0), 5);
    // At rest it comes back to the feasible step and stays.
    let mut t = 4_600.0;
    for _ in 0..20 {
        t += 1_000.0;
        s.follow(0, 0, wait, t);
    }
    assert_eq!(s.at(), 0);
}

/// Asked twice in one instant, the smoothing steps once.
#[test]
fn one_instant_is_one_step() {
    let mut s = Smooth::default();
    s.follow(3, 3, 1_000.0, 0.0);
    assert_eq!(s.follow(0, 0, 1_000.0, 0.0), 3);
    assert_eq!(s.follow(0, 0, 1_000.0, 1_000.0), 2);
    assert_eq!(s.follow(0, 0, 1_000.0, 1_000.0), 2);
}

/// Asking only the steps about the one in force moves the smoothing as the
/// whole ladder's feasibility does, wherever it stands, for counts never
/// less at a finer step — and asks the step in force, the one finer, and
/// the ones coarser only until one fits.
#[test]
fn asking_about_the_step_in_force_moves_as_the_whole_ladder_does() {
    let c = counts();
    let spare = [20, 100];
    for arena in [[40, 300], [100, 2_000], [200, 600], [260, 1_500], [400, 2_000], [1_000, 5_000], [5, 50]] {
        let last = c.len() - 1;
        let star = feasible(&c, arena, 10, [0; POOLS]).unwrap_or(last);
        let roomy = feasible(&c, arena, 10, spare).unwrap_or(last);
        for at in 0..c.len() {
            let mut asked = Vec::new();
            let local = around(at, c.len(), |k, room| {
                asked.push(k);
                fits(c[k], arena, 10, if room { spare } else { [0; POOLS] })
            });
            let (mut whole, mut near) = (Smooth::default(), Smooth::default());
            for s in [&mut whole, &mut near] {
                s.follow(at, at, 1_000.0, 0.0);
            }
            for t in [1.0, 2_000.0] {
                assert_eq!(whole.follow(star, roomy, 1_000.0, t), near.follow(local.0, local.1, 1_000.0, t), "{arena:?} from {at}");
            }
            let fit_at = fits(c[at], arena, 10, [0; POOLS]);
            let most = if fit_at { 3 } else { star.max(at) - at + 1 };
            assert!(asked.len() <= most, "{arena:?} from {at}: asked {asked:?}");
        }
    }
}
