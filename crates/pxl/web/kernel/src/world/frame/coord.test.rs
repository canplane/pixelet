// pxl-kernel/world/frame/coord.test.rs
//
// The frame is a translation: the same world read against another origin
// says the same things. Where a point stands over the ground is one height,
// whichever origin the view is drawn against — on the ground, at the eye's
// height and past the Z lane's half, where the lanes alone would wrap.

use super::*;

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
