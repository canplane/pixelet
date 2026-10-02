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
    })
    .top;
    for (i, t) in top.iter().enumerate() {
        assert!((t - (20.0 + 2.0 * i as f64)).abs() < 1e-9, "{i}: {t}");
    }
}

#[test]
fn a_clearance_raises_the_deck_and_it_still_meets_its_ends() {
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
    })
    .top;
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
    })
    .top;
    assert_eq!(top[1], MAX_SLOPE * 5.0);
}

#[test]
fn an_approach_lies_on_its_ground_past_where_the_deck_comes_down_to_it() {
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
    })
    .top;
    assert_eq!(top[60], 6.0);
    for (i, t) in top.iter().enumerate().take(15).skip(5) {
        assert_eq!(*t, -20.0, "{i}");
    }
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
    let ends = [(tips[0], 0.0), (tips[1], 0.0), (tips[2], MAX_SLOPE * 20.0)];
    let p = solve(&Problem {
        adj: &adj,
        need: &need,
        ends: &ends,
        ground: &ground,
        carried: &vec![true; adj.len()],
    });
    assert!(p.forced.iter().all(|f| !f));
    assert!((p.top[0] - MAX_SLOPE * 10.0).abs() < 1e-9, "the junction: {}", p.top[0]);
    for (v, edges) in adj.iter().enumerate() {
        for &(w, d) in edges {
            assert!((p.top[v] - p.top[w as usize]).abs() <= MAX_SLOPE * d + 1e-9, "{v}-{w}: {:?}", (p.top[v], p.top[w as usize]));
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
    });
    assert!(p.forced[1]);
    assert_eq!(p.top, vec![0.0, 0.5, 1.0]);
}
