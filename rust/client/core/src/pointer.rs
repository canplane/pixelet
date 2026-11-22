// pxl-core/pointer.rs

//! Decides what the pointer is over, here alone and in one order. A marker a
//! tap would act on comes first, because markers are drawn over the world.
//! Then comes whatever the eye's ray meets first in the world. The scene owns
//! the markers and control owns the ray, and neither names the other
//! (domain.rs), so the order is kept here, above both.

use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::layer::octant::Octant;

use crate::domain::control::edit::{self, Tool};
use crate::domain::control::navigation;
use crate::domain::layer::object::asset::Asset;
use crate::domain::query::unknown;
use crate::domain::scene::marker::{self, MarkerHit};
use crate::domain::scene::resolution;
use crate::domain::view::frame::pose;
use pxl_render::ray::RayPixel;

/// What a tap on a scene marker does with the tool held.
enum MarkerTap {
    Go(Q3),
    Remove(Octant),
    Draft(Octant),
}

/// What a tap at `x`, `y` would do to the scene marker there with the tool
/// held. The pointer tool goes to any marker. The eraser removes a pin someone
/// put up, and the marker tool opens that pin's draft. The eraser cannot
/// remove a place's marker.
fn marker_at(x: f64, y: f64) -> Option<MarkerTap> {
    match (edit::tool(), marker::tap(x, y)?) {
        (Tool::Pointer, MarkerHit::To(at) | MarkerHit::Pin(_, at)) => Some(MarkerTap::Go(at)),
        (Tool::Eraser, MarkerHit::Pin(e, _)) => Some(MarkerTap::Remove(e)),
        (
            Tool::Item {
                asset: Asset::Marker,
                ..
            },
            MarkerHit::Pin(e, _),
        ) => Some(MarkerTap::Draft(e)),
        _ => None,
    }
}

pub(crate) fn hover(x: f64, y: f64) {
    match marker_at(x, y) {
        Some(_) => edit::point_at(x, y),
        None => edit::hover_at(x, y),
    }
}

/// The drawn pixel the pointer is on, with the view's ray through its middle,
/// its origin then its unit direction (`pose::ray_of`), as the renderer is
/// asked that ray's words back (pxl-render ray/readback.rs). The cast aims a
/// pixel's ray through its middle moved by that cast's jitter (pxl-render
/// grid.wgsl `px_tan`), which the core does not know, so the ray the GPU
/// walked lies within half a drawn pixel of this one. None while the pointer
/// is off the screen, or with no drawn world.
pub(crate) fn ray_px() -> Option<RayPixel> {
    let [x, y] = edit::pointer_ndc()?;
    let size = resolution::drawn_size()?;
    let at = |ndc: f64, n: u32| (((ndc + 1.0) / 2.0 * f64::from(n)) as u32).min(n - 1);
    let px = [at(x, size[0]), at(-y, size[1])];
    let middle = |k: usize| (f64::from(px[k]) + 0.5) / f64::from(size[k]) * 2.0 - 1.0;
    let (mut origin, mut dir) = ([0.0; 3], [0.0; 3]);
    pose::ray_of(middle(0), -middle(1), &mut origin, &mut dir);
    Some(RayPixel {
        px,
        ray: [origin[0], origin[1], origin[2], dir[0], dir[1], dir[2]],
    })
}

/// Whether the pointer is over a marker a tap would act on.
pub(crate) fn over_marker() -> bool {
    edit::pointer_ndc().is_some_and(|[x, y]| marker_at(x, y).is_some())
}

pub(crate) fn tap(x: f64, y: f64) {
    match marker_at(x, y) {
        Some(MarkerTap::Go(at)) => navigation::go_to_ground(unknown(&at)),
        Some(MarkerTap::Remove(e)) => edit::write::remove(e),
        Some(MarkerTap::Draft(e)) => edit::write::draft_at(e),
        None => edit::tap(x, y),
    }
}
