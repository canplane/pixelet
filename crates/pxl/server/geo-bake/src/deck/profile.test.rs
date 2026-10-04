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
fn deck(floor: &[f64], halo: f64) -> Vec<f64> {
    let adj = chain(floor.len(), 5.0);
    let floor: Vec<f64> = floor.iter().map(|&f| if f > 0.0 { f } else { -NONE }).collect();
    solve(&Problem {
        adj: &adj,
        ground: &vec![0.0; floor.len()],
        floor: &floor,
        ceiling: &vec![NONE; floor.len()],
        base: &vec![2.0; floor.len()],
        ends: &vec![false; floor.len()],
        halo: &vec![halo; floor.len()],
    })
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
fn a_structure_runs_straight_between_its_ends_and_bends_only_for_a_need() {
    // ends held 4 m and 2 m over flat ground, a 6 m need at 10
    let adj = chain(21, 5.0);
    let mut floor = vec![1.0; 21];
    (floor[0], floor[20], floor[10]) = (4.0, 2.0, 6.0);
    let mut ends = vec![false; 21];
    (ends[0], ends[20]) = (true, true);
    let p = solve(&Problem {
        adj: &adj,
        ground: &[0.0; 21],
        floor: &floor,
        ceiling: &[NONE; 21],
        base: &[1.0; 21],
        ends: &ends,
        halo: &[1000.0; 21],
    });
    assert_eq!((p[0], p[20], p[10]), (4.0, 2.0, 6.0));
    // past the need's reach the line between the ends holds
    let chord = |i: usize| 4.0 - 2.0 * i as f64 / 20.0;
    assert!((p[2] - chord(2)).abs() < 1e-9 && (p[18] - chord(18)).abs() < 1e-9);
}
