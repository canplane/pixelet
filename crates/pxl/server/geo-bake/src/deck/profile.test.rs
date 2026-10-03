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
    solve(&Problem { adj: &adj, ground: &vec![0.0; floor.len()], floor: &floor, ceiling: &vec![NONE; floor.len()], halo: &vec![halo; floor.len()] })
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
fn a_structure_keeps_its_form_to_its_end_and_its_way_meets_the_ground_at_the_grade() {
    // a 2 m floor over 20..=30, the way carrying on either side
    let floor: Vec<f64> = (0..51).map(|i| if (20..=30).contains(&i) { 2.0 } else { 0.0 }).collect();
    let p = deck(&floor, 1000.0);
    assert!((20..=30).all(|i| p.top[i] == 2.0));
    assert!((p.top[31] - (2.0 - PREFERRED_SLOPE * 5.0)).abs() < 1e-9);
    assert_eq!((p.top[35], p.top[15]), (0.0, 0.0));
    assert!(p.forced.iter().all(|f| !f));
}

#[test]
fn a_way_ending_first_meets_the_ground_there_as_steep_as_it_must() {
    // a 6 m floor over 0..=10, a way of four steps ending on the ground
    let adj = chain(15, 5.0);
    let mut floor: Vec<f64> = (0..15).map(|i| if i <= 10 { 6.0 } else { -NONE }).collect();
    let mut ceiling = vec![NONE; 15];
    (floor[14], ceiling[14]) = (0.0, 0.0);
    let p = solve(&Problem { adj: &adj, ground: &[0.0; 15], floor: &floor, ceiling: &ceiling, halo: &vec![1000.0; adj.len()] });
    assert_eq!((p.top[10], p.top[14]), (6.0, 0.0));
    assert!((11..14).all(|i| (p.top[i] - 6.0 * (14 - i) as f64 / 4.0).abs() < 1e-9));
}

#[test]
fn a_deck_and_a_bore_a_short_way_apart_are_one_road_between_them() {
    // a deck's 6 m floor over 0..=5, a bore's −6 m ceiling over 10..=15
    let adj = chain(16, 5.0);
    let floor: Vec<f64> = (0..16).map(|i| if i <= 5 { 6.0 } else { -NONE }).collect();
    let ceiling: Vec<f64> = (0..16).map(|i| if i >= 10 { -6.0 } else { NONE }).collect();
    let p = solve(&Problem { adj: &adj, ground: &[0.0; 16], floor: &floor, ceiling: &ceiling, halo: &vec![1000.0; adj.len()] });
    assert_eq!((p.top[5], p.top[10]), (6.0, -6.0));
    assert!((5..=10).all(|i| (p.top[i] - (6.0 - 12.0 * (i - 5) as f64 / 5.0)).abs() < 1e-9));
}

#[test]
fn a_bore_keeps_its_cover_to_its_portal_and_its_way_climbs_out_at_the_grade() {
    let adj = chain(51, 5.0);
    let ceiling: Vec<f64> = (0..51).map(|i| if (20..=30).contains(&i) { -7.5 } else { NONE }).collect();
    let p = solve(&Problem { adj: &adj, ground: &vec![0.0; 51], floor: &vec![-NONE; 51], ceiling: &ceiling, halo: &vec![1000.0; adj.len()] });
    assert!((20..=30).all(|i| p.top[i] == -7.5));
    assert!((p.top[31] - (-7.5 + PREFERRED_SLOPE * 5.0)).abs() < 1e-9);
    assert_eq!(p.top[50], 0.0);
}
