// pxl-render/gpu/renderer.test.rs

use super::*;

#[test]
fn a_build_that_ran_out_is_judged_by_the_pool_it_was_built_in() {
    // Built in the pool as it is, below its most: it grows, built again.
    assert_eq!(ran_out(16, 16, 64), RanOut::Grow);
    // Built in a pool since grown, landing a frame late: built again, the
    // pool grown no further, even at its most.
    assert_eq!(ran_out(16, 32, 64), RanOut::Again);
    assert_eq!(ran_out(32, 64, 64), RanOut::Again);
    // The pool at its most ran out: refused.
    assert_eq!(ran_out(64, 64, 64), RanOut::Refused);
}
