// @pxl/shared/spec/space/indexer.test.ts

use super::*;
use crate::spec::space::pvf3::{ivec, projection, PVF3};

#[test]
fn the_masks_are_what_fill_makes() {
    assert_eq!(L2_MASK, ivec::fill(field_mask64(L2_SHAMT) as i64));
    assert_eq!(L3_MASK, ivec::fill(field_mask64(L3_SHAMT) as i64));
    assert_eq!(L4_MASK, ivec::fill(field_mask64(L4_SHAMT) as i64));
}

#[test]
fn indexer_l2_is_a_signed_64_cubed_render_chunk() {
    let addr = ivec::gather(-1, 95, -65);
    assert_eq!(L2_RANGE, 64);
    assert_eq!(ivec::repr(L2_BASE(addr)), "-64:64:-128");
}

#[test]
fn indexer_l3_is_a_signed_1024_cubed_addressing_level_same_shape_as_l2() {
    // Z=1801 (not just <1024, like L2's own test addr) so a nonzero Z
    // BASE/CENTER/LIMIT actually proves Z participates, instead of a Z<1024
    // value that floors to 0 either way and would silently pass even with the
    // old XY-only behavior.
    let addr = ivec::gather(-1, 2049, 1801);
    assert_eq!(L3_RANGE, 1024);
    assert_eq!(L2_SPAN_IN_L3, 16);
    // Z participates like X/Y — no XY-only narrowing at this level (that's
    // terrain_page.ts's own policy, tested separately).
    assert_eq!(ivec::repr(L3_BASE(addr)), "-1024:2048:1024");
    assert_eq!(ivec::repr(L3_CENTER(addr)), "-512:2560:1536");
    assert_eq!(ivec::repr(L3_LIMIT(addr)), "0:3072:2048");
}

#[test]
fn indexer_l3_idx_from_l3_idx_round_trip_on_all_three_axes() {
    let addr = ivec::gather(-1, 2049, 1801);
    let idx = L3_IDX(addr);
    assert_eq!(ivec::repr(idx), "-1:2:1");
    assert_eq!(ivec::repr(FROM_L3_IDX(idx)), "-1024:2048:1024");
}

/// The half has to track the span. A marker that snaps to a cell centre with
/// a fixed half is right at exactly one tier and wrong at every other, which
/// is what happens the moment the thing being marked changes LOD.
#[test]
fn indexer_span_center_s_half_is_half_of_the_span_at_every_span() {
    let mut p = PVF3::default();
    let addr = projection::project(&mut p, 100.7, 200.2, 33.9).i;
    let mut out = PVF3::default();

    for (shamt, span) in [(0, 1.0), (1, 2.0), (6, 64.0), (10, 1024.0)] {
        let c = *SPAN_CENTER(&mut out, addr, shamt);
        let mut centre = [0.0; 3];
        projection::unproject(&mut centre, c.i, c.f);
        let floor = |v: f64| (v / span).floor() * span + span / 2.0;
        assert_eq!(centre[0], floor(100.0), "x at span {span}");
        assert_eq!(centre[1], floor(200.0), "y at span {span}");
        assert_eq!(centre[2], floor(33.0), "z at span {span}");
    }
}

/// A 1u cell's centre is 0.5u, which is not an integer — so it lands in the
/// fractional lane, and the packed-OR the named CENTER helpers use would drop
/// it. This is the case that makes the parameterised form necessary.
#[test]
fn indexer_span_center_at_1u_puts_the_half_in_the_fractional_lane() {
    let mut out = PVF3::default();
    let out = *SPAN_CENTER(&mut out, ivec::gather(10, -3, 7), 0);
    assert_eq!(ivec::repr(out.i), "10:-3:7");
    assert!(out.f != 0);
    let mut u = [0.0; 3];
    projection::unproject(&mut u, out.i, out.f);
    assert_eq!(u, [10.5, -2.5, 7.5]);
}

/// The stability the whole thing rests on: where a badge sits must depend on
/// the cell, never on which member of it was asked. Two points in one cell
/// have to resolve to the same anchor, or the badge moves when the camera
/// does.
#[test]
fn indexer_every_point_in_a_cell_resolves_to_the_same_centre() {
    let (mut pa, mut pb, mut oa, mut ob) = (PVF3::default(), PVF3::default(), PVF3::default(), PVF3::default());
    let a = *SPAN_CENTER(&mut oa, projection::project(&mut pa, 100.1, 200.9, 33.2).i, 6);
    let b = *SPAN_CENTER(&mut ob, projection::project(&mut pb, 120.8, 250.4, 60.7).i, 6);
    assert_eq!(a.i, b.i);
    assert_eq!(a.f, b.f);
    // and it agrees with the named helper at the level it names
    let mut pc = PVF3::default();
    assert_eq!(ivec::repr(a.i), ivec::repr(L2_CENTER(projection::project(&mut pc, 100.1, 200.9, 33.2).i)));
    assert_eq!(a.f, 0);
}
