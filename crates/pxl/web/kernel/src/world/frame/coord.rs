// pxl-kernel/world/frame/coord.rs

use pxl_spec::space::indexer::L4_RANGE;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::projection;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::V3;

use crate::world::frame::view::view;
use pxl_spec::space::pyramid::anchor_span_of;
use xpute_core::math::scalar::round;

/// The center floored to the level's anchor span across and a leaf's anchor
/// up (heights do not scale). Scaled with the level so a rebase is as far on
/// screen at every zoom; a fixed 4096u rebased nearly every frame of a pan.
pub fn origin_for(center: IVec, level: u32) -> IVec {
    let across = anchor_span_of(level);
    let up = L4_RANGE as i64;
    let floor = |v: i64, s: i64| v.div_euclid(s) * s;
    ivec::gather(floor(ivec::X(center), across), floor(ivec::Y(center), across), floor(ivec::Z(center), up))
}

pub fn local_world_origin() -> IVec {
    view().origin
}

/// X the short way round, since its lane wraps the world. Y and Z are
/// bounded: wrapping their difference would land a point a lane away.
pub fn offset(i: IVec, origin: IVec) -> V3 {
    let along = |lane: fn(IVec) -> i64| ivec::to_real(lane(i)) - ivec::to_real(lane(origin));
    [ivec::to_real(ivec::X(ivec::sub(i, origin))), along(ivec::Y), along(ivec::Z)]
}

/// The origin follows the eye up, so a height read against the frame's own
/// Z without this is off by the eye's height.
pub fn local_height(z: f64, origin: IVec) -> f64 {
    z - ivec::to_real(ivec::Z(origin))
}

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

/// The nearest canonical point to `local`, unlike `local_world_to_canonical`
/// which takes the cell it lies in. A point that went through floats and
/// came back a hair under would otherwise step down each round trip.
pub fn local_world_to_canonical_nearest<'a>(out: &'a mut Pvf3, local: &V3, map_origin: IVec) -> &'a mut Pvf3 {
    let one = pxl_spec::space::pvf3::core::ONE as f64;
    local_world_to_canonical(out, &local.map(|v| round(v * one) / one), map_origin)
}

#[cfg(test)]
#[path = "coord.test.rs"]
mod test;
