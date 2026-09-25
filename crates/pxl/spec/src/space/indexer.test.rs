// pxl-spec/space/indexer.test.rs

use super::*;
use crate::space::pvf3::{ivec, projection, Pvf3};

#[test]
fn the_masks_are_what_fill_makes() {
    assert_eq!(L2_MASK, ivec::fill(field_mask64(L2.shamt() as u32, 0) as i64));
    assert_eq!(L3_MASK, ivec::fill(field_mask64(L3.shamt() as u32, 0) as i64));
    assert_eq!(L4_MASK, ivec::fill(field_mask64(L4.shamt() as u32, 0) as i64));
}

#[test]
fn indexer_l2_is_a_signed_64_cubed_render_chunk() {
    let addr = ivec::gather(-1, 95, -65);
    assert_eq!(L2_RANGE, 64);
    assert_eq!(ivec::repr(L2_BASE(addr)), "-64:64:-128");
}

#[test]
fn indexer_l3_is_a_signed_512_cubed_addressing_level_same_shape_as_l2() {
    // Z=1801, past a whole span, so a nonzero Z BASE/CENTER/LIMIT actually
    // proves Z participates, instead of a Z inside the first span that floors
    // to 0 either way and would silently pass even with the old XY-only
    // behavior.
    let addr = ivec::gather(-1, 2049, 1801);
    assert_eq!(L3_RANGE, 512);
    assert_eq!(L2_SPAN_IN_L3, 8);
    // Z participates like X/Y — no XY-only narrowing at this level, which is
    // the terrain page's own policy and tested with it.
    assert_eq!(ivec::repr(L3_BASE(addr)), "-512:2048:1536");
    assert_eq!(ivec::repr(L3_CENTER(addr)), "-256:2304:1792");
    assert_eq!(ivec::repr(L3_LIMIT(addr)), "0:2560:2048");
}

#[test]
fn indexer_l3_idx_from_l3_idx_round_trip_on_all_three_axes() {
    let addr = ivec::gather(-1, 2049, 1801);
    let idx = L3_IDX(addr);
    assert_eq!(ivec::repr(idx), "-1:4:3");
    assert_eq!(ivec::repr(FROM_L3_IDX(idx)), "-512:2048:1536");
}

/// The half has to track the span. A marker that snaps to a cell centre with
/// a fixed half is right at exactly one tier and wrong at every other, which
/// is what happens the moment the thing being marked changes LOD.
#[test]
fn indexer_span_center_s_half_is_half_of_the_span_at_every_span() {
    let mut p = Pvf3::default();
    let addr = projection::project(&mut p, 100.7, 200.2, 33.9).i;
    let mut out = Pvf3::default();

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
    let mut out = Pvf3::default();
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
    let (mut pa, mut pb, mut oa, mut ob) = (Pvf3::default(), Pvf3::default(), Pvf3::default(), Pvf3::default());
    let a = *SPAN_CENTER(&mut oa, projection::project(&mut pa, 100.1, 200.9, 33.2).i, 6);
    let b = *SPAN_CENTER(&mut ob, projection::project(&mut pb, 120.8, 250.4, 60.7).i, 6);
    assert_eq!(a.i, b.i);
    assert_eq!(a.f, b.f);
    // and it agrees with the named helper at the level it names
    let mut pc = Pvf3::default();
    assert_eq!(ivec::repr(a.i), ivec::repr(L2_CENTER(projection::project(&mut pc, 100.1, 200.9, 33.2).i)));
    assert_eq!(a.f, 0);
}

/// What the TypeScript computed, recorded: the generator that wrote it
/// is gone and the vector stands as it is (xpute-core/golden.rs).
const GOLDEN: &str = include_str!("../../golden/spec/space/indexer.tsv");

#[test]
fn indexer_computes_what_the_record_holds() {
    let v = xpute_core::golden::Golden::load(GOLDEN);
    assert_eq!(L2_RANGE, v.u64("constants.L2_RANGE"));
    assert_eq!(L3_RANGE, v.u64("constants.L3_RANGE"));
    assert_eq!(L4_RANGE, v.u64("constants.L4_RANGE"));
    assert_eq!(L2_SPAN_IN_L3, v.u64("constants.L2_SPAN_IN_L3"));
    assert_eq!(L2_SPAN_IN_L4, v.u64("constants.L2_SPAN_IN_L4"));
    assert_eq!(L3_SPAN_IN_L4, v.u64("constants.L3_SPAN_IN_L4"));
    assert_eq!(L2_MASK, v.u64("constants.L2_MASK"));
    assert_eq!(L3_MASK, v.u64("constants.L3_MASK"));
    assert_eq!(L4_MASK, v.u64("constants.L4_MASK"));
    type Step = (&'static str, fn(IVec) -> IVec);
    let fns: [Step; 21] = [
        ("L2_BASE", L2_BASE),
        ("L3_BASE", L3_BASE),
        ("L4_BASE", L4_BASE),
        ("L2_CENTER", L2_CENTER),
        ("L3_CENTER", L3_CENTER),
        ("L4_CENTER", L4_CENTER),
        ("L2_LIMIT", L2_LIMIT),
        ("L3_LIMIT", L3_LIMIT),
        ("L4_LIMIT", L4_LIMIT),
        ("L1_IDX_IN_L2", L1_IDX_IN_L2),
        ("L1_IDX_IN_L3", L1_IDX_IN_L3),
        ("L2_IDX_IN_L3", L2_IDX_IN_L3),
        ("L1_IDX_IN_L4", L1_IDX_IN_L4),
        ("L2_IDX_IN_L4", L2_IDX_IN_L4),
        ("L3_IDX_IN_L4", L3_IDX_IN_L4),
        ("L2_IDX", L2_IDX),
        ("L3_IDX", L3_IDX),
        ("L4_IDX", L4_IDX),
        ("FROM_L2_IDX", FROM_L2_IDX),
        ("FROM_L3_IDX", FROM_L3_IDX),
        ("FROM_L4_IDX", FROM_L4_IDX),
    ];
    v.each("addrs", |k| {
        let a = v.u64(&format!("{k}.a"));
        for (name, f) in fns {
            assert_eq!(f(a), v.u64(&format!("{k}.{name}")), "{k}.{name}");
        }
        for (j, shamt) in [0, 1, 3, 6, 10, 12].into_iter().enumerate() {
            let mut out = Pvf3::default();
            let c = *SPAN_CENTER(&mut out, a, shamt);
            assert_eq!((c.i, c.f), (v.u64(&format!("{k}.span.{j}.i")), v.u32(&format!("{k}.span.{j}.f"))), "{k}.SPAN_CENTER({shamt})");
        }
    });
}
