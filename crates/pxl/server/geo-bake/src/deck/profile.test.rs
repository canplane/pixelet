// pxl-geo-bake/deck/profile.test.rs

use super::*;

/// A chain of `n` points `step` metres apart.
fn chain(n: usize, step: f64) -> Vec<Vec<(u32, f64)>> {
    let mut adj = vec![Vec::new(); n];
    for i in 1..n {
        adj[i - 1].push((i as u32, step));
        adj[i].push(((i - 1) as u32, step));
    }
    adj
}

#[test]
fn a_bare_span_is_the_straight_line_between_its_ends() {
    let adj = chain(11, 10.0);
    let need = vec![0.0; 11];
    let top = solve(&Problem {
        adj: &adj,
        need: &need,
        ends: &[(0, 20.0), (10, 40.0)],
        ground: &need,
        carried: &[true; 11],
    });
    for (i, t) in top.iter().enumerate() {
        assert!((t - (20.0 + 2.0 * i as f64)).abs() < 1e-9, "{i}: {t}");
    }
}

#[test]
fn a_clearance_raises_the_deck_and_it_still_meets_its_ends() {
    // Long enough either side to climb 6 m at the steepest a deck may.
    let half = (6.0 / MAX_SLOPE / 5.0).ceil() as usize + 1;
    let n = 2 * half + 1;
    let adj = chain(n, 5.0);
    let mut need = vec![0.0; n];
    need[half] = 6.0;
    let top = solve(&Problem {
        adj: &adj,
        need: &need,
        ends: &[(0, 0.0), (n as u32 - 1, 0.0)],
        ground: &need,
        carried: &vec![true; n],
    });
    assert_eq!(top[half], 6.0);
    assert_eq!((top[0], top[n - 1]), (0.0, 0.0));
    for w in top.windows(2) {
        assert!((w[1] - w[0]).abs() <= MAX_SLOPE * 5.0 + 1e-9, "a step: {w:?}");
    }
}

#[test]
fn a_clearance_too_near_an_end_is_held_to_what_the_deck_can_climb() {
    let adj = chain(5, 5.0);
    let mut need = vec![0.0; 5];
    need[1] = 50.0;
    let top = solve(&Problem {
        adj: &adj,
        need: &need,
        ends: &[(0, 0.0), (4, 0.0)],
        ground: &[0.0; 5],
        carried: &[true; 5],
    });
    assert_eq!(top[1], MAX_SLOPE * 5.0);
}

#[test]
fn an_approach_lies_on_its_ground_past_where_the_deck_comes_down_to_it() {
    // A bridge over a 6 m clearance at 58..=62, its approaches out to ground
    // at 0 m on either side, and a valley 20 m deep beside one of them.
    let n = 121;
    let adj = chain(n, 10.0);
    let ground: Vec<f64> = (0..n).map(|i| if (5..15).contains(&i) { -20.0 } else { 0.0 }).collect();
    let mut need = ground.clone();
    need[60] = 6.0;
    let carried: Vec<bool> = (0..n).map(|i| (58..=62).contains(&i)).collect();
    let top = solve(&Problem {
        adj: &adj,
        need: &need,
        ends: &[(0, 0.0), (n as u32 - 1, 0.0)],
        ground: &ground,
        carried: &carried,
    });
    assert_eq!(top[60], 6.0);
    for (i, t) in top.iter().enumerate().take(15).skip(5) {
        assert_eq!(*t, -20.0, "{i}");
    }
}
