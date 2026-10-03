// pxl-kernel/control/navigation/walk.rs

//! The map center standing on the ground, and where every camera's pan
//! puts it. The center's Z is half a cell over the floor the character
//! stands on, as a cell's center stands over its ground. Only z16 gives
//! that floor; a center put over coarser ground keeps its Z unknown and
//! stands on the finest ground known there until z16 is.

use pxl_spec::bake::constant::CELL;
use pxl_spec::space::pvf3::core::{FRAC_W, IVEC_Z_W};
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::v3::V3;

use crate::world::access::{Access, Answer};
use crate::world::frame::coord::{canonical_to_local, local_world_origin, local_world_to_canonical};

/// The Z lane's floor, under any ground.
const UNKNOWN_Z: i64 = -(1 << (IVEC_Z_W - 1)) << FRAC_W;

pub fn unknown_z(at: &Pvf3) -> Pvf3 {
    let mut out = Pvf3::default();
    pvf3::gather(&mut out, pvf3::X(at), pvf3::Y(at), UNKNOWN_Z);
    out
}

pub fn is_unknown(at: &Pvf3) -> bool {
    pvf3::Z(at) == UNKNOWN_Z
}

/// Keeps `at`'s Z only where z16 is known.
pub fn settle(at: &Pvf3) -> Pvf3 {
    match Access::now().surface_support(at) {
        Answer::Exact(_) => *at,
        _ => unknown_z(at),
    }
}

/// Every interaction sets its center through this, so the first pan after a
/// tap moves nothing. A wall keeps the coordinate itself.
pub fn stand(at: &Pvf3) -> Pvf3 {
    match Access::now().support(at) {
        Answer::Exact(Some(top)) => stand_over(at, top),
        Answer::Exact(None) => *at,
        Answer::Coarser(..) | Answer::Nothing(_) => unknown_z(at),
    }
}

/// The center's height over a floor whose top is `top`.
pub fn over(top: f64) -> f64 {
    top + CELL.side() / 2.0
}

/// The top of the floor `center` stands over.
pub fn floor_of(center: &Pvf3) -> f64 {
    pvf3::to_real(pvf3::Z(center)) - CELL.side() / 2.0
}

fn stand_over(at: &Pvf3, top: f64) -> Pvf3 {
    let mut out = Pvf3::default();
    pvf3::gather(&mut out, pvf3::X(at), pvf3::Y(at), pvf3::from_real(over(top)));
    out
}

/// An unknown Z on the surface of the finest ground known there.
fn standing(at: &Pvf3) -> Answer<Pvf3> {
    if !is_unknown(at) {
        return Answer::Exact(*at);
    }
    Access::now().surface_support(at).map(|top| stand_over(at, top))
}

/// Where `at` is drawn in the frame at `origin`; at `held`'s height where nothing is known.
pub fn resolve(at: &Pvf3, held: V3, origin: IVec) -> V3 {
    match standing(at) {
        Answer::Exact(p) | Answer::Coarser(p, _) => canonical_to_local(&p, origin),
        Answer::Nothing(_) => {
            let l = canonical_to_local(at, origin);
            [l[0], l[1], held[2]]
        }
    }
}

/// An unknown Z once z16 gives it.
pub fn exact(at: &Pvf3) -> Option<Pvf3> {
    standing(at).exact()
}

/// `resolve` for a camera's target, writing z16's Z into it once known.
pub fn target(to: &mut Option<Pvf3>, held: V3, origin: IVec) -> V3 {
    let Some(at) = *to else { return held };
    if let Some(p) = exact(&at) {
        *to = Some(p);
    }
    resolve(&to.unwrap_or(at), held, origin)
}

/// A pan moves the center to a new XY and stands it there, over the floor
/// its height finds (`floor_under`): down to it where the cell is open, up
/// over what fills it. Nothing between is walked, so no wall stops a pan and
/// where it lands is the destination's, not the way's. Off z16, or from an
/// unknown height, the Z is unknown and the surface stands it once known.
pub fn pan_to(from: &Pvf3, dx: f64, dy: f64, units_per_px: f64, yaw: f64) -> Pvf3 {
    let (lx, ly) = (-dx * units_per_px, dy * units_per_px);
    let (s, c) = yaw.sin_cos();
    let origin = local_world_origin();
    let l = canonical_to_local(from, origin);
    let to = *local_world_to_canonical(&mut Pvf3::default(), &[l[0] + lx * c + ly * s, l[1] - lx * s + ly * c, l[2]], origin);
    if is_unknown(from) {
        return unknown_z(&to);
    }
    match Access::now().floor_under(&to) {
        Answer::Exact(top) => stand_over(&to, top),
        _ => unknown_z(&to),
    }
}
