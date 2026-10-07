// pxl-kernel/view/reach.rs

//! How far the view reaches — the end of the ground it can see — and the clip
//! planes set from it.

use pxl_render::look::globe::MERCATOR_RADIUS_KM;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;

use crate::query::World;
use crate::view::frame::pose::globe;
use crate::view::observation::eye;

/// The highest ground any source holds, Everest's 8,849 m, against the
/// Earth's radius: the farthest ground that still stands over the horizon.
const HIGHEST_GROUND: f64 = 8.849 / MERCATOR_RADIUS_KM;

/// From `altitude` over the datum, in units: to the horizon, and on past it as
/// far as the highest ground stands over it.
pub fn reach_at(altitude: f64) -> f64 {
    let r = globe().radius_u();
    let horizon = |h: f64| (2.0 * r * h + h * h).sqrt();
    horizon(altitude.max(0.0)) + horizon(r * HIGHEST_GROUND)
}

/// From the point's own height, so a cave's floor counts in a cave; the
/// datum where nothing is held.
fn ground_under(at: V3) -> f64 {
    let point = world::point(at[0], at[1], at[2]);
    World::now().floor_under(&point).value().flatten().unwrap_or(0.0)
}

/// Where the sky draws its own ground, so the terrain does not stand over it.
pub fn eye_ground() -> f64 {
    eye().map_or(0.0, ground_under)
}

/// What the origin grain is measured from.
pub fn height_at(at: V3) -> f64 {
    (at[2] - ground_under(at)).max(0.0)
}

pub fn eye_height() -> f64 {
    eye().map_or(0.0, height_at)
}

/// None before the camera stands anywhere, where nothing is drawn.
pub fn view_reach() -> f64 {
    eye().map_or(0.0, |e| reach_at(e[2]))
}

/// Against the reach from the datum, never under 1, so a street view keeps its
/// precision and a climb is only a change of scale.
fn scale(reach: f64) -> f64 {
    (reach / reach_at(0.0)).max(1.0)
}

pub fn near_clip_at(altitude: f64) -> f64 {
    crate::param::NEAR * scale(reach_at(altitude))
}

pub fn far_clip_at(altitude: f64) -> f64 {
    reach_at(altitude) * crate::param::FAR_CLIP
}
