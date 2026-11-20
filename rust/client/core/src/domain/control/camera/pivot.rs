// pxl-core/domain/control/camera/pivot.rs

//! How the camera's pivot stands on the ground, and where a pan moves it.
//! The pivot moves in reals, as the camera does, and stands as a point
//! does: the world is asked at the point of the cell it lies in, and its Z
//! is half a cell above the floor the character stands on, the height of a
//! cell's center over its floor (`query::over`). A place the camera is sent
//! to is made a point first (navigation/place.rs), and a pan walks to
//! its height where the eye draws a body, and carries the height it has
//! elsewhere. The pivot then follows its floor when the world under it
//! changes. Where it stands is read at the leaves (`SizeBy::Leaf`), the
//! ground's own, and only an exact answer moves it: a coarser tile standing
//! in until the finer one arrives is never made its height (ARCHITECTURE.md
//! "Honest answers"). A pan's walk is a body's, read as the eye draws one.

use crate::domain::query::change::Memo;
use crate::domain::query::{is_unknown, on_floor};
use pxl_global::Global;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;

use crate::domain::query::{SizeBy, World};
use crate::domain::setting::settings;
use crate::domain::view::frame::coord::{to_world, world_to_canonical};

/// The last answer of `stood` where the surface is followed, kept by what it
/// is a function of beside the world: the point. It is kept until another is
/// asked or the world changes in the rect it read (`change::Memo`): the
/// camera asks for the same point on every tick.
static STAND_MEMO: Global<Memo<Q3, Q3>> = Global::new();

fn stand_memo() -> &'static mut Memo<Q3, Q3> {
    STAND_MEMO.get(|| Memo::NONE)
}

/// Drops the kept answer of `stood` where a change touched the rect it
/// read. It runs on every frame, so each change is seen exactly once
/// (`change::within`).
pub(crate) fn forget_touched() {
    stand_memo().forget_touched();
}

/// Where the pivot `at` stands. It stays as it is unless the surface is
/// followed (the `surface_follow` setting). When it is followed, it stands
/// on the floor under it where that is answered exactly: it drops when what
/// it stood on is taken away, and it rises out of what fills its cell
/// (`World::floor_under`). The pivot and the figures at it then stand on one
/// floor. The pivot is the camera's point and no body: it stands on its
/// floor for the camera whether or not the eye draws a body there.
pub(crate) fn stood(at: &Q3) -> Q3 {
    if !settings().surface_follow {
        return *at;
    }
    stand_memo().of(*at, || {
        let floor = World::now().floor_under(at, SizeBy::Leaf);
        floor.exact().flatten().map_or(*at, |top| on_floor(at, top))
    })
}

/// The pivot `at` with its height the one it stands at: the world is asked
/// at the point of the cell it lies in (`stood`).
pub(crate) fn stood_at(at: &V3) -> V3 {
    [at[0], at[1], to_world(&stood(&world_to_canonical(at)))[2]]
}

/// Where a pan moves the pivot from `from`, by `lx` units across the screen
/// and `ly` units up it, in reals. Where the eye draws a body the pivot
/// walks as one steps (`World::step_slide`), from the point of the cell it
/// lies in to the one it is moved into, and takes the height the walk ends
/// at. On each axis the walk leaves the pivot its real: where the body
/// reached the point asked, the real asked, so a pan of any length moves it
/// by that length; where the body did not leave its point, the real the
/// pivot had; and only where the walk was turned aside onto another point,
/// that point. Elsewhere, and where what the eye asks on the way is not
/// held, the pivot goes where it is moved at the height it has, and follows
/// its floor from there (`stood`).
pub(crate) fn pan_to(from: &V3, lx: f64, ly: f64, azimuth: f64) -> V3 {
    let (s, c) = libm::sincos(azimuth);
    let way = [lx * c + ly * s, -lx * s + ly * c];
    // Held inside the world, which ends in Y, as a point is.
    let to = world::within([from[0] + way[0], from[1] + way[1], from[2]]);
    if !settings().surface_follow || way == [0.0, 0.0] {
        return to;
    }
    let (was, asked) = (world_to_canonical(from), world_to_canonical(&to));
    let walked = World::now().step_slide(&was, &asked, false);
    if is_unknown(&walked) {
        return to;
    }
    let at = to_world(&walked);
    let on = |k: usize| match (walked[k] == asked[k], walked[k] == was[k]) {
        (true, _) => to[k],
        (false, true) => from[k],
        (false, false) => at[k],
    };
    [on(0), on(1), at[2]]
}

#[cfg(test)]
#[path = "pivot.test.rs"]
mod test;
