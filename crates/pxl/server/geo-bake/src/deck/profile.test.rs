// pxl-geo-bake/deck/profile.test.rs

use super::*;

fn chain(n: usize, step: f64) -> Vec<Vec<(u32, f64)>> {
    let mut adj = vec![Vec::new(); n];
    for i in 1..n {
        adj[i - 1].push((i as u32, step));
        adj[i].push(((i - 1) as u32, step));
    }
    adj
}

/// A chain 5 m a step under a 2 m shell, pinned where `need` is 0.
fn deck(need: &[f64], halo: f64) -> Profile {
    let adj = chain(need.len(), 5.0);
    let (hard, pinned): (Vec<bool>, Vec<bool>) = need.iter().map(|&n| (n > 2.0, n == 0.0)).unzip();
    solve(&Problem { adj: &adj, need, ground: &vec![0.0; need.len()], hard: &hard, pinned: &pinned, halo })
}

#[test]
fn a_need_stands_over_its_point_and_falls_away_at_the_grade() {
    let mut need = vec![2.0; 101];
    need[50] = 8.0;
    let p = deck(&need, 1000.0);
    assert_eq!(p.top[50], 8.0);
    assert!((p.top[49] - (8.0 - PREFERRED_SLOPE * 5.0)).abs() < 1e-9);
    assert!(p.top.iter().zip(&need).all(|(t, n)| t >= n));
    assert!(p.forced.iter().all(|f| !f));
}

#[test]
fn nothing_past_the_halo_moves_a_point() {
    let mut need = vec![2.0; 101];
    need[0] = 30.0;
    let near = deck(&need, 100.0);
    need[0] = 60.0;
    let far = deck(&need, 100.0);
    assert_eq!(near.top[25..], far.top[25..], "20 steps is past a 100 m halo");
}

#[test]
fn a_need_too_high_for_the_grade_lands_on_the_ground_by_the_halo_without_a_step() {
    let mut need = vec![2.0; 101];
    need[0] = 30.0;
    let p = deck(&need, 100.0);
    assert!((p.top[20] - 2.0).abs() < 1e-9, "on its need at the halo: {}", p.top[20]);
    assert!(p.forced[1]);
    for w in p.top.windows(2) {
        assert!((w[0] - w[1]).abs() <= (30.0 - 2.0) / 100.0 * 5.0 + 1e-9);
    }
}

#[test]
fn a_deck_comes_down_to_its_pinned_ends_at_the_grade() {
    let mut need = vec![2.0; 21];
    (need[0], need[20]) = (0.0, 0.0);
    let p = deck(&need, 1000.0);
    assert_eq!((p.top[0], p.top[20]), (0.0, 0.0));
    assert!((p.top[1] - PREFERRED_SLOPE * 5.0).abs() < 1e-9);
    assert_eq!(p.top[10], 2.0);
    assert!(p.forced.iter().all(|f| !f));
}

#[test]
fn a_need_too_near_a_pin_takes_the_grade_it_must_and_no_step() {
    let mut need = vec![2.0; 41];
    (need[0], need[3]) = (0.0, 8.0);
    let p = deck(&need, 1000.0);
    assert_eq!((p.top[0], p.top[3]), (0.0, 8.0));
    let grade = 8.0 / 15.0;
    assert!((1..=3).all(|i| (p.top[i] - grade * 5.0 * i as f64).abs() < 1e-9));
    assert!(p.top.iter().zip(&need).all(|(t, n)| t >= n));
    assert!(p.forced[1] && !p.forced[10]);
}
