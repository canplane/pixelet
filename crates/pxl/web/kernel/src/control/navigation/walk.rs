// pxl-kernel/control/navigation/walk.rs

//! The map center standing on the ground, and the one walk every camera's
//! pan takes. The center's Z is half a cell over the floor the character
//! stands on, as a cell's center stands over its ground.

use pxl_spec::space::pvf3::pvf3::{self, Pvf3};

use pxl_spec::bake::constant::CELL;
use pxl_spec::space::pyramid::GROUND;

use crate::world::access::{Access, Answer, FINEST};
use crate::world::frame::coord::{canonical_to_local, local_height, local_world_origin, local_world_to_canonical};

pub enum Stand {
    /// Half a cell over the floor that supports a walker from its cell.
    On(Pvf3),
    /// Filled taller than a step: a wall.
    Blocked,
    Unknown,
}

/// Every interaction sets its center through this, so the first pan after a
/// tap moves nothing.
pub fn stand(at: &Pvf3) -> Stand {
    match Access::now().support(at) {
        Answer::Exact(Some(top)) | Answer::Coarser(Some(top), _) => Stand::On(stand_over(at, top)),
        Answer::Exact(None) | Answer::Coarser(None, _) => Stand::Blocked,
        Answer::Nothing(_) => Stand::Unknown,
    }
}

pub fn stand_over(at: &Pvf3, top: f64) -> Pvf3 {
    let mut out = Pvf3::default();
    pvf3::gather(&mut out, pvf3::X(at), pvf3::Y(at), pvf3::from_real(top + CELL.side() / 2.0));
    out
}

/// Falls back to the coordinate itself where it stands nowhere known.
pub fn stand_or_raw(coord: &Pvf3) -> Pvf3 {
    match stand(coord) {
        Stand::On(p) => p,
        Stand::Blocked | Stand::Unknown => *coord,
    }
}

/// Steps a cell at a time of whatever level the ground is known at. At z16
/// it walks as a walker does (down, up a step, stopped by a wall); on a
/// coarser level it follows the surface with no step, since a 1u step means
/// nothing there; where nothing is known it keeps its height.
pub fn walk(from: &Pvf3, dx: f64, dy: f64, units_per_px: f64, yaw: f64) -> Pvf3 {
    let (lx, ly) = (-dx * units_per_px, dy * units_per_px);
    let (s, c) = yaw.sin_cos();
    let way = [lx * c + ly * s, -lx * s + ly * c];
    let length = way[0].hypot(way[1]);
    let origin = local_world_origin();
    let mut here = canonical_to_local(from, origin);
    let mut gone = 0.0;
    let access = Access::now();
    // A center stood on a coarser surface before the exact ground arrived may be
    // inside the ground: stand it on what it is inside first.
    if access.known_level(from) == Some(FINEST) {
        if let Answer::Exact(top) = access.floor_under(from) {
            here[2] = here[2].max(local_height(top, origin) + CELL.side() / 2.0);
        }
    }
    while gone < length {
        let mut at = Pvf3::default();
        local_world_to_canonical(&mut at, &here, origin);
        let known = access.known_level(&at);
        let step = known.map_or(CELL.side(), |level| GROUND.scale(level).side()).min(length - gone);
        let next = [here[0] + way[0] / length * step, here[1] + way[1] / length * step, here[2]];
        let mut probe = Pvf3::default();
        local_world_to_canonical(&mut probe, &next, origin);
        let z = match access.known_level(&probe) {
            Some(FINEST) => match access.support(&probe) {
                Answer::Exact(Some(top)) => Some(top),
                // A wall, or z16 turning out coarser: it stops where it got to.
                _ => break,
            },
            Some(_) => access.surface_support(&probe).value(),
            None => None,
        };
        here = [next[0], next[1], z.map_or(next[2], |top| local_height(top, origin) + CELL.side() / 2.0)];
        gone += step;
    }
    let mut out = Pvf3::default();
    local_world_to_canonical(&mut out, &here, origin);
    out
}
