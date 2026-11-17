// pxl-core/domain/scene/marker/place.test.rs

use super::*;
use pxl_spec::space::coord::zoom::LEAF_ZOOM;

/// A place read from the brick it stands in, on the lap of X `lap` rounds
/// from the world's own.
fn place_in(brick_zoom: Zoom, lap: i32) -> Place {
    let src = Quadrant::new(LEAF_ZOOM, [3, 5]);
    let ground = stands(src, [7, 9]);
    let [x, y, _] = idx_of(brick_zoom, &ground.mid());
    Place {
        brick: (brick_zoom, [x + lap * (1 << brick_zoom), y]),
        kind: 0,
        first_zoom: 0,
        name: None,
        ground,
        finest: 0,
        top: None,
    }
}

/// A place's brick at any zoom lies over or under its own brick, on its
/// brick's lap of X: at its own zoom it is its brick, a finer one's ancestor
/// there is its brick, and a coarser one is its brick's ancestor.
#[test]
fn a_place_s_brick_at_a_zoom_is_in_line_with_its_own() {
    for lap in [-1, 0, 1] {
        let p = place_in(12, lap);
        let (zoom, column) = p.brick;
        assert_eq!(p.brick_at(zoom), p.brick);
        for z in zoom + 1..=zoom + 4 {
            let (_, finer) = p.brick_at(z);
            assert_eq!(ancestor_of(finer, z - zoom), column, "lap {lap}, zoom {z}");
        }
        for z in zoom - 4..zoom {
            let coarser = (z, ancestor_of(column, zoom - z));
            assert_eq!(p.brick_at(z), coarser, "lap {lap}, zoom {z}");
        }
    }
}
