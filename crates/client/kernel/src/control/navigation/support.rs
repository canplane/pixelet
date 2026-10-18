// pxl-kernel/control/navigation/support.rs

//! The focus standing on the ground, and where every camera's pan
//! puts it. The center's Z is half a cell over the floor the character
//! stands on, as a cell's center stands over its ground. Only a leaf gives
//! that floor; a center put over coarser ground keeps its Z unknown and
//! stands on the finest ground known there until the leaves' is.

use crate::query::changed::Asked;
use crate::query::{over, under};
use pxl_spec::space::coord::q3::{self, FRAC_W, Q3};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::{self, EXTENT};
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;
use xpute_guest::global::Global;

use crate::layer::Answer;
use crate::query::World;
use crate::setting::store::settings_store;
use crate::view::frame::coord::{to_world, world_to_canonical};
use pxl_spec::space::layer::tile::LEAF_ZOOM;

/// The world's floor, under any ground.
const UNKNOWN_Z: i64 = -(1 << (EXTENT[2] - 1)) << FRAC_W;

fn with_z(at: &Q3, z: i64) -> Q3 {
    world::at(i64::from(at[0]), i64::from(at[1]), z)
}

pub fn unknown_z(at: &Q3) -> Q3 {
    with_z(at, UNKNOWN_Z)
}

pub fn is_unknown(at: &Q3) -> bool {
    i64::from(at[2]) == UNKNOWN_Z
}

/// A tap names the surface it struck: the center stands in the cell across
/// that face, on the floor under it.
pub fn stand(at: &Q3) -> Q3 {
    match World::now().floor_under(at, LEAF_ZOOM) {
        Answer::Exact(Some(top)) => stand_over(at, top),
        // Nothing to stand on: the point keeps its height.
        Answer::Exact(None) => *at,
        Answer::Coarser(..) | Answer::Nothing(_) => unknown_z(at),
    }
}

/// The top of the floor `center` stands over.
pub fn floor_of(center: &Q3) -> f64 {
    under(q3::to_real(i64::from(center[2])))
}

pub fn stand_over(at: &Q3, top: f64) -> Q3 {
    with_z(at, q3::from_real(over(top)))
}

/// What `standing` found under an unknown Z, of that point and its cell's
/// columns and one about: a camera's target and a destination ask it every
/// tick until a leaf gives the Z, and a target twice in one (`target`).
static STOOD: Global<Asked<Q3, Answer<Option<Q3>>>> = Global::new();

fn stood() -> &'static mut Asked<Q3, Answer<Option<Q3>>> {
    STOOD.get(|| Asked::NONE)
}

/// The turn's look at what `standing` kept, a change under it letting it go
/// (`changed::Asked::kept`): every frame turn, so each change is seen once.
pub fn support_step() {
    stood().kept();
}

/// An unknown Z on the surface of the finest ground known there; none where
/// nothing stands. Kept while the point is the one asked and nothing changed
/// under it (`STOOD`).
fn standing(at: &Q3) -> Answer<Option<Q3>> {
    if !is_unknown(at) {
        return Answer::Exact(Some(*at));
    }
    let cell = units(GROUND.cell());
    let c = to_world(at);
    let columns = [[c[0] - cell, c[1] - cell], [c[0] + cell, c[1] + cell]];
    stood().of(*at, columns, || World::now().surface_support(at).map(|top| top.map(|top| stand_over(at, top))))
}

/// The floor `center` is drawn on: its own, or where its height is not known
/// the finest ground's there; none where nothing is known.
pub fn floor_seen(center: &Q3) -> Option<f64> {
    match standing(center) {
        Answer::Exact(Some(p)) | Answer::Coarser(Some(p), _) => Some(floor_of(&p)),
        _ => None,
    }
}

/// Where `at` is drawn; at `held`'s height where nothing is known.
pub fn resolve(at: &Q3, held: V3) -> V3 {
    match standing(at) {
        Answer::Exact(Some(p)) | Answer::Coarser(Some(p), _) => to_world(&p),
        Answer::Exact(None) | Answer::Coarser(None, _) | Answer::Nothing(_) => {
            let l = to_world(at);
            [l[0], l[1], held[2]]
        }
    }
}

/// A known Z, a fix's altitude, on the floor nearest it once a leaf gives it.
pub fn nearest(at: &Q3) -> Option<Q3> {
    World::now().support_nearest(at).exact().flatten().map(|top| stand_over(at, top))
}

/// An unknown Z once a leaf gives it.
pub fn exact(at: &Q3) -> Option<Q3> {
    standing(at).exact().flatten()
}

/// `resolve` for a camera's target, writing the leaves' Z into it once known.
pub fn target(to: &mut Option<Q3>, held: V3) -> V3 {
    let Some(at) = *to else { return held };
    if let Some(p) = exact(&at) {
        *to = Some(p);
    }
    resolve(&to.unwrap_or(at), held)
}

/// By the world's step-slide, the way `lx`, `ly` in units across and up the
/// screen. It stops at the edge of a leaf on the way not held; where the leaf
/// under it, or a map's `to`, is not held, it goes on with its Z unknown until
/// a leaf gives it.
pub fn pan_to(from: &Q3, lx: f64, ly: f64, azimuth: f64, eye: &Q3) -> Q3 {
    let (s, c) = libm::sincos(azimuth);
    let l = to_world(from);
    let way = [lx * c + ly * s, -lx * s + ly * c];
    let to = world_to_canonical(&[l[0] + way[0], l[1] + way[1], l[2]]);
    if is_unknown(from) {
        return unknown_z(&to);
    }
    if !settings_store().surface_follow {
        return to;
    }
    if way == [0.0, 0.0] {
        return *from;
    }
    World::now().step_slide(from, &to, eye, true).unwrap_or_else(|| unknown_z(&to))
}
