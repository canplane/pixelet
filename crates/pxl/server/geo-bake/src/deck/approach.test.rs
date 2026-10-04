// pxl-geo-bake/deck/approach.test.rs

use super::*;

fn branch(length_m: f64, n: usize) -> Branch {
    Branch {
        end: 0,
        way: 0,
        from: 0.0,
        to: 1.0,
        length_m,
        across: None,
        at: (0..n).map(|k| (k as f64 * 5.0, [k as f64, 0.0])).collect(),
    }
}

#[test]
fn the_distance_from_the_ground_falls_at_the_grade_however_the_ground_runs() {
    // a cutting 4 m deep, out of a portal into ground rising 10% away from it
    let b = branch(200.0, 41);
    let ground: Vec<f64> = b.at.iter().map(|&(d, _)| 0.1 * d).collect();
    let to = meets(&b, -4.0, None);
    // out at 4 / 0.08 = 50 m
    assert!((to - 50.0 / 200.0).abs() < 1e-9);
    let points = profile(&b, -4.0, &ground, to);
    assert_eq!(points.len(), 10);
    for (k, p) in points.iter().enumerate() {
        assert!((ground[k] - p[2] - (4.0 - PREFERRED_SLOPE * b.at[k].0)).abs() < 1e-9);
    }
}

#[test]
fn a_short_stretch_is_taken_steeper() {
    let b = branch(20.0, 5);
    let points = profile(&b, 4.0, &[0.0; 5], meets(&b, 4.0, None));
    assert_eq!(points.iter().map(|p| p[2]).collect::<Vec<_>>(), vec![4.0, 3.0, 2.0, 1.0]);
}

#[test]
fn two_ends_across_a_stretch_take_what_they_need_and_share_it_by_their_change_when_short() {
    // 3.9 m and 7.5 m need 49 m and 94 m: both fit in 200 m
    assert!((meets(&branch(200.0, 41), 3.9, Some(-7.5)) - 3.9 / 0.08 / 200.0).abs() < 1e-9);
    // in 100 m they meet where each has made its share of the 11.4 m, the same bits from either end
    let (a, mut b) = (branch(100.0, 21), branch(100.0, 21));
    (b.from, b.to) = (1.0, 0.0);
    assert_eq!(meets(&a, 3.9, Some(-7.5)), meets(&b, -7.5, Some(3.9)));
    assert!((meets(&a, 3.9, Some(-7.5)) - 3.9 / 11.4).abs() < 1e-12);
}
