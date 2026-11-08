// pxl-core/domain/view/frame/coord.test.rs

use super::*;
use pxl_spec::space::coord::q3;
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::world::ROUND;
use pxl_spec::space::coord::zoom::{idx_of, UNIT_ZOOM};
use xpute_kit::math::rng::SplitMix64;

/// A point that floats leave a hair under itself would fall a 256th of a
/// unit on each round trip if it were taken back by the cell it lies in. The
/// round trip must return the same point on whichever lap it is read.
#[test]
fn canonical_round_trip() {
    let one = q3::ONE;
    let mut rng = SplitMix64::new(0x9e37_79b9_7f4a_7c15);
    let mut next = |span: i64| (u64::from(rng.next_u32()) % (2 * span as u64 + 1)) as i64 - span;
    let nears = [0.0, 249_856.0, -3.0 * ROUND, 2.5 * ROUND, -(1 << 23) as f64];
    for n in 0..20_000 {
        // The components are whole units near and far from the origin, and
        // fractions a hair off a whole unit.
        let fraction = |r: i64| match r % 4 {
            0 => 0,
            1 => 1,
            2 => one - 1,
            _ => r.rem_euclid(one),
        };
        let unit = |r: i64| if r % 2 == 0 { r % 8 } else { r };
        let comps: [i64; 3] = std::array::from_fn(|_| {
            let (u, f) = (unit(next(1 << 21)), fraction(next(1 << 21)));
            u * one + f
        });
        let p = world::at(comps[0], comps[1], comps[2]);
        for near in nears {
            let w = canonical_to_world(&p, near);
            assert!(
                (w[0] - near).abs() <= ROUND / 2.0,
                "{n}: {comps:?} unwrapped off {near}"
            );
            assert_eq!(
                world_to_canonical_nearest(&w),
                p,
                "{n}: {comps:?} near {near}"
            );
        }
    }
}

/// X is continuous across the seam where it wraps. Seen from an eye on the
/// seam, the points one unit to either side are two units apart, and a
/// position one unit past the seam is the point one unit inside the far side.
#[test]
fn x_continuous_at_seam() {
    let east = world::point(ROUND / 2.0 - 1.0, 0.0, 0.0);
    let west = world::point(-ROUND / 2.0 + 1.0, 0.0, 0.0);
    let eye = ROUND / 2.0;
    assert_eq!(
        canonical_to_world(&west, eye)[0] - canonical_to_world(&east, eye)[0],
        2.0
    );
    assert_eq!(
        idx_of(UNIT_ZOOM, &world_to_canonical(&[eye + 1.0, 0.0, 0.0]))[0],
        idx_of(UNIT_ZOOM, &west)[0]
    );
}
