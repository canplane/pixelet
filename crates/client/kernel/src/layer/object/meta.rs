// pxl-kernel/layer/object/meta.rs

//! What a placed thing carries beyond its value: decor's variant and heading,
//! seeded by where it is placed, its one stable identity.

use pxl_portable::scatter::placed_pose;
use pxl_spec::block::decor::Decor;
use pxl_spec::block::object::{decor_id_from_num, object_value_is_decor};
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::zoom::{index_of, UNIT_ZOOM};

use crate::layer::instance::{Instance, Meta, Source};
use crate::layer::object::store::Entry;

/// The variant and quarter turns placing `decor` with its corner at
/// `corner` gives it (pxl-portable `placed_pose`), so the hover ghost
/// predicts exactly what a placement gets.
pub fn user_decor_pick(corner: &Q3, decor: &Decor) -> (u32, u32) {
    placed_pose(decor.row, index_of(UNIT_ZOOM, corner).map(|v| v as i32))
}

/// A placed decor as one instance, its box's min corner at its entry's; none
/// for anything else placed.
pub fn placed(e: &Entry) -> Option<Instance> {
    if !object_value_is_decor(e.state) {
        return None;
    }
    let decor = Decor::of(decor_id_from_num(e.state));
    let (variant, turn) = user_decor_pick(&e.corner(), decor);
    let (at, meta) = decor.stand(e.cube().min, variant, turn, 1.0);
    Some(Instance {
        id: decor.value(),
        at,
        source: Source::Object(e.id()),
        meta: Meta::Decor(meta),
    })
}
