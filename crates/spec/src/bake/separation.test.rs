// pxl-spec/bake/separation.test.rs

use super::*;
use crate::cell::cube_types::ground;
use crate::procedural::{pier_stands, Frame, BEAM_PILLAR_INTERVAL_M};
use crate::space::coord::adapter::epsg3857::u_per_m;

fn one(points: &[[f64; 2]], tops: &[i16], width_cells: f64) -> Separation {
    let mut b = Builder::default();
    b.push(points.iter().zip(tops).map(|(&p, &t)| (p, t)), (width_cells * POINT_SCALE) as u8, 3, Form::Bridge, 0);
    b.finish()
}

#[test]
fn a_column_is_under_a_deck_within_its_reach() {
    // Four cells wide: two either side of the line.
    let d = one(&[[10.0, 10.0], [30.0, 10.0]], &[100, 120], 4.0);
    let over: Vec<Crossing> = d.crossing_point(None, [20.0, 12.0], 1.0).collect();
    assert_eq!(over.len(), 1);
    // rising a cell a cell, a column a cell wide sweeps half a cell either side of its middle
    assert_eq!((over[0].low, over[0].top), (109.5, 110.5));
    assert_eq!(d.crossing_point(None, [20.0, 12.5], 1.0).count(), 0);
}

#[test]
fn a_narrow_line_along_a_diagonal_is_one_band() {
    let d = one(&[[0.0, 0.0], [40.0, 40.0]], &[10, 10], 0.5);
    let mut covered = std::collections::HashSet::new();
    d.crossing_grid(None, [0.0, 0.0], 1.0, [40, 40], |x, y, _, _| {
        covered.insert((x, y));
    });
    for k in 0..39 {
        assert!(covered.contains(&(k + 1, k)) || covered.contains(&(k, k + 1)), "cells {k} and {} meet only at a corner", k + 1);
    }
}

#[test]
fn a_pier_stands_at_each_world_station_and_its_column_keeps_it_unless_blocked() {
    // Two u a cell; a narrow deck's bent is the one cell at its station.
    let frame = Some(Frame { corner: [0.0, 0.0], cell: 2.0 });
    let station = BEAM_PILLAR_INTERVAL_M * u_per_m(0.0) / 2.0;
    let d = one(&[[1.0, 3.0], [41.0, 3.0]], &[20, 20], 4.0);
    let piers: Vec<f64> = (0..42).map(|x| f64::from(x) + 0.5).filter(|&x| d.crossing_point(frame, [x, 3.0], 1.0).any(|o| o.pier)).collect();
    let expected: Vec<f64> = (0..42)
        .map(|x| f64::from(x) + 0.5)
        .filter(|&x| x > 1.0 && x < 41.0 && (x - round(x / station) * station).abs() <= 0.5)
        .collect();
    assert!(!expected.is_empty());
    assert_eq!(piers, expected);
    let at = [expected[0], 3.0];
    let over = d.crossing_point(frame, at, 1.0).next().unwrap();
    let Occupies::Over { pier: Some(pier), .. } = over.occupies(5, 0.5) else {
        panic!("no pier over ground at 5")
    };
    assert_eq!(pier, [5, 19]);
    let (land, road) = (ground::grass, ground::primary);
    assert_eq!(pier_stands(pier, land, d.crossing_point(frame, at, 1.0), 5, 0.5), Some(pier));
    assert_eq!(pier_stands(pier, road, d.crossing_point(frame, at, 1.0), 5, 0.5), None);
    let mut under = Builder::default();
    under.push([([0.0, 3.0], 10), ([40.0, 3.0], 10)], 4 * POINT_SCALE as u8, 3, Form::Bridge, 0);
    let under = under.finish();
    let column = d.crossing_point(frame, at, 1.0).chain(under.crossing_point(frame, at, 1.0));
    assert_eq!(pier_stands(pier, land, column, 5, 0.5), None);
}

#[test]
fn a_line_is_kept_up_the_pyramid_by_its_width_alone() {
    let empty = Separation::default();
    // Two cells wide: one cell a zoom coarser, kept at its own width.
    let wide = one(&[[0.0, 0.0], [100.0, 0.0]], &[50, 50], 2.0);
    let up = fold_separation([&wide, &empty, &empty, &empty], 15);
    assert_eq!(up.len(), 1);
    assert_eq!(up.line(0).width_cells(), 1.0);
    // Under a cell wide: under half the parent's, and gone, whatever it is.
    let narrow = one(&[[0.0, 0.0], [100.0, 0.0]], &[50, 50], 0.75);
    assert_eq!(fold_separation([&narrow, &empty, &empty, &empty], 15).len(), 0);
}

#[test]
fn a_straight_flat_line_folds_to_its_ends() {
    let d = one(&[[0.0, 0.0], [10.0, 0.0], [20.0, 0.0], [30.0, 0.0]], &[40, 40, 40, 40], 4.0);
    let empty = Separation::default();
    let up = fold_separation([&d, &empty, &empty, &empty], 15);
    assert_eq!(up.line(0).points.len(), 2);
    assert_eq!(up.line(0).point(1), [15.0, 0.0]);
}

#[test]
fn each_form_is_drawn_from_its_surface() {
    use crate::cell::cube_types::ground as t;
    let over = |form: Form, top: f64| Crossing {
        low: top,
        top,
        kind: t::primary,
        form,
        pier: false,
        clear: 3,
    };
    // Two cells a metre: a 2 m slab is 4 cells; a bore is clear its line's cells over its surface.
    let cpm = 2.0;
    assert_eq!(over(Form::Bridge, 30.0).occupies(10, cpm), Occupies::Over { slab: [26, 30], pier: None });
    assert_eq!(over(Form::Tunnel, 10.0).occupies(40, cpm), Occupies::Under { span: [10, 13] });
    // a way on the ground moves the ground: filled up to it, cut down to it
    assert_eq!(over(Form::Approach, 14.0).occupies(10, cpm), Occupies::Over { slab: [10, 14], pier: None });
    assert_eq!(over(Form::Approach, 20.0).occupies(10, cpm), Occupies::Over { slab: [10, 20], pier: None });
    assert_eq!(over(Form::Approach, 10.0).occupies(14, cpm), Occupies::Under { span: [10, 14] });
    assert_eq!(over(Form::Approach, 10.0).occupies(10, cpm), Occupies::Grade);
    // a surface falling 8 cells across a column is solid over all of it, and a bore clear over all of it
    let steep = Crossing {
        low: 22.0,
        ..over(Form::Bridge, 30.0)
    };
    assert_eq!(steep.occupies(10, cpm), Occupies::Over { slab: [18, 30], pier: None });
    let steep = Crossing {
        low: 10.0,
        ..over(Form::Tunnel, 18.0)
    };
    assert_eq!(steep.occupies(40, cpm), Occupies::Under { span: [10, 21] });
}
