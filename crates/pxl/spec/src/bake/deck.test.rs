// pxl-spec/bake/deck.test.rs

use super::*;

fn one(points: &[[f64; 2]], tops: &[i16], width_cells: f64) -> Decks {
    let mut b = Builder::default();
    b.push(points.iter().zip(tops).map(|(&p, &t)| (p, t)), (width_cells * POINT_SCALE) as u8, 3, Form::Deck);
    b.finish()
}

#[test]
fn a_column_is_under_a_deck_within_its_reach() {
    // Four cells wide: two either side of the line.
    let d = one(&[[10.0, 10.0], [30.0, 10.0]], &[100, 120], 4.0);
    let over: Vec<Over> = d.over_point([20.0, 12.0]).collect();
    assert_eq!(over.len(), 1);
    assert_eq!(over[0].top, 110.0);
    assert_eq!(d.over_point([20.0, 12.5]).count(), 0);
}

#[test]
fn a_narrow_deck_along_a_diagonal_is_one_band() {
    let d = one(&[[0.0, 0.0], [40.0, 40.0]], &[10, 10], 0.5);
    let mut covered = std::collections::HashSet::new();
    d.over_grid([0.0, 0.0], 1.0, [40, 40], |x, y, _, _| {
        covered.insert((x, y));
    });
    for k in 0..39 {
        assert!(covered.contains(&(k + 1, k)) || covered.contains(&(k, k + 1)), "cells {k} and {} meet only at a corner", k + 1);
    }
}

#[test]
fn a_deck_is_kept_up_the_pyramid_by_its_width_alone() {
    let empty = Decks::default();
    // Two cells wide: one cell a level up, kept at its own width.
    let wide = one(&[[0.0, 0.0], [100.0, 0.0]], &[50, 50], 2.0);
    let up = fold_decks([&wide, &empty, &empty, &empty], 1);
    assert_eq!(up.len(), 1);
    assert_eq!(up.deck(0).width_cells(), 1.0);
    // Under a cell wide: under half the parent's, and gone, whatever it is.
    let narrow = one(&[[0.0, 0.0], [100.0, 0.0]], &[50, 50], 0.75);
    assert_eq!(fold_decks([&narrow, &empty, &empty, &empty], 1).len(), 0);
}

#[test]
fn a_straight_level_deck_folds_to_its_ends() {
    let d = one(&[[0.0, 0.0], [10.0, 0.0], [20.0, 0.0], [30.0, 0.0]], &[40, 40, 40, 40], 4.0);
    let empty = Decks::default();
    let up = fold_decks([&d, &empty, &empty, &empty], 1);
    assert_eq!(up.deck(0).points.len(), 2);
    assert_eq!(up.deck(0).point(1), [15.0, 0.0]);
}

#[test]
fn a_bore_carves_only_where_the_ground_stands_over_its_floor() {
    let over = |top: f64| Over {
        top,
        kind: 3,
        form: Form::Bore,
        pier: false,
    };
    assert_eq!(over(10.0).occupies(10), Occupies::Grade);
    assert_eq!(over(10.0).occupies(11), Occupies::Through { air: [10, 11] });
    assert_eq!(over(10.0).occupies(40), Occupies::Through { air: [10, 10 + BORE_HEIGHT] });
}
