// pxl-kernel/control/camera/ray.test.rs

//
// The corner the eraser carves at. A carve writes a cell-sized AIR leaf
// (camera/input.rs), because a zero material is a voxel the scan emits nothing
// for whatever its sub-cell bytes say — so the corner it is written at has to
// be the cell's own and not the size tier's. The two differ by up to a tier,
// which is enough to record a bite where nobody took one, and the store is
// saved, so that number outlives the session.

use crate::layer::object::size::OBJECT_SIZES;
use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::pvf3::{fvec, ivec};

use super::*;

/// The voxel spanning [3,4] x [5,6] x [2,3], struck on the face `normal` names
/// at a point deliberately off every tier boundary.
fn struck(normal: V3, hit_point: V3) -> WorldHit {
    WorldHit {
        material_class: MaterialClasses::LAND,
        face_normal: normal,
        hit_point,
        t: 1.0,
        box_min: [3.0, 5.0, 2.0],
        box_max: [4.0, 6.0, 3.0],
    }
}

/// One hit per branch of world_struck_corner: a positive normal, a negative
/// one, and the axes with no normal component at all, which every case has two
/// of.
fn cases() -> [(&'static str, WorldHit); 3] {
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
        let p = world_struck_corner(&hit, U_PER_CELL, 0);
        assert_eq!(lattice(&p), (3, 5, 2), "{name}");
        assert_eq!((fvec::X(p.f), fvec::Y(p.f), fvec::Z(p.f)), (0, 0, 0), "{name}");
    }
}

/// Asking for the cell is not a different answer from asking for a tier — it
/// is the same cell, named without the residue. A branch that rounded instead
/// of flooring, or that lost the epsilon on a face flush with a boundary,
/// would name a neighbor for one of the tiers and not for the others.
#[test]
fn world_struck_corner_at_cell_size_names_the_cell_every_tier_lands_inside() {
    for (name, hit) in cases() {
        let cell = lattice(&world_struck_corner(&hit, U_PER_CELL, 0));
        for size in OBJECT_SIZES {
            assert_eq!(lattice(&world_struck_corner(&hit, size, 0)), cell, "{name} at tier {size}");
        }
    }
}
