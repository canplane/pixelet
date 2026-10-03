// pxl-kernel/control/navigation/support.rs

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

use crate::world::access::{Access, Answer, Detail, Ray, FINEST};
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

/// A tap names the surface it struck: the center stands in the cell across
/// that face, on the floor under it.
pub fn stand(at: &Pvf3) -> Pvf3 {
    match Access::now().floor_under(at) {
        Answer::Exact(top) => stand_over(at, top),
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

/// A pan moves the center to a new XY and stands it there. In the open it
/// follows the destination's surface (`floor_under`): down where the cell is
/// open, up over what fills it, nothing between walked. Covered — under a
/// roof, a deck, a cave's rock — it keeps its layer: it goes as far along the
/// way at its height as nothing solid stands (half a cell short of what
/// does), at any zoom, and down to the floor there. Off z16, or from an
/// unknown height, the Z is unknown and the surface stands it once known.
pub fn pan_to(from: &Pvf3, dx: f64, dy: f64, units_per_px: f64, yaw: f64) -> Pvf3 {
    let (lx, ly) = (-dx * units_per_px, dy * units_per_px);
    let (s, c) = yaw.sin_cos();
    let origin = local_world_origin();
    let l = canonical_to_local(from, origin);
    let way = [lx * c + ly * s, -lx * s + ly * c];
    let along = |f: f64| *local_world_to_canonical(&mut Pvf3::default(), &[l[0] + way[0] * f, l[1] + way[1] * f, l[2]], origin);
    let to = along(1.0);
    if is_unknown(from) {
        return unknown_z(&to);
    }
    let access = Access::now();
    if access.covered(from).exact() == Some(true) {
        let length = way[0].hypot(way[1]);
        let ray = Ray {
            origin: [pvf3::to_real(pvf3::X(from)), pvf3::to_real(pvf3::Y(from)), pvf3::to_real(pvf3::Z(from))],
            direction: [way[0] / length, way[1] / length, 0.0],
            reach: length,
        };
        // Where the way is not known exactly, it goes nowhere.
        let free = match access.first_hit(&ray, Detail::Level(FINEST)) {
            Answer::Exact(None) => length,
            Answer::Exact(Some(hit)) => (hit.t - CELL.side() / 2.0).max(0.0),
            _ => 0.0,
        };
        let to = along(if length > 0.0 { free / length } else { 0.0 });
        return match access.support(&to, 0) {
            Answer::Exact(Some(top)) => stand_over(&to, top),
            _ => *from,
        };
    }
    match access.floor_under(&to) {
        Answer::Exact(top) => stand_over(&to, top),
        _ => unknown_z(&to),
    }
}
