// pxl-kernel/domain/control/interaction/pick.test.rs

// A carve writes a cell-sized AIR leaf, so its corner must be the cell's,
// not the size tier's; the store is saved, so a wrong bite outlives the
// session.

use pxl_spec::bake::constant::{OBJECT_COARSEST, OBJECT_FINEST};
use pxl_spec::space::coord::q3::{FRAC_W, Q3};
use pxl_spec::space::coord::zoom::{idx_of, UNIT_ZOOM};
use pxl_spec::space::layer::lattice::GROUND;

use super::*;

/// Struck at a point off every tier boundary.
fn struck(normal: V3, point: V3) -> Hit {
    Hit {
        t: 1.0,
        point,
        normal,
        cube: Aabb {
            min: [3.0, 5.0, 2.0],
            max: [4.0, 6.0, 3.0],
        },
        struck: Struck::Ground,
    }
}

fn cases() -> [(&'static str, Hit); 3] {
    [
        ("top", struck([0.0, 0.0, 1.0], [3.62, 5.37, 3.0])),
        ("+x wall", struck([1.0, 0.0, 0.0], [4.0, 5.37, 2.44])),
        ("-y wall", struck([0.0, -1.0, 0.0], [3.62, 5.0, 2.44])),
    ]
}

fn lattice(p: &Q3) -> (i32, i32, i32) {
    let [x, y, z] = idx_of(UNIT_ZOOM, p);
    (x, y, z)
}

#[test]
fn struck_corner_lands_on_cell() {
    for (name, hit) in cases() {
        let p = world_struck_corner(&hit, units(GROUND.cell()));
        assert_eq!(lattice(&p), (3, 5, 2), "{name}");
        assert_eq!(p.map(|v| v.rem_euclid(1 << FRAC_W)), [0, 0, 0], "{name}");
    }
}

#[test]
fn struck_corner_names_cell() {
    for (name, hit) in cases() {
        let cell = lattice(&world_struck_corner(&hit, units(GROUND.cell())));
        for size in (OBJECT_COARSEST..=OBJECT_FINEST).map(units) {
            assert_eq!(
                lattice(&world_struck_corner(&hit, size)),
                cell,
                "{name} at tier {size}"
            );
        }
    }
}
