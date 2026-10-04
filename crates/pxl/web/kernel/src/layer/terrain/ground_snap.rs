// pxl-kernel/layer/terrain/ground_snap.rs

//! The map center standing on the ground, and the one walk every camera's
//! pan takes.
//!
//! The map center is the character's place and the camera's center, one
//! point, where it says it is: X and Y where the hand left it, Z half a cell
//! over the floor the character stands on (world/access.rs `support`), as
//! a cell's center stands over its ground (pxl-spec space/pyramid.rs
//! `Cell::center`).

use pxl_spec::space::pvf3::pvf3::{self, Pvf3};

use pxl_spec::bake::constant::CELL;
use pxl_spec::space::pyramid::GROUND;

use crate::world::access::{Access, Answer, FINEST};
use crate::world::frame::coord::{canonical_to_local, local_height, local_world_origin, local_world_to_canonical};

/// Where a walker standing from a coordinate is.
pub enum Pivot {
    /// The coordinate half a cell over the floor of what supports a walker
    /// starting from the cell it lies in (world/access.rs `support`).
    Stands(Pvf3),
    /// That cell is filled taller than a step: a wall.
    Blocked,
    /// No page over it is resident.
    Unknown,
}

/// Every map center an interaction sets — a pan, a tap — is this one, in
/// either person, so the first pan after a tap moves nothing.
pub fn ground_pivot(at: &Pvf3) -> Pivot {
    match Access::now().support(at) {
        Answer::Exact(Some(top)) | Answer::Coarser(Some(top), _) => Pivot::Stands(pivot_over(at, top)),
        Answer::Exact(None) | Answer::Coarser(None, _) => Pivot::Blocked,
        Answer::Nothing(_) => Pivot::Unknown,
    }
}

/// `at` on ground whose top is `top`: at its X and Y, half a cell over that
/// floor.
pub fn pivot_over(at: &Pvf3, top: f64) -> Pvf3 {
    let mut out = Pvf3::default();
    pvf3::gather(&mut out, pvf3::X(at), pvf3::Y(at), pvf3::from_real(top + CELL.side() / 2.0));
    out
}

/// ground_pivot for a coordinate a walker is put at, falling back to the
/// coordinate itself where it stands nowhere known.
pub fn ground_or_raw(coord: &Pvf3) -> Pvf3 {
    match ground_pivot(coord) {
        Pivot::Stands(p) => p,
        Pivot::Blocked | Pivot::Unknown => *coord,
    }
}

/// A pan's walk from pivot `from`: `dx`, `dy` pixels of drag at
/// `units_per_px`, turned by the heading `yaw`, taken a cell at a time of
/// whatever the ground along it is known at (world/access.rs
/// `known_level`), so it costs the cells it crosses and not the units.
///
/// Where the ground is known exactly — z16, the 1u world a walker lives in
/// — it steps as a walker does (`support`): down to the floor beside, up a
/// step (param.rs `STEP_CELLS`), and a wall stops it where it got to. Where
/// only a coarser level is known, nothing says where a 1u wall stands, and
/// it follows that level's surface (`surface_support`) with no step to
/// it — a step's meaning is not stretched to a coarser cell. Where nothing
/// is known it goes on at its height. The ground arriving later moves
/// nothing; the next walk takes it. Every camera pans by it, so the walk is
/// the same whichever looks.
pub fn walk(from: &Pvf3, dx: f64, dy: f64, units_per_px: f64, yaw: f64) -> Pvf3 {
    let (lx, ly) = (-dx * units_per_px, dy * units_per_px);
    let (s, c) = yaw.sin_cos();
    let way = [lx * c + ly * s, -lx * s + ly * c];
    let length = way[0].hypot(way[1]);
    let origin = local_world_origin();
    let mut here = canonical_to_local(from, origin);
    let mut gone = 0.0;
    let access = Access::now();
    // A map center found inside the ground — stood on a coarser level's
    // surface before the exact ground came — is out of a walker's world:
    // the walk first stands it on what it is inside (`floor_under`), which
    // the arrival itself never did.
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
                // A wall, or what z16 says turning out coarser: it stops
                // where it got to.
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
