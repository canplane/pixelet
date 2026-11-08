// pxl-core/domain/layer/object/meta.rs

//! What a placed thing carries beyond its value: a decor's variant and
//! heading. Both are seeded by where the thing is placed, which is its one
//! stable identity.

use pxl_content::generation::placed_pose;
use pxl_spec::asset::decor::Decor;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::zoom::{idx_of, UNIT_ZOOM};

use crate::domain::layer::instance::{Instance, Source};
use crate::domain::layer::object::store::Entry;

/// The variant and quarter turns that `decor` gets when placed with its
/// corner at `corner` (pxl-content `placed_pose`). The hover ghost calls
/// this too, so it shows exactly what the placement will get.
pub fn user_decor_pick(corner: &Q3, decor: &Decor) -> (u32, u32) {
    placed_pose(decor.row, idx_of(UNIT_ZOOM, corner))
}

/// A placed decor as one instance, with its box's min corner at its entry's
/// min corner. None for anything placed that is not a decor.
pub fn decor_instance(e: &Entry) -> Option<Instance> {
    let decor = e.asset().decor()?;
    let (variant, turn) = user_decor_pick(&e.corner(), decor);
    let (at, meta) = decor.stand(e.cube().min, variant, turn, 1.0);
    Some(Instance {
        id: decor.val(),
        at,
        src: Source::Object(e.id()),
        meta,
    })
}
