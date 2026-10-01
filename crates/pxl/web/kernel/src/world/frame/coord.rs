// pxl-kernel/world/frame/coord.rs

use pxl_spec::space::indexer::L4_RANGE;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::projection;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::V3;

use crate::world::frame::view::view;
use pxl_spec::space::pyramid::anchor_span_of;

/// The floating origin for a view centered on `center` whose finest level is
/// `level`: the center floored to that level's anchor across (level.rs
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
/// view is drawn in, decided where the view is composed, about the eye
/// (view.rs `view_compose`), so that nothing reads a position in one origin
/// and draws it in another.
pub fn local_world_origin() -> IVec {
    view().origin
}

/// Where a lattice address lies from `origin`, in units of the frame the
/// view is drawn in. X the short way round, since its lane is one world;
/// Y and Z are bounded, and a difference past half a lane is a real one, not
/// the way back round — the lanes' own difference would wrap it, and a point
/// read across that wrap lands a lane's length away.
pub fn offset(i: IVec, origin: IVec) -> V3 {
    let along = |lane: fn(IVec) -> i64| ivec::to_real(lane(i)) - ivec::to_real(lane(origin));
    [ivec::to_real(ivec::X(ivec::sub(i, origin))), along(ivec::Y), along(ivec::Z)]
}

/// A height in units — the ground's top, as the query answers it — in the
/// frame whose origin is `origin`. The origin follows the eye up, so a
/// height read against the frame's own Z without this is off by as much as
/// the eye is high.
pub fn local_height(z: f64, origin: IVec) -> f64 {
    z - ivec::to_real(ivec::Z(origin))
}

/// A canonical point in the frame whose origin is `origin`:
/// `local_world_to_canonical`'s inverse.
pub fn canonical_to_local(p: &Pvf3, origin: IVec) -> V3 {
    let at = offset(p.i, origin);
    let mut f = [0.0; 3];
    projection::unproject(&mut f, 0, p.f);
    std::array::from_fn(|k| at[k] + f[k])
}

pub fn local_world_to_canonical<'a>(out: &'a mut Pvf3, local: &V3, map_origin: IVec) -> &'a mut Pvf3 {
    projection::project(out, local[0], local[1], local[2]);
    out.i = ivec::add(out.i, map_origin);
    out
}

#[cfg(test)]
#[path = "coord.test.rs"]
mod test;
