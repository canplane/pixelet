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
        approaches: &[],
    })
    .top;
    for (i, t) in top.iter().enumerate() {
        assert!((t - (20.0 + 2.0 * i as f64)).abs() < 1e-9, "{i}: {t}");
    }
}

#[test]
fn a_clearance_raises_the_deck_and_it_still_meets_its_ends() {
    let half = (6.0 / PREFERRED_SLOPE / 5.0).ceil() as usize + 1;
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
        approaches: &[],
    })
    .top;
    assert_eq!(top[half], 6.0);
    assert_eq!((top[0], top[n - 1]), (0.0, 0.0));
    for w in top.windows(2) {
        assert!((w[1] - w[0]).abs() <= PREFERRED_SLOPE * 5.0 + 1e-9, "a step: {w:?}");
    }
}

#[test]
fn a_clearance_too_near_an_end_is_met_by_a_steeper_grade() {
    let adj = chain(5, 5.0);
    let mut need = vec![0.0; 5];
    need[1] = 50.0;
    need[2] = 10.0;
    let top = solve(&Problem {
        adj: &adj,
        need: &need,
        ends: &[(0, 0.0), (4, 0.0)],
        ground: &[0.0; 5],
        carried: &[true; 5],
        approaches: &[],
    })
    .top;
    assert_eq!(top[1], 50.0);
    assert!((top[2] - 50.0 * 2.0 / 3.0).abs() < 1e-9, "on the line from the held point to the end: {top:?}");
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
        ends: &[],
        ground,
        carried: &carried,
        approaches: &[&left, &right],
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

fn star(arm: usize, step: f64) -> (Vec<Vec<(u32, f64)>>, [u32; 3]) {
    let n = 1 + 3 * arm;
    let mut adj = vec![Vec::new(); n];
    let mut tips = [0; 3];
    for (a, tip) in tips.iter_mut().enumerate() {
        let mut prev = 0u32;
        for i in 0..arm {
            let v = (1 + a * arm + i) as u32;
            adj[prev as usize].push((v, step));
            adj[v as usize].push((prev, step));
            prev = v;
        }
        *tip = prev;
    }
    (adj, tips)
}

#[test]
fn where_spans_meet_no_step_is_steeper_than_the_steepest_two_ends() {
    // The steepest pair is 1.6 m over 20 m, exactly a deck's steepest.
    let (adj, tips) = star(2, 5.0);
    let need = vec![f64::NEG_INFINITY; adj.len()];
    let ground = vec![0.0; adj.len()];
    let ends = [(tips[0], 0.0), (tips[1], 0.0), (tips[2], PREFERRED_SLOPE * 20.0)];
    let p = solve(&Problem {
        adj: &adj,
        need: &need,
        ends: &ends,
        ground: &ground,
        carried: &vec![true; adj.len()],
        approaches: &[],
    });
    assert!(p.forced.iter().all(|f| !f));
    assert!((p.top[0] - PREFERRED_SLOPE * 10.0).abs() < 1e-9, "the junction: {}", p.top[0]);
    for (v, edges) in adj.iter().enumerate() {
        for &(w, d) in edges {
            assert!((p.top[v] - p.top[w as usize]).abs() <= PREFERRED_SLOPE * d + 1e-9, "{v}-{w}: {:?}", (p.top[v], p.top[w as usize]));
        }
    }
}

#[test]
fn ends_steeper_apart_than_a_deck_climbs_force_only_the_stretch_between_them() {
    // Two tips 2 m apart in 20 m are forced; the third arm's gentler stretch is not.
    let mut adj = vec![Vec::new(); 6];
    for &(a, b, d) in &[(0u32, 1u32, 5.0), (1, 2, 5.0), (0, 3, 5.0), (3, 4, 5.0), (0, 5, 20.0)] {
        adj[a as usize].push((b, d));
        adj[b as usize].push((a, d));
    }
    let need = vec![f64::NEG_INFINITY; 6];
    let ground = vec![0.0; 6];
    let p = solve(&Problem {
        adj: &adj,
        need: &need,
        ends: &[(2, 0.0), (4, 2.0), (5, 0.0)],
        ground: &ground,
        carried: &[true; 6],
        approaches: &[],
    });
    assert!((p.top[0] - 1.0).abs() < 1e-9, "the junction: {}", p.top[0]);
    assert!(p.forced[0] && p.forced[1] && p.forced[3]);
    assert!((p.top[5] - 0.0).abs() < 1e-9);
}

#[test]
fn a_pair_of_ends_steeper_than_a_deck_climbs_is_forced() {
    let adj = chain(3, 2.5);
    let need = vec![0.0; 3];
    let p = solve(&Problem {
        adj: &adj,
        need: &need,
        ends: &[(0, 0.0), (2, 1.0)],
        ground: &need,
        carried: &[true; 3],
        approaches: &[],
    });
    assert!(p.forced[1]);
    assert_eq!(p.top, vec![0.0, 0.5, 1.0]);
}
