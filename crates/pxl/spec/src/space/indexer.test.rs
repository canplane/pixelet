// pxl-spec/space/indexer.test.rs

use super::*;
use crate::space::pvf3::{ivec, projection, Pvf3};

#[test]
fn indexer_l3_idx_from_l3_idx_round_trip_on_all_three_axes() {
    let addr = ivec::gather(-1, 2049, 1801);
    let idx = L3_IDX(addr);
    assert_eq!(ivec::repr(idx), "-1:4:3");
    assert_eq!(ivec::repr(FROM_L3_IDX(idx)), "-512:2048:1536");
}

/// The half tracks the span; a fixed half is right at only one tier.
#[test]
fn indexer_span_center_s_half_is_half_of_the_span_at_every_span() {
    let _p = Pvf3::default();
    let addr = projection::project(100.7, 200.2, 33.9).i;
    let _out = Pvf3::default();

    for (shamt, span) in [(0, 1.0), (1, 2.0), (6, 64.0), (10, 1024.0)] {
        let c = SPAN_CENTER(addr, shamt);
        let center = projection::unproject(c.i, c.f);
        let floor = |v: f64| (v / span).floor() * span + span / 2.0;
        assert_eq!(center[0], floor(100.0), "x at span {span}");
        assert_eq!(center[1], floor(200.0), "y at span {span}");
        assert_eq!(center[2], floor(33.0), "z at span {span}");
    }
}

/// A 1u cell's center is 0.5u, in the fraction lane, which the packed-OR helpers would drop.
#[test]
fn indexer_span_center_at_1u_puts_the_half_in_the_fractional_lane() {
    let _out = Pvf3::default();
    let out = SPAN_CENTER(ivec::gather(10, -3, 7), 0);
    assert_eq!(ivec::repr(out.i), "10:-3:7");
    assert!(out.f != 0);
    let u = projection::unproject(out.i, out.f);
    assert_eq!(u, [10.5, -2.5, 7.5]);
}

/// Two points in one cell resolve to the same anchor.
#[test]
fn indexer_every_point_in_a_cell_resolves_to_the_same_center() {
    let (_pa, _pb, _oa, _ob) = (Pvf3::default(), Pvf3::default(), Pvf3::default(), Pvf3::default());
    let a = SPAN_CENTER(projection::project(100.1, 200.9, 33.2).i, 6);
    let b = SPAN_CENTER(projection::project(120.8, 250.4, 60.7).i, 6);
    assert_eq!(a.i, b.i);
    assert_eq!(a.f, b.f);
    let _pc = Pvf3::default();
    assert_eq!(ivec::repr(a.i), ivec::repr(L2_CENTER(projection::project(100.1, 200.9, 33.2).i)));
    assert_eq!(a.f, 0);
}
