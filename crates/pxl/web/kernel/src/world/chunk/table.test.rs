// pxl-kernel/world/chunk/table.test.rs

use pxl_spec::space::hierarchy::Scale;

use super::*;

/// An object's cube and the ground's are one size wherever both have data:
/// only nearer than a z16 cell's reach do a z16 chunk's objects go finer,
/// down to their finest tier, while the ground stays at its atom.
#[test]
fn objects_draw_the_grounds_cube_and_go_finer_only_under_z16() {
    let z16 = GROUND.scale(0);
    for shamt in -6..=8 {
        let cube = Scale::of(shamt);
        let (ground, objects) = (drawn_cube(cube, 0, true, false), drawn_cube(cube, 0, true, true));
        if cube >= z16 {
            assert_eq!(objects, ground, "{cube:?}");
        } else {
            assert_eq!(ground, z16, "{cube:?}");
            assert_eq!(objects, cube.max(OBJECT_FINEST), "{cube:?}");
        }
    }
}
