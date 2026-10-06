// pxl-spec/space/observation.test.rs

use super::*;
use crate::space::pyramid::TOP_Z;
use crate::space::zoom::units;

#[test]
fn tiled_storage_maps_a_square_to_its_tile_and_ground_to_every_tile_under_it() {
    for zoom in [LEAF_Z, 13, TOP_Z] {
        let span = units(zoom);
        for [x, y] in [[0i64, 0], [-3, 5], [17, -11]] {
            let mut tiles = Vec::new();
            let (x0, y0) = (x as f64 * span, y as f64 * span);
            let square = Aabb {
                min: [x0, y0, -7.0],
                max: [x0 + span, y0 + span, 900.0],
            };
            Tiled.tiles(&Requirement { zoom, region: square }, &mut |p| tiles.push(p));
            assert_eq!(tiles, vec![tile_at(zoom, [x, y])]);
            tiles.clear();
            let wider = Aabb {
                min: [x0 + 0.5 * span, y0, 0.0],
                max: [x0 + 2.0 * span, y0 + 1.5 * span, 0.0],
            };
            Tiled.tiles(&Requirement { zoom, region: wider }, &mut |p| tiles.push(p));
            assert_eq!(tiles.len(), 4, "z{zoom}: {tiles:?}");
        }
    }
}
