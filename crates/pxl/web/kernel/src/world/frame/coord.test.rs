// pxl-kernel/world/frame/coord.test.rs
//
// The frame is a translation: the same world read against another origin
// says the same things. Where a point stands over the ground is one height,
// whichever origin the view is drawn against — on the ground, at the eye's
// height and past the Z lane's half, where the lanes alone would wrap.

use super::*;
use pxl_spec::space::pvf3::pvf3;

#[test]
fn a_height_over_the_ground_is_the_same_from_every_origin() {
    let mut point = Pvf3::default();
    projection::project(&mut point, 250_000.0, -90_000.0, 30.25);
    let ground = 12.5;
    for origin_z in [0, 4096, 28_672, 32_768, -4096] {
        let origin = ivec::gather(249_856, -94_208, origin_z);
        let over = canonical_to_local(&point, origin)[2] - local_height(ground, origin);
        assert_eq!(over, 30.25 - ground, "origin at z {origin_z}: {over} over the ground");
    }
}

/// A canonical point put through local floats and taken back nearest is the
/// point it was: off either side of a whole unit, below zero, and against
/// origins near it, far from it and past the Z lane's half. Taken back by
/// the cell it lies in, a point the floats leave a hair under itself would
/// fall a 256th of a unit, which is what the nearest taking is for.
#[test]
fn a_canonical_point_comes_back_through_floats() {
    let one = pxl_spec::space::pvf3::core::ONE as i64;
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = |span: i64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % (2 * span as u64 + 1)) as i64 - span
    };
    let origins = [
        ivec::gather(0, 0, 0),
        ivec::gather(249_856, -94_208, 4096),
        ivec::gather(-3, 7, 32_768),
        ivec::gather(1 << 20, -(1 << 20), -4096),
    ];
    for n in 0..20_000 {
        // Whole units near the origin and far from it, and a fraction with
        // the ones a hair off a whole unit drawn often.
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
        let mut p = Pvf3::default();
        pvf3::gather(&mut p, comps[0], comps[1], comps[2]);
        for &origin in &origins {
            let local = canonical_to_local(&p, origin);
            let mut back = Pvf3::default();
            local_world_to_canonical_nearest(&mut back, &local, origin);
            assert_eq!(back, p, "{n}: {comps:?} against {origin}");
        }
    }
}
