// pxl-render/gpu/ray/ground.test.rs

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

#[test]
fn a_build_that_ran_out_says_of_what() {
    // `STARVED` is the pool's; `CRAMPED` the scratch's.
    assert_eq!(short_of(ways::tree::STARVED), Some(Short::Pool));
    assert_eq!(short_of(ways::tree::CRAMPED), Some(Short::Scratch));
    // A tree built, none for a square with no ways, or past the square's
    // own most, grows neither.
    assert_eq!(short_of(0), None);
    assert_eq!(short_of(1), None);
    assert_eq!(short_of(ways::tree::SQUARE_FAMILIES), None);
    assert_eq!(short_of(ways::tree::NONE), None);
}

#[test]
fn a_scratch_at_its_most_is_as_it_was() {
    // 18.01 MiB, `HANDED` squares of `SCRATCH_WORDS`.
    assert_eq!(
        scratch_bytes(ways::tree::SQUARE_FAMILIES),
        HANDED * ways::tree::SCRATCH_WORDS * 4
    );
}
