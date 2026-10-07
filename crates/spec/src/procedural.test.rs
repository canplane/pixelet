// pxl-spec/procedural.test.rs

use super::*;

#[test]
fn a_pier_stands_on_the_ground_and_not_where_the_terrain_gives_none() {
    use crate::cell::cube_types::ground;
    assert_eq!(pier_stands([0, 10], ground::grass, [], 0, 1.0), Some([0, 10]));
    assert_eq!(pier_stands([0, 10], sentinel::void, [], 0, 1.0), None);
}

/// The two ways of asking what is scattered agree: a region at the leaves
/// holds exactly what each of its cells holds asked alone, so what a chunk
/// draws and what a ray strikes are one set.
#[test]
fn a_region_scattered_at_the_leaves_is_its_cells_scattered_alone() {
    for (tag, density) in [(0x7ee5_eed7_eec0_de1, 0.04), (0xb0a7_5eed_0002_dec0, 0.002), (0x0ca5_eed0_0003_dec0, 0.3)] {
        for min in [[0, 0], [-37, 91], [1 << 20, -(1 << 20) + 5]] {
            let max = [min[0] + 70, min[1] + 45];
            let mut region = Vec::new();
            scatter(tag, density, LEAF_ZOOM, [min, max], &mut region).unwrap();
            let mut alone = Vec::new();
            for y in min[1]..max[1] {
                for x in min[0]..max[0] {
                    alone.extend(scatter_at(tag, density, [x, y]));
                }
            }
            alone.sort_unstable_by_key(|c| c.cell);
            region.sort_unstable_by_key(|c| c.cell);
            assert_eq!(region, alone, "tag {tag:x} at {min:?}");
            assert!(!region.is_empty() || density < 0.01, "a region this size holds some");
        }
    }
}
