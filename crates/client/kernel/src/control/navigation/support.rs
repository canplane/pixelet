// pxl-kernel/control/navigation/support.rs

//! The focus standing on the ground, and where every camera's pan
//! puts it. The center's Z is half a cell over the floor the character
//! stands on, as a cell's center stands over its ground. Only a leaf gives
//! that floor; a center put over coarser ground keeps its Z unknown and
//! stands on the finest ground known there until the leaves' is.

use crate::query::{over, under};
use pxl_spec::space::coord::pvf3::core::{FRAC_W, IVEC_Z_W};
use pxl_spec::space::coord::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::coord::v3::V3;

use crate::layer::Answer;
use crate::query::World;
use crate::setting::store::settings_store;
use crate::view::frame::coord::{to_world, world_to_canonical};
use pxl_spec::space::layer::tile::LEAF_ZOOM;

/// The Z lane's floor, under any ground.
const UNKNOWN_Z: i64 = -(1 << (IVEC_Z_W - 1)) << FRAC_W;

fn with_z(at: &Pvf3, z: i64) -> Pvf3 {
    pvf3::gather(pvf3::X(at), pvf3::Y(at), z)
}

pub fn unknown_z(at: &Pvf3) -> Pvf3 {
    with_z(at, UNKNOWN_Z)
}

pub fn is_unknown(at: &Pvf3) -> bool {
    pvf3::Z(at) == UNKNOWN_Z
}

/// A tap names the surface it struck: the center stands in the cell across
/// that face, on the floor under it.
pub fn stand(at: &Pvf3) -> Pvf3 {
    match World::now().floor_under(at, LEAF_ZOOM) {
        Answer::Exact(Some(top)) => stand_over(at, top),
        // Nothing to stand on: the point keeps its height.
        Answer::Exact(None) => *at,
        Answer::Coarser(..) | Answer::Nothing(_) => unknown_z(at),
    }
}

/// The top of the floor `center` stands over.
pub fn floor_of(center: &Pvf3) -> f64 {
    under(pvf3::to_real(pvf3::Z(center)))
}

pub fn stand_over(at: &Pvf3, top: f64) -> Pvf3 {
    with_z(at, pvf3::from_real(over(top)))
}

/// An unknown Z on the surface of the finest ground known there; none where
/// nothing stands.
fn standing(at: &Pvf3) -> Answer<Option<Pvf3>> {
    if !is_unknown(at) {
        return Answer::Exact(Some(*at));
    }
    World::now().surface_support(at).map(|top| top.map(|top| stand_over(at, top)))
}

/// The floor `center` is drawn on: its own, or where its height is not known
/// the finest ground's there; none where nothing is known.
pub fn floor_seen(center: &Pvf3) -> Option<f64> {
    match standing(center) {
        Answer::Exact(Some(p)) | Answer::Coarser(Some(p), _) => Some(floor_of(&p)),
        _ => None,
    }
}

/// Where `at` is drawn; at `held`'s height where nothing is known.
pub fn resolve(at: &Pvf3, held: V3) -> V3 {
    match standing(at) {
        Answer::Exact(Some(p)) | Answer::Coarser(Some(p), _) => to_world(&p),
        Answer::Exact(None) | Answer::Coarser(None, _) | Answer::Nothing(_) => {
            let l = to_world(at);
            [l[0], l[1], held[2]]
        }
    }
}

/// An unknown Z once a leaf gives it.
pub fn exact(at: &Pvf3) -> Option<Pvf3> {
    standing(at).exact().flatten()
}

/// `resolve` for a camera's target, writing the leaves' Z into it once known.
pub fn target(to: &mut Option<Pvf3>, held: V3) -> V3 {
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
pub fn pan_to(from: &Pvf3, lx: f64, ly: f64, azimuth: f64, eye: &Pvf3) -> Pvf3 {
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
