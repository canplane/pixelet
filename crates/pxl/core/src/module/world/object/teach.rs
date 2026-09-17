// pxl_core/module/world/object/teach.rs

//! What the painter is taught about the objects' states
//! (crates/pxl/core/src/render/layer/object/adapter.rs): every placeable cube's
//! material, AIR, and the decor kinds. Once, before any page is told.

use pxl_shared::spec::cell::{cube_types, decor_ids, decor_num_id, placeable_cube_num_id, placeable_cubes};

use crate::module::world::object::material::resolve_solid_material;
use crate::render::layer::object::adapter::Kind;
use crate::render::layer::pipeline::volume::{pack_material, NO_LAYER};
use crate::sys::kernel::kernel;

/// Teaches the painter every state the catalog can put in a leaf: each
/// placeable cube's material, AIR, and the decor kinds, which paint nothing.
pub fn teach_objects() {
    let painter = &mut kernel().painter;
    for &cube in placeable_cubes.iter() {
        let m = resolve_solid_material(cube);
        painter.teach(placeable_cube_num_id(cube), Kind::Solid, pack_material(&m), m.layer_idx as u8);
    }
    painter.teach(cube_types::common::air as u32, Kind::Air, 0, NO_LAYER as u8);
    for &id in decor_ids.iter() {
        painter.teach(decor_num_id(id), Kind::Decor, 0, NO_LAYER as u8);
    }
}
