// pxl-spec/space/layer/quadrant.test.rs

use super::*;
use crate::space::coord::world::{ROUND, nearest};
use crate::space::coord::zoom::{LEAF_ZOOM, MIN_ZOOM};

/// X goes round the world and Y stops at the Mercator square's edge, and a
/// stored X stands for an eye at its nearest copy.
#[test]
fn point_wrap_x_clamp_y() {
    use crate::space::coord::world::{self, BOUNDS};
    let half = ROUND / 2.0;
    assert_eq!(
        world::point(half + 3.0, BOUNDS.max[1] + 100.0, 0.0),
        world::point(-half + 3.0, BOUNDS.max[1], 0.0)
    );
    assert_eq!(nearest(-half + 3.0, half - 1.0), half + 3.0);
    assert_eq!(nearest(half - 1.0, -half + 3.0), -half - 1.0);
    for zoom in [MIN_ZOOM, LEAF_ZOOM] {
        let n = 1i32 << zoom;
        assert_eq!(
            Quadrant::new(zoom, [n / 2, 0]),
            Quadrant::new(zoom, [-n / 2, 0])
        );
        assert!(!quad_rows(zoom).contains(&(n / 2)));
    }
}

/// A Bundang tile, (11569, 3671): rows count from the south and a first
/// digit is the signs. The bucket's keys are these names.
const BUNDANG: [i32; 2] = [11569, 3671];
const BUNDANG_NAME: &str = "010332102130223";

#[test]
fn known_tile_name() {
    let leaf = Quadrant::new(LEAF_ZOOM, BUNDANG);
    assert_eq!(leaf.key(), BUNDANG_NAME);
    assert_eq!(Quadrant::of_key(BUNDANG_NAME), Some(leaf));
}

/// One prefix is one region at every zoom, and a name's length is its zoom.
#[test]
fn tile_name_prefix() {
    for zoom in MIN_ZOOM..LEAF_ZOOM {
        let coarse = Quadrant::over(zoom, BUNDANG).key();
        assert_eq!(coarse.len(), zoom as usize);
        assert!(BUNDANG_NAME.starts_with(&coarse), "z{zoom} {coarse}");
    }
}

#[test]
fn tile_name_round_trip() {
    let n = 1i32 << LEAF_ZOOM;
    for x in [-n / 2, -12345, -1, 0, 1, 11569, n / 2 - 1] {
        for y in [-n / 2, -7777, -1, 0, 1, 3671, n / 2 - 1] {
            for zoom in MIN_ZOOM..=LEAF_ZOOM {
                let p = Quadrant::over(zoom, [x, y]);
                assert_eq!(Quadrant::of_key(&p.key()), Some(p), "({x}, {y}) at z{zoom}");
            }
        }
    }
}

#[test]
fn bad_tile_name_refused() {
    assert_eq!(Quadrant::of_key(""), None);
    assert_eq!(Quadrant::of_key("4"), None, "a digit is a zoom's two bits");
    assert_eq!(Quadrant::of_key("13211032031200a"), None);
    assert_eq!(
        Quadrant::of_key("-1321103203120"),
        None,
        "the digits carry the sign"
    );
    assert_eq!(
        Quadrant::of_key(&"0".repeat(LEAF_ZOOM as usize + 1)),
        None,
        "nothing is finer than a leaf"
    );
    assert_eq!(Quadrant::of_key("0"), None, "coarser than the top");
}
