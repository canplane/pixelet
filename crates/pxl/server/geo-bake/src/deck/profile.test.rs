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

/// A chain 5 m a step over flat ground, its own where `need` is above it.
fn deck(need: &[f64], halo: f64) -> Profile {
    let adj = chain(need.len(), 5.0);
    let own: Vec<bool> = need.iter().map(|&n| n > 0.0).collect();
    solve(&Problem { adj: &adj, need, own: &own, pinned: &vec![false; need.len()], halo })
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
fn a_structure_keeps_its_form_to_its_end_and_its_approach_meets_the_ground_at_the_grade() {
    // a 2 m shell over 20..=30, an approach either side
    let need: Vec<f64> = (0..51).map(|i| if (20..=30).contains(&i) { 2.0 } else { 0.0 }).collect();
    let p = deck(&need, 1000.0);
    assert!((20..=30).all(|i| p.top[i] == 2.0));
    assert!((p.top[31] - (2.0 - PREFERRED_SLOPE * 5.0)).abs() < 1e-9);
    assert_eq!((p.top[35], p.top[15]), (0.0, 0.0));
    assert!(p.forced.iter().all(|f| !f));
}

#[test]
fn an_approach_whose_way_ends_first_meets_the_ground_there_as_steep_as_it_must() {
    // a 6 m need over 0..=10, an approach of four steps ending on the ground
    let adj = chain(15, 5.0);
    let need: Vec<f64> = (0..15).map(|i| if i <= 10 { 6.0 } else { 0.0 }).collect();
    let own: Vec<bool> = (0..15).map(|i| i <= 10).collect();
    let pinned: Vec<bool> = (0..15).map(|i| i == 14).collect();
    let p = solve(&Problem { adj: &adj, need: &need, own: &own, pinned: &pinned, halo: 1000.0 });
    assert_eq!((p.top[10], p.top[14]), (6.0, 0.0));
    assert!((11..14).all(|i| (p.top[i] - 6.0 * (14 - i) as f64 / 4.0).abs() < 1e-9));
}
