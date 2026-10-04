// pxl-kernel/control/navigation/walk.rs

//! The map center standing on the ground, and the one walk every camera's
//! pan takes. The center's Z is half a cell over the floor the character
//! stands on, as a cell's center stands over its ground. Only z16 gives
//! that floor; a center put over coarser ground keeps its Z unknown and
//! stands on the finest ground known there until z16 is.

use pxl_spec::bake::constant::CELL;
use pxl_spec::space::pvf3::core::{FRAC_W, IVEC_Z_W};
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::pyramid::GROUND;
use pxl_spec::space::v3::V3;

use crate::world::access::{Access, Answer, FINEST};
use crate::world::frame::coord::{canonical_to_local, local_height, local_world_origin, local_world_to_canonical};

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

/// Steps a cell at a time of whatever level the ground is known at. At z16
/// it walks as a walker does (down, up a step, stopped by a wall); on a
/// coarser level it follows the surface with no step, since a 1u step means
/// nothing there; where nothing is known it keeps its height. It ends with
/// its Z unknown off z16.
pub fn walk(from: &Pvf3, dx: f64, dy: f64, units_per_px: f64, yaw: f64) -> Pvf3 {
    let (lx, ly) = (-dx * units_per_px, dy * units_per_px);
    let (s, c) = yaw.sin_cos();
    let way = [lx * c + ly * s, -lx * s + ly * c];
    let length = way[0].hypot(way[1]);
    let origin = local_world_origin();
    let access = Access::now();
    let canonical = |l: &V3| *local_world_to_canonical(&mut Pvf3::default(), l, origin);
    let Some(from) = standing(from).value() else {
        let l = canonical_to_local(from, origin);
        return unknown_z(&canonical(&[l[0] + way[0], l[1] + way[1], 0.0]));
    };
    let mut here = canonical_to_local(&from, origin);
    // A block placed where the center stands buries it.
    if let Answer::Exact(top) = access.floor_under(&from) {
        here[2] = here[2].max(local_height(over(top), origin));
    }
    // Whether the height came from z16; a gap with nothing known keeps it.
    let mut exact = access.known_level(&from) == Some(FINEST);
    let mut gone = 0.0;
    while gone < length {
        let known = access.known_level(&canonical(&here));
        let step = known.map_or(CELL.side(), |level| GROUND.scale(level).side()).min(length - gone);
        let next = [here[0] + way[0] / length * step, here[1] + way[1] / length * step, here[2]];
        let probe = canonical(&next);
        let z = match access.known_level(&probe) {
            // A height from coarser ground was its surface's, so z16's surface takes over, with no wall.
            Some(FINEST) if !exact => {
                exact = true;
                access.surface_support(&probe).value()
            }
            Some(FINEST) => match access.support(&probe) {
                Answer::Exact(Some(top)) => Some(top),
                // A wall, or z16 turning out coarser: it stops where it got to.
                _ => break,
            },
            Some(_) => {
                exact = false;
                access.surface_support(&probe).value()
            }
            None => None,
        };
        here = [next[0], next[1], z.map_or(next[2], |top| local_height(over(top), origin))];
        gone += step;
    }
    settle(&canonical(&here))
}
