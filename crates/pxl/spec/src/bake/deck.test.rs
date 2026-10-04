// pxl-spec/bake/deck.test.rs

use super::*;
use crate::bake::ground::{GroundChannelKey, ShownGround};
use crate::procedural::{pier_stands, Frame, PIER_ALONG, PIER_STEP_M};
use crate::space::adapter::epsg3857::u_per_m;

fn one(points: &[[f64; 2]], tops: &[i16], width_cells: f64) -> Decks {
    let mut b = Builder::default();
    b.push(points.iter().zip(tops).map(|(&p, &t)| (p, t)), (width_cells * POINT_SCALE) as u8, 3, Form::Deck);
    b.finish()
}

#[test]
fn a_column_is_under_a_deck_within_its_reach() {
    // Four cells wide: two either side of the line.
    let d = one(&[[10.0, 10.0], [30.0, 10.0]], &[100, 120], 4.0);
    let over: Vec<Over> = d.over_point(None, [20.0, 12.0]).collect();
    assert_eq!(over.len(), 1);
    assert_eq!(over[0].top, 110.0);
    assert_eq!(d.over_point(None, [20.0, 12.5]).count(), 0);
}

#[test]
fn a_narrow_deck_along_a_diagonal_is_one_band() {
    let d = one(&[[0.0, 0.0], [40.0, 40.0]], &[10, 10], 0.5);
    let mut covered = std::collections::HashSet::new();
    d.over_grid(None, [0.0, 0.0], 1.0, [40, 40], |x, y, _, _| {
        covered.insert((x, y));
    });
    for k in 0..39 {
        assert!(covered.contains(&(k + 1, k)) || covered.contains(&(k, k + 1)), "cells {k} and {} meet only at a corner", k + 1);
    }
}

#[test]
fn a_pier_stands_at_each_world_station_and_its_column_keeps_it_unless_blocked() {
    // Two u a cell; a pier is the cells within its half-length of a station.
    let frame = Some(Frame { corner: [0.0, 0.0], cell: 2.0 });
    let station = PIER_STEP_M * u_per_m(0.0) / 2.0;
    let d = one(&[[1.0, 3.0], [41.0, 3.0]], &[20, 20], 4.0);
    let piers: Vec<f64> = (0..42).map(|x| f64::from(x) + 0.5).filter(|&x| d.over_point(frame, [x, 3.0]).any(|o| o.pier)).collect();
    let expected: Vec<f64> = (0..42)
        .map(|x| f64::from(x) + 0.5)
        .filter(|&x| x > 1.0 && (x - round(x / station) * station).abs() <= PIER_ALONG * 4.0 / 2.0)
        .collect();
    assert!(!expected.is_empty());
    assert_eq!(piers, expected);
    let at = [expected[0], 3.0];
    let over = d.over_point(frame, at).next().unwrap();
    let Occupies::Above { pier: Some(pier), .. } = over.occupies(5, 0.5) else {
        panic!("no pier over ground at 5")
    };
    assert_eq!(pier, [5, 19]);
    let land = ShownGround {
        channel: GroundChannelKey::LAND,
        type_id: 1,
    };
    let road = ShownGround {
        channel: GroundChannelKey::TRANSPORT,
        type_id: 1,
    };
    assert_eq!(pier_stands(pier, land, d.over_point(frame, at), 5, 0.5), Some(pier));
    assert_eq!(pier_stands(pier, road, d.over_point(frame, at), 5, 0.5), None);
    let mut under = Builder::default();
    under.push([([0.0, 3.0], 10), ([40.0, 3.0], 10)], 4 * POINT_SCALE as u8, 3, Form::Deck);
    let under = under.finish();
    let column = d.over_point(frame, at).chain(under.over_point(frame, at));
    assert_eq!(pier_stands(pier, land, column, 5, 0.5), None);
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
fn a_full_parent_keeps_its_widest_lines_within_both_bounds() {
    let line = |b: &mut Builder, n: usize, y: f64, width_cells: f64| {
        // A zigzag two parent cells deep, which no simplification shortens.
        let points = (0..n).map(|i| ([i as f64 * 0.25, y + 4.0 * (i % 2) as f64], 40));
        b.push(points, (width_cells * POINT_SCALE) as u8, 3, Form::Deck);
    };
    // Points: four lines of 2000 and a short one; the narrowest long line does not fit, the short one does.
    let mut children: Vec<Decks> = (0..3)
        .map(|k| {
            let mut b = Builder::default();
            line(&mut b, 2000, 10.0, 8.0 - k as f64);
            b.finish()
        })
        .collect();
    let mut b = Builder::default();
    line(&mut b, 2000, 10.0, 4.0);
    line(&mut b, 2, 100.0, 2.0);
    children.push(b.finish());
    let up = fold_decks([&children[0], &children[1], &children[2], &children[3]], 1);
    let widths: Vec<f64> = up.iter().map(|d| d.width_cells()).collect();
    assert_eq!(widths, [4.0, 3.5, 3.0, 1.0]);
    // Decks: 1200 two-point lines, widest first, ties in the children's order.
    let mut b = Builder::default();
    for i in 0..300 {
        line(&mut b, 2, i as f64, if i < 100 { 2.0 } else { 4.0 });
    }
    let child = b.finish();
    let fold = || fold_decks([&child, &child, &child, &child], 1);
    let up = fold();
    assert_eq!(up.len(), MAX_DECKS);
    assert_eq!(up.iter().filter(|d| d.width_cells() == 2.0).count(), 800);
    assert_eq!(up, fold());
}

#[test]
fn each_form_is_drawn_from_its_surface() {
    use crate::cell::cube::cube_types::transport as t;
    let over = |form: Form, top: f64| Over {
        top,
        kind: t::road_primary,
        form,
        pier: false,
    };
    // Two cells a metre: a 2 m slab is 4 cells; a road's 4.5 m clear across a 30% grade
    // is 9.4 cells straight up, so 10, and a staircase's step more.
    let cpm = 2.0;
    assert_eq!(over(Form::Deck, 30.0).occupies(10, cpm), Occupies::Above { slab: [26, 30], pier: None });
    assert_eq!(over(Form::Bore, 10.0).occupies(40, cpm), Occupies::Through { air: [10, 21] });
    // a road raised past a structure is a slab, down to the ground where it is thinner
    assert_eq!(over(Form::Grade, 14.0).occupies(10, cpm), Occupies::Above { slab: [10, 14], pier: None });
    assert_eq!(over(Form::Grade, 20.0).occupies(10, cpm), Occupies::Above { slab: [16, 20], pier: None });
    assert_eq!(over(Form::Grade, 10.0).occupies(14, cpm), Occupies::Through { air: [10, 14] });
    assert_eq!(over(Form::Grade, 10.0).occupies(10, cpm), Occupies::Grade);
}

/// Every segment tried, in order: what the boxes must answer.
fn scan(d: &Decks, frame: Option<Frame>, at: [f64; 2]) -> Vec<Over> {
    d.iter()
        .flat_map(|deck| (0..deck.points.len().saturating_sub(1)).filter_map(move |s| deck.over_segment(frame, s, at)))
        .collect()
}

#[test]
fn the_boxes_answer_what_every_segment_does() {
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut rng = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let frame = Some(Frame { corner: [0.0, 0.0], cell: 2.0 });
    for round in 0..4 {
        let mut b = Builder::default();
        // Lines that wander past the page's edge, long single segments across it,
        // and every width up to the widest.
        for line in 0..60 {
            let n = 2 + (rng() * 30.0) as usize;
            let mut p = [rng() * 600.0 - 44.0, rng() * 600.0 - 44.0];
            let step = if line % 7 == 0 { 400.0 } else { 6.0 };
            let points: Vec<([f64; 2], i16)> = (0..n)
                .map(|_| {
                    p = [p[0] + (rng() - 0.5) * step, p[1] + (rng() - 0.5) * step];
                    (p, (rng() * 200.0) as i16)
                })
                .collect();
            let width = if line % 5 == 0 { u8::MAX } else { 1 + (rng() * 64.0) as u8 };
            b.push(points, width, 3, if line % 3 == 0 { Form::Bore } else { Form::Deck });
        }
        let d = b.finish();
        for k in 0..2000 {
            // A tenth on the edges of the page and of the eighth points a leaf spans.
            let at = if k % 10 == 0 {
                [f64::from(k % 513), (rng() * 64.0).round() * 8.0]
            } else {
                [rng() * 520.0 - 4.0, rng() * 520.0 - 4.0]
            };
            assert_eq!(d.over_point(frame, at).collect::<Vec<_>>(), scan(&d, frame, at), "round {round} at {at:?}");
        }
    }
}
