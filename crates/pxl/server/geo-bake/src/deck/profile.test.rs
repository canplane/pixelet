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
    let top = solve(&Problem { adj: &adj, need: &need, ends: &[(0, 20.0), (10, 40.0)] });
    for (i, t) in top.iter().enumerate() {
        assert!((t - (20.0 + 2.0 * i as f64)).abs() < 1e-9, "{i}: {t}");
    }
}

#[test]
fn a_clearance_raises_the_deck_and_it_still_meets_its_ends() {
    let adj = chain(21, 5.0);
    let mut need = vec![0.0; 21];
    need[10] = 6.0;
    let top = solve(&Problem { adj: &adj, need: &need, ends: &[(0, 0.0), (20, 0.0)] });
    assert_eq!(top[10], 6.0);
    assert_eq!((top[0], top[20]), (0.0, 0.0));
    for w in top.windows(2) {
        assert!((w[1] - w[0]).abs() <= MAX_SLOPE * 5.0 + 1e-9, "a step: {w:?}");
    }
}

#[test]
fn a_clearance_too_near_an_end_is_held_to_what_the_deck_can_climb() {
    let adj = chain(5, 5.0);
    let mut need = vec![0.0; 5];
    need[1] = 50.0;
    let top = solve(&Problem { adj: &adj, need: &need, ends: &[(0, 0.0), (4, 0.0)] });
    assert_eq!(top[1], MAX_SLOPE * 5.0);
}
