// pxl-spec/bake/separation.test.rs

use super::*;
use pxl_portable::ways::{self, Columns, Segment, Stations, BEAM_PILLAR_INTERVAL_M};

use crate::block::cube::{ground, sentinel};
use crate::procedural::Frame;
use crate::space::coord::adapter::epsg3857::u_per_m;
use crate::space::coord::zoom::Zoom;

/// One column of leaf cells, the rule's blocks its leaf cells: its
/// segments, its center, its look and its face.
struct Column {
    segments: Vec<Segment>,
    at: [f32; 2],
    look: u32,
    face: i32,
    stations: Stations,
    per_m: f32,
}

impl Columns for Column {
    fn cell(&self) -> Zoom {
        pxl_portable::LEAF_CELL
    }
    fn per_m(&self) -> f32 {
        self.per_m
    }
    fn count(&self, c: [i32; 2]) -> u32 {
        if c == [0, 0] {
            self.segments.len() as u32
        } else {
            0
        }
    }
    fn segment(&self, _: [i32; 2], k: u32) -> Segment {
        self.segments[k as usize]
    }
    fn center(&self, _: [i32; 2]) -> [f32; 2] {
        self.at
    }
    fn side(&self, _: [i32; 2]) -> f32 {
        1.0
    }
    fn stations(&self, _: [i32; 2]) -> Stations {
        self.stations
    }
    fn look(&self, _: [i32; 2]) -> u32 {
        self.look
    }
    fn face(&self, _: [i32; 2]) -> i32 {
        self.face
    }
    fn joins(&self, _: [i32; 2], k: u32) -> bool {
        self.segments[k as usize].first == 0
    }
}

/// Every segment of `d`.
fn segments(d: &Separation) -> Vec<Segment> {
    (0..d.len()).flat_map(|k| (d.starts[k] as usize..d.starts[k + 1] as usize - 1).map(|p| d.segment(p))).collect()
}

/// A leaf tile's frame at the origin: a u a cell.
const NOWHERE: Frame = Frame { corner: [0.0, 0.0], cell: 1.0 };

fn one(points: &[[f64; 2]], tops: &[i16], width_cells: f64) -> Separation {
    let mut b = Builder::default();
    b.push(points.iter().zip(tops).map(|(&p, &t)| (p, t)), (width_cells * POINT_SCALE) as u8, 3, Form::Bridge, 0);
    b.finish()
}

#[test]
fn a_column_is_under_a_deck_within_its_reach() {
    // Four cells wide: two either side of the line.
    let d = one(&[[10.0, 10.0], [30.0, 10.0]], &[100, 120], 4.0);
    let over: Vec<Crossing> = d.crossing_point(NOWHERE, [20.0, 12.0], 1.0).collect();
    assert_eq!(over.len(), 1);
    // rising a cell a cell, a column a cell wide sweeps half a cell either side of its middle
    assert_eq!((over[0].low, over[0].top), (109.5, 110.5));
    assert_eq!(d.crossing_point(NOWHERE, [20.0, 12.5], 1.0).count(), 0);
}

#[test]
fn a_narrow_line_along_a_diagonal_is_one_band() {
    let d = one(&[[0.0, 0.0], [40.0, 40.0]], &[10, 10], 0.5);
    let mut covered = fnv::FnvHashSet::default();
    d.crossing_grid(NOWHERE, [0.0, 0.0], 1.0, [40, 40], |x, y, _, _| {
        covered.insert((x, y));
    });
    for k in 0..39 {
        assert!(covered.contains(&(k + 1, k)) || covered.contains(&(k, k + 1)), "cells {k} and {} meet only at a corner", k + 1);
    }
}

#[test]
fn a_pier_stands_at_each_world_station_and_its_column_keeps_it_unless_blocked() {
    // A u a cell; a narrow deck's bent is the one cell at its station.
    let frame = NOWHERE;
    let station = f64::from(BEAM_PILLAR_INTERVAL_M) * u_per_m(0.0);
    let d = one(&[[1.0, 3.0], [41.0, 3.0]], &[20, 20], 4.0);
    let piers: Vec<f64> = (0..42).map(|x| f64::from(x) + 0.5).filter(|&x| d.crossing_point(frame, [x, 3.0], 1.0).any(|o| o.pier == 1)).collect();
    let expected: Vec<f64> = (0..42)
        .map(|x| f64::from(x) + 0.5)
        .filter(|&x| x > 1.0 && x < 41.0 && (x - round(x / station) * station).abs() <= 0.5)
        .collect();
    assert!(!expected.is_empty());
    assert_eq!(piers, expected);
    let at = [expected[0], 3.0];
    let over = d.crossing_point(frame, at, 1.0).next().unwrap();
    assert_eq!(ways::occupies(&over, 5, 0.5).pier, [5, 19]);
    // The pier stands on the ground and not where the terrain gives none.
    let column = |segments: Vec<Segment>, look: u8| Column {
        segments,
        at: at.map(|v| v as f32),
        look: u32::from(look),
        face: 5,
        stations: frame.stations(),
        per_m: 0.5,
    };
    let pier = |c: &Column| (5..19).all(|z| ways::block(c, [0, 0, z], false) == ways::PIER);
    let none = |c: &Column| (5..19).all(|z| ways::block(c, [0, 0, z], false) == ways::NOTHING);
    assert!(pier(&column(segments(&d), ground::grass)));
    assert!(none(&column(segments(&d), ground::primary)));
    assert!(none(&column(segments(&d), sentinel::unknown)));
    assert!(none(&column(segments(&d), sentinel::void)));
    // A deck under it blocks it over that deck, which has its own pier.
    let mut under = Builder::default();
    under.push([([0.0, 3.0], 10), ([40.0, 3.0], 10)], 4 * POINT_SCALE as u8, 3, Form::Bridge, 0);
    let under = under.finish();
    let both = column(segments(&d).into_iter().chain(segments(&under)).collect(), ground::grass);
    assert!((10..19).all(|z| ways::block(&both, [0, 0, z], false) == ways::NOTHING));
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
    use crate::block::cube::ground as t;
    let over = |form: Form, top: f32| Crossing {
        low: top,
        top,
        kind: u32::from(t::primary),
        form: u32::from(form.byte()),
        pier: 0,
        clear: 3,
    };
    let no = [0, 0];
    let on = |c: Crossing, ground: i32, cpm: f32| ways::occupies(&c, ground, cpm);
    let at = |what: Occupy, span: [i32; 2]| Occupies { what, span, pier: [0, 0] };
    // Two cells a metre: a 2 m slab is 4 cells; a bore is clear its line's cells over its surface.
    let cpm = 2.0;
    assert_eq!(on(over(Form::Bridge, 30.0), 10, cpm), at(Occupy::Over, [26, 30]));
    assert_eq!(on(over(Form::Tunnel, 10.0), 40, cpm), at(Occupy::Under, [10, 13]));
    // a way on the ground moves the ground: filled up to it, cut down to it
    assert_eq!(on(over(Form::Approach, 14.0), 10, cpm), at(Occupy::Over, [10, 14]));
    assert_eq!(on(over(Form::Approach, 20.0), 10, cpm), at(Occupy::Over, [10, 20]));
    assert_eq!(on(over(Form::Approach, 10.0), 14, cpm), at(Occupy::Under, [10, 14]));
    assert_eq!(on(over(Form::Approach, 10.0), 10, cpm), at(Occupy::Grade, no));
    // a surface falling 8 cells across a column is solid over all of it, and a bore clear over all of it
    let steep = Crossing {
        low: 22.0,
        ..over(Form::Bridge, 30.0)
    };
    assert_eq!(on(steep, 10, cpm), at(Occupy::Over, [18, 30]));
    let steep = Crossing {
        low: 10.0,
        ..over(Form::Tunnel, 18.0)
    };
    assert_eq!(on(steep, 40, cpm), at(Occupy::Under, [10, 21]));
}
