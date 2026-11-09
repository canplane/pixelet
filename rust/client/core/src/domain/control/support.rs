// pxl-core/domain/control/support.rs

//! How the camera's pivot stands on the ground, and where a pan moves it.
//! The pivot's Z is half a cell above the floor the character stands on,
//! the height of a cell's center over its floor (`query::over`). The pivot
//! follows that floor when the world under it changes. Only a leaf, a tile
//! at `LEAF_ZOOM`, gives the floor. A pivot put over coarser ground keeps
//! its Z unknown (`UNKNOWN_Z`) and is drawn on the finest ground known there
//! until the leaf arrives.

use crate::domain::query::change::Memo;
use crate::domain::query::{floor_of, is_unknown, on_floor, unknown};
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;
use xpute_core::global::Global;

use crate::domain::layer::Answer;
use crate::domain::query::World;
use crate::domain::setting::store::settings;
use crate::domain::view::frame::coord::{to_world, world_to_canonical};
use pxl_spec::space::layer::quadrant::LEAF_ZOOM;

/// Where the pivot stands for the cell center `at`, such as the center of
/// the cell across the face a tap struck. It stands on the floor under that
/// cell. Its Z is unknown until a leaf gives that floor.
pub fn stand(at: &Q3) -> Q3 {
    match World::now().floor_under(at, LEAF_ZOOM) {
        Answer::Exact(Some(top)) => on_floor(at, top),
        // Nothing stands under the point, so it keeps its height.
        Answer::Exact(None) => *at,
        Answer::Coarser(..) | Answer::Nothing(_) => unknown(at),
    }
}

/// The last answer of `standing`. It is kept until another point is asked
/// or the world changes in the columns the answer read (`change::Memo`),
/// because a camera's `to` and a destination ask for the same point on every
/// tick until a leaf gives its Z, and `resolve_to` asks twice in one tick.
static STAND_MEMO: Global<Memo<Q3, Answer<Option<Q3>>>> = Global::new();

fn stand_memo() -> &'static mut Memo<Q3, Answer<Option<Q3>>> {
    STAND_MEMO.get(|| Memo::NONE)
}

/// Drops the kept answer of `standing` where a change touched the columns
/// it read. It runs on every frame turn, so each change is seen exactly once
/// (`change::within`).
pub fn support_step() {
    stand_memo().forget_touched();
}

/// Where `at` stands in `world`. A point with an unknown Z stands on the
/// surface of the finest ground known there, or on nothing where nothing
/// stands. A point with a known Z stays as it is unless the surface is
/// followed (the `surface_follow` setting). When it is followed, the point
/// stands on the floor under it as a leaf gives it: it drops when what it
/// stood on is taken away, and it rises out of what fills its cell
/// (`World::floor_under`). The pivot and the figures at it then stand on
/// one floor.
fn standing(world: World<'_>, at: &Q3) -> Answer<Option<Q3>> {
    if !is_unknown(at) {
        if !settings().surface_follow {
            return Answer::Exact(Some(*at));
        }
        let floor = stand_memo().of(world, *at, |w| {
            w.floor_under(at, LEAF_ZOOM)
                .map(|top| top.map(|top| on_floor(at, top)))
        });
        return Answer::Exact(Some(floor.exact().flatten().unwrap_or(*at)));
    }
    stand_memo().of(world, *at, |w| {
        w.surface_support(at)
            .map(|top| top.map(|top| on_floor(at, top)))
    })
}

/// The top of the floor `at` is drawn on in `world`. For a point whose
/// height is not known, it is the floor of the finest ground known there.
/// None where nothing is known there.
pub fn floor_at(world: World<'_>, at: &Q3) -> Option<f64> {
    match standing(world, at) {
        Answer::Exact(Some(p)) | Answer::Coarser(Some(p), _) => Some(floor_of(&p)),
        _ => None,
    }
}

/// Where `at` is drawn, in world coordinates. Where nothing is known under
/// it, it is drawn at the height of `cur`.
pub fn resolve(at: &Q3, cur: V3) -> V3 {
    match standing(World::now(), at) {
        Answer::Exact(Some(p)) | Answer::Coarser(Some(p), _) => to_world(&p),
        Answer::Exact(None) | Answer::Coarser(None, _) | Answer::Nothing(_) => {
            let l = to_world(at);
            [l[0], l[1], cur[2]]
        }
    }
}

/// A point with a known Z, such as a location fix's altitude, put on the
/// floor nearest that Z. None until a leaf gives that floor.
pub fn nearest(at: &Q3) -> Option<Q3> {
    World::now()
        .support_nearest(at)
        .exact()
        .flatten()
        .map(|top| on_floor(at, top))
}

/// Where `at` stands, once a leaf gives the floor there. A point with an
/// unknown Z gets its Z from here.
pub fn exact(at: &Q3) -> Option<Q3> {
    standing(World::now(), at).exact().flatten()
}

/// `resolve` for a camera's `to`. A Z not yet known is written into `to` once
/// a leaf gives it, and a known Z is not written over.
pub fn resolve_to(to: &mut Option<Q3>, cur: V3) -> V3 {
    let Some(at) = *to else { return cur };
    if let Some(p) = exact(&at).filter(|_| is_unknown(&at)) {
        *to = Some(p);
    }
    resolve(&to.unwrap_or(at), cur)
}

/// Where a pan moves the pivot from `from`, by `lx` units across the screen
/// and `ly` units up it. The pivot slides over the world as a walker steps
/// (`World::step_slide`). Where a leaf on the way is not held, the pivot goes
/// on with its Z unknown until a leaf gives it.
pub fn pan_to(from: &Q3, lx: f64, ly: f64, azimuth: f64) -> Q3 {
    let (s, c) = libm::sincos(azimuth);
    let l = to_world(from);
    let way = [lx * c + ly * s, -lx * s + ly * c];
    let to = world_to_canonical(&[l[0] + way[0], l[1] + way[1], l[2]]);
    if !settings().surface_follow {
        return to;
    }
    if way == [0.0, 0.0] {
        return *from;
    }
    World::now().step_slide(from, &to, false)
}

#[cfg(test)]
#[path = "support.test.rs"]
mod test;
