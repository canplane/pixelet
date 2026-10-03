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

const NONE: f64 = f64::INFINITY;

/// A chain 5 m a step over flat ground, a floor where `floor` is above it.
fn deck(floor: &[f64], halo: f64) -> Profile {
    let adj = chain(floor.len(), 5.0);
    let floor: Vec<f64> = floor.iter().map(|&f| if f > 0.0 { f } else { -NONE }).collect();
    solve(&Problem { adj: &adj, ground: &vec![0.0; floor.len()], floor: &floor, ceiling: &vec![NONE; floor.len()], base: &vec![2.0; floor.len()], ends: &vec![false; floor.len()], halo: &vec![halo; floor.len()] })
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
fn a_structure_runs_straight_between_its_ends_and_bends_only_for_a_need() {
    // ends held 4 m and 2 m over flat ground, a 6 m need at 10
    let adj = chain(21, 5.0);
    let mut floor = vec![1.0; 21];
    (floor[0], floor[20], floor[10]) = (4.0, 2.0, 6.0);
    let mut ends = vec![false; 21];
    (ends[0], ends[20]) = (true, true);
    let p = solve(&Problem { adj: &adj, ground: &[0.0; 21], floor: &floor, ceiling: &[NONE; 21], base: &[1.0; 21], ends: &ends, halo: &[1000.0; 21] });
    assert_eq!((p.top[0], p.top[20], p.top[10]), (4.0, 2.0, 6.0));
    // past the need's reach the line between the ends holds
    let chord = |i: usize| 4.0 - 2.0 * i as f64 / 20.0;
    assert!((p.top[2] - chord(2)).abs() < 1e-9 && (p.top[18] - chord(18)).abs() < 1e-9);
}
