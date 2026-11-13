// pxl-core/domain/view/frame/coord.test.rs

use super::*;
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::world::ROUND;
use pxl_spec::space::coord::world::idx_of;
use pxl_spec::space::coord::zoom::UNIT_ZOOM;

/// X is continuous across the seam where it wraps. Seen from an eye on the
/// seam, the points one unit to either side are two units apart, and a
/// position one unit past the seam is the point one unit inside the far side.
#[test]
fn x_continuous_at_seam() {
    crate::on_core(|| {
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
    });
}
