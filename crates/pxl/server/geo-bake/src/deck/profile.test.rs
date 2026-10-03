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
    solve(&Problem { adj: &adj, need, ground: &vec![0.0; need.len()], carried: &vec![true; need.len()], approaches: &[], halo })
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

/// `shell` over `deck`, an approach out of each end; 5 m a step.
fn with_approaches(ground: &[f64], deck: std::ops::RangeInclusive<usize>, shell: f64) -> Profile {
    let n = ground.len();
    let adj = chain(n, 5.0);
    let carried: Vec<bool> = (0..n).map(|i| deck.contains(&i)).collect();
    let need: Vec<f64> = (0..n).map(|i| ground[i] + if carried[i] { shell } else { 0.0 }).collect();
    let left: Vec<(u32, f64)> = (0..=*deck.start()).rev().map(|i| (i as u32, (*deck.start() - i) as f64 * 5.0)).collect();
    let right: Vec<(u32, f64)> = (*deck.end()..n).map(|i| (i as u32, (i - *deck.end()) as f64 * 5.0)).collect();
    solve(&Problem {
        adj: &adj,
        need: &need,
        ground,
        carried: &carried,
        approaches: &[&left, &right],
        halo: 1000.0,
    })
}

/// Right approach: over its ground, falling, then on it.
fn assert_meets_its_ground(p: &Profile, ground: &[f64], from: usize) {
    let contact = (from + 1..ground.len()).find(|&i| p.top[i] == ground[i]).expect("the approach meets its ground");
    for (i, (&top, &g)) in p.top.iter().zip(ground).enumerate().skip(from + 1) {
        assert!(top >= g, "{i} under its ground");
        if i < contact {
            assert!(top <= p.top[i - 1] + 1e-9, "{i} rises before its contact");
        } else {
            assert_eq!(top, g, "{i} off its ground past its contact");
        }
    }
}

#[test]
fn an_approach_meets_its_ground_at_the_grade_and_follows_it_from_there() {
    // a dip past the contact
    let ground: Vec<f64> = (0..60).map(|i| if (40..45).contains(&i) { -6.0 } else { 0.0 }).collect();
    let p = with_approaches(&ground, 20..=30, 2.0);
    assert_meets_its_ground(&p, &ground, 30);
    assert_eq!(p.top[30 + 5], 0.0, "2 m at 8 % is 25 m");
    assert!(p.forced.iter().skip(31).all(|f| !f));
}

#[test]
fn an_approach_down_a_slope_steeper_than_the_grade_is_no_higher_than_its_joint() {
    // ground falling 20 %
    let ground: Vec<f64> = (0..60).map(|i| if i <= 30 { 0.0 } else { -(i - 30) as f64 }).collect();
    let p = with_approaches(&ground, 20..=30, 2.0);
    assert_meets_its_ground(&p, &ground, 30);
    for (i, (&top, &g)) in p.top.iter().zip(&ground).enumerate().skip(31) {
        assert!(top - g <= 2.0 + 1e-9, "{i} stands higher than the deck's own shell");
    }
}

#[test]
fn a_cutting_past_a_hollow_by_its_portal_does_not_dip_and_climb() {
    // a bore 7.5 m deep, a hollow by its portal
    let mut ground: Vec<f64> = (0..80).map(|i| if i <= 30 { 0.0 } else { -3.2 }).collect();
    ground[31] = 5.7;
    ground[32] = 5.7;
    let p = with_approaches(&ground, 10..=30, 7.5);
    assert_meets_its_ground(&p, &ground, 30);
}

