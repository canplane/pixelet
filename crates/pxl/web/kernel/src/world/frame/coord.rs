// pxl-kernel/world/frame/coord.rs

use pxl_spec::space::indexer::L4_RANGE;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::projection;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::V3;

use crate::layer::terrain::level::anchor_span_of;
use crate::world::frame::view::view;

/// The floating origin for a view centred on `center` whose finest level is
/// `level`: the centre floored to that level's anchor across (level.rs
/// `anchor_span_of`), and to a leaf's anchor up, since heights do not scale
/// with the level. Scaled with the level so it is as far on screen at every
/// zoom: a leaf's 4096u is twenty pixels at the farthest, a rebase in nearly
/// every frame of a pan.
pub fn origin_for(center: IVec, level: u32) -> IVec {
    let across = anchor_span_of(level);
    let up = L4_RANGE as i64;
    let floor = |v: i64, s: i64| v.div_euclid(s) * s;
    ivec::gather(floor(ivec::X(center), across), floor(ivec::Y(center), across), floor(ivec::Z(center), up))
}

/// The floating origin every local position is measured from: the one the
/// view is drawn in, decided once a tick by the translation step
/// (view_frame.rs), so that nothing reads a position in one origin and draws
/// it in another.
pub fn local_world_origin() -> IVec {
    view().origin
}

pub fn local_world_to_canonical<'a>(out: &'a mut Pvf3, local: &V3, map_origin: IVec) -> &'a mut Pvf3 {
    projection::project(out, local[0], local[1], local[2]);
    out.i = ivec::add(out.i, map_origin);
    out
}
