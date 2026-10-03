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

/// A carried chain 5 m a step over flat ground.
fn deck(need: &[f64], halo: f64) -> Profile {
    let adj = chain(need.len(), 5.0);
    solve(&Problem { adj: &adj, need, ground: &vec![0.0; need.len()], carried: &vec![true; need.len()], halo })
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
fn an_approach_comes_down_from_its_joint_to_its_next_point() {
    // a deck over 20..=30 on flat ground, an approach either side
    let n = 60;
    let adj = chain(n, 5.0);
    let carried: Vec<bool> = (0..n).map(|i| (20..=30).contains(&i)).collect();
    let need: Vec<f64> = (0..n).map(|i| if carried[i] { 2.0 } else { 0.0 }).collect();
    let p = solve(&Problem { adj: &adj, need: &need, ground: &vec![0.0; n], carried: &carried, halo: 1000.0 });
    assert_eq!((p.top[30], p.top[31]), (2.0, 0.0));
}
