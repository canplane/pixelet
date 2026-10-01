// pxl-kernel/layer/terrain/ground_snap.rs

//! The map center standing on the ground: where the camera pivots over a
//! coordinate, and the one walk every camera's pan takes.

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};

use crate::world::access::{Access, Answer};
use crate::world::frame::coord::{canonical_to_local, local_world_origin, local_world_to_canonical};

/// Where a walker standing from a coordinate is, as the camera pivots on it.
pub enum Pivot {
    /// The center of the cube standing on what supports a walker starting
    /// from the cell the coordinate lies in (world/access.rs `support`) —
    /// the cell the character occupies.
    Stands(Pvf3),
    /// That cell is filled taller than a step: a wall.
    Blocked,
    /// No page over it is resident.
    Unknown,
}

/// A center, not the top face, because the camera pivots on the cube the
/// character would fill. Every map center an interaction sets — a pan, a
/// tap — is this one, and the free camera's eye stands on the same support,
/// so the first pan after a tap moves nothing. Standing things (the puck,
/// decor, a place) are grounded to the top face itself; this is the one
/// place the camera's own convention is applied.
pub fn ground_pivot(at: &Pvf3) -> Pivot {
    match Access::now().support(at) {
        Answer::Exact(Some(top)) | Answer::Coarser(Some(top), _) => Pivot::Stands(pivot_over(at, top)),
        Answer::Exact(None) | Answer::Coarser(None, _) => Pivot::Blocked,
        Answer::Nothing(_) => Pivot::Unknown,
    }
}

/// The pivot over `at` on ground whose top is `top`.
pub fn pivot_over(at: &Pvf3, top: f64) -> Pvf3 {
    let mut out = Pvf3::default();
    pvf3::gather(&mut out, pvf3::X(at), pvf3::Y(at), pvf3::from_real(top + U_PER_CELL / 2.0));
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
/// `units_per_px`, turned by the heading `yaw`, stepping as a walker does —
/// down to the floor beside, up a step, and stopped by a wall, where it
/// stays where it was. Where the ground beside is not known yet it goes on
/// at its height, and the ground arriving later moves nothing. Every camera
/// pans by it, so the walk is the same whichever looks.
pub fn walk(from: &Pvf3, dx: f64, dy: f64, units_per_px: f64, yaw: f64) -> Pvf3 {
    let next = moved(from, dx, dy, units_per_px, yaw);
    match ground_pivot(&next) {
        Pivot::Stands(p) => p,
        Pivot::Blocked => *from,
        Pivot::Unknown => next,
    }
}

/// `from` moved by `dx`, `dy` pixels of drag at `units_per_px`, turned by
/// the heading `yaw`, at its own height: a drag's move before anything
/// stands it.
pub fn moved(from: &Pvf3, dx: f64, dy: f64, units_per_px: f64, yaw: f64) -> Pvf3 {
    let (lx, ly) = (-dx * units_per_px, dy * units_per_px);
    let (s, c) = yaw.sin_cos();
    let origin = local_world_origin();
    let at = canonical_to_local(from, origin);
    let mut next = Pvf3::default();
    local_world_to_canonical(&mut next, &[at[0] + lx * c + ly * s, at[1] - lx * s + ly * c, at[2]], origin);
    next
}
