// pxl-shared/spec/space/pvf3/core.test.rs

use super::*;
use crate::spec::space::pvf3::{fvec, ivec};

#[test]
fn pvf3_core_ivec_exposes_the_full_signed_24_24_16_lane_widths() {
    let packed = ivec::gather(-(1 << 23), (1 << 23) - 1, -(1 << 15));

    assert_eq!(ivec::X(packed), -(1 << 23));
    assert_eq!(ivec::Y(packed), (1 << 23) - 1);
    assert_eq!(ivec::Z(packed), -(1 << 15));
}

#[test]
fn pvf3_core_fvec_exposes_all_frac_w_bits_in_every_residue_lane() {
    let frac_w = FRAC_W;
    let max = (1i64 << frac_w) - 1;
    let packed = fvec::gather(max, max, max);

    assert_eq!(fvec::X(packed), max);
    assert_eq!(fvec::Y(packed), max);
    assert_eq!(fvec::Z(packed), max);
    assert_eq!(fvec::not(0), (1u64 << (frac_w * 3)) - 1);
}
