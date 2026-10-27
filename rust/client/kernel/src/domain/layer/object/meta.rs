// pxl-kernel/domain/layer/object/meta.rs

//! What a placed thing carries beyond its value: decor's variant and heading,
//! seeded by where it is placed, its one stable identity.

use pxl_spec::asset::decor::Decor;
use pxl_spec::asset::decor_id_from_num;
use pxl_spec::object::is_decor;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::zoom::{index_of, UNIT_ZOOM};
use pxl_spec_core::scatter::placed_pose;

use crate::domain::layer::instance::{Instance, Source};
use crate::domain::layer::object::store::Entry;

/// The variant and quarter turns placing `decor` with its corner at
/// `corner` gives it (pxl-spec-core `placed_pose`), so the hover ghost
/// predicts exactly what a placement gets.
pub fn user_decor_pick(corner: &Q3, decor: &Decor) -> (u32, u32) {
    placed_pose(decor.row, index_of(UNIT_ZOOM, corner))
}

/// A placed decor as one instance, its box's min corner at its entry's; none
/// for anything else placed.
pub fn placed(e: &Entry) -> Option<Instance> {
    if !is_decor(e.state) {
        return None;
    }
    let decor = Decor::of(decor_id_from_num(e.state));
    let (variant, turn) = user_decor_pick(&e.corner(), decor);
    let (at, meta) = decor.stand(e.cube().min, variant, turn, 1.0);
    Some(Instance {
        id: decor.value(),
        at,
        source: Source::Object(e.id()),
        meta,
    })
}
