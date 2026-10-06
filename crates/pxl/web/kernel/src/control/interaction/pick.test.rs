// pxl-kernel/control/interaction/pick.test.rs

// A carve writes a cell-sized AIR leaf, so its corner must be the cell's,
// not the size tier's; the store is saved, so a wrong bite outlives the
// session.

use crate::layer::object::size::OBJECT_SIZES;
use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::pvf3::{fvec, ivec};

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

fn lattice(p: &Pvf3) -> (i64, i64, i64) {
    (ivec::X(p.i), ivec::Y(p.i), ivec::Z(p.i))
}

#[test]
fn world_struck_corner_at_cell_size_lands_on_the_struck_cell_with_no_sub_cell_residue() {
    for (name, hit) in cases() {
        let p = world_struck_corner(&hit, U_PER_CELL);
        assert_eq!(lattice(&p), (3, 5, 2), "{name}");
        assert_eq!((fvec::X(p.f), fvec::Y(p.f), fvec::Z(p.f)), (0, 0, 0), "{name}");
    }
}

#[test]
fn world_struck_corner_at_cell_size_names_the_cell_every_tier_lands_inside() {
    for (name, hit) in cases() {
        let cell = lattice(&world_struck_corner(&hit, U_PER_CELL));
        for size in OBJECT_SIZES {
            assert_eq!(lattice(&world_struck_corner(&hit, size)), cell, "{name} at tier {size}");
        }
    }
}
