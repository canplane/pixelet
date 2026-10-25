// pxl-spec/bake/separation.test.rs

use super::*;
use pxl_spec_core::ways;

use crate::procedural::Frame;

/// A leaf tile's frame at the origin: a u a cell.
const NOWHERE: Frame = Frame { corner: [0.0, 0.0], cell: 1.0 };

fn one(points: &[[f64; 2]], tops: &[i16], width_cells: f64) -> Separation {
    let mut b = Builder::default();
    b.push(points.iter().zip(tops).map(|(&p, &t)| (p, t, 0)), (width_cells * POINT_SCALE) as u8, 3, Form::Bridge, 0);
    b.finish()
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

/// A parent's neighbourhood with `d` its first child's lines and no other.
fn first_child(d: &Separation) -> [Option<&Separation>; 16] {
    let mut around = [None; 16];
    around[crate::space::layer::tile::neighbor_index(0, 0)] = Some(d);
    around
}

#[test]
fn a_line_is_kept_up_the_pyramid_by_its_width_alone() {
    // Two cells wide: one cell a zoom coarser, kept at its own width.
    let wide = one(&[[0.0, 0.0], [100.0, 0.0]], &[50, 50], 2.0);
    let up = fold_separation(first_child(&wide), 14);
    assert_eq!(up.len(), 1);
    assert_eq!(up.line(0).width_cells(), 1.0);
    // Under a cell wide: under half the parent's, and gone, whatever it is.
    let narrow = one(&[[0.0, 0.0], [100.0, 0.0]], &[50, 50], 0.75);
    assert_eq!(fold_separation(first_child(&narrow), 14).len(), 0);
}

#[test]
fn each_form_is_drawn_from_its_surface() {
    use crate::asset::cube::ground as t;
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
