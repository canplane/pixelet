// pxl-core/domain/control/edit/pick.rs

//! The pointer's pick (`Pick`), and what it struck means for a placement or
//! an erase. The pick asks the world at terrain's finest zoom, the leaves',
//! because the leaves are what the screen shows around the eye.

use crate::domain::layer::instance::{Instance, Source};
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::zoom::{of_side, units, Zoom};

use crate::domain::control::camera::controller;
use crate::domain::control::edit::collision::object_aabb;
use crate::domain::control::edit::Target;
use crate::domain::layer::object::store::Entry;
use crate::domain::query;
use crate::domain::query::{finest, Hit, HitKind, Ray, World};
use crate::domain::setting::store::settings;
use crate::domain::view::frame::coord::world_to_canonical;
use crate::domain::view::frame::pose::{eye, ray_of};
use crate::domain::view::lens::decor_zooms;
use pxl_spec::space::coord::aabb::Aabb;

/// How far from the eye the pointer can pick, in units. It is a product
/// decision: what can be edited is a sphere about the observer.
const PICK_DIST_U: f64 = 32.0;

// Larger than the error of a round trip through the 1/256 grid (±1/512 u),
// and smaller than the gap between any two object tiers (the finest tier's
// side).
const SAME_EXTENT_EPS: f64 = units(OBJECT_FINEST) / 16.0;

/// The octree stores an object of side `size` only on these grid lines. A
/// value within `SAME_EXTENT_EPS` of a line counts as on it.
fn grid_ceil(v: f64, size: f64) -> f64 {
    ((v - SAME_EXTENT_EPS) / size).ceil() * size
}

fn grid_floor(v: f64, size: f64) -> f64 {
    ((v + SAME_EXTENT_EPS) / size).floor() * size
}

/// What the hover's pick struck, and the cell zoom at which the eye's rays
/// draw the point it struck (domain/query/ray.rs `by_dist`). Near the eye
/// that zoom is finer than the ground's cells, as placed objects may be.
/// `ray_zoom` is None without an eye.
pub(crate) struct Pick {
    pub hit: Hit,
    pub ray_zoom: Option<Zoom>,
}

impl Pick {
    /// The cell struck: for a decor the decor's cell, whatever its box;
    /// otherwise the box struck.
    fn cell(&self) -> Aabb {
        match self.hit.kind {
            HitKind::Decor(i) => i.cell(),
            _ => self.hit.cube,
        }
    }

    pub(crate) fn zoom(&self) -> Zoom {
        of_side(self.cell().extent()[0])
    }

    /// Whether the rays draw the struck point coarser than the cell struck.
    /// What is drawn there is then a summary of the cells, not the cells
    /// themselves, so nothing in it can be struck.
    pub(crate) fn is_folded(&self) -> bool {
        self.ray_zoom.is_some_and(|z| z < self.zoom())
    }

    /// The placed object struck: a cube, or a decor someone placed. None for
    /// the ground and for decor the ground generated.
    pub(super) fn object(&self) -> Option<Entry> {
        match self.hit.kind {
            HitKind::Object(at)
            | HitKind::Decor(Instance {
                src: Source::Object(at),
                ..
            }) => World::now().entry_at(at),
            HitKind::Ground | HitKind::Objects | HitKind::Decor(_) => None,
        }
    }

    /// What was struck, as a tool reads it: the placed object, or the decor
    /// the ground generated and the leaf cell it stands in.
    pub(super) fn target(&self) -> Option<Target> {
        if let HitKind::Decor(i) = self.hit.kind {
            if let Some(cell) = i.generated_in() {
                return Some(Target::Prop(cell, i));
            }
        }
        self.object().map(|o| Target::Object(o.id()))
    }

    /// The corner a new cube of `zoom` takes flush against the face struck.
    /// On the axis of the face's normal, the cube starts just past the struck
    /// box: a placed cube's entry as it is stored, which the hit's box only
    /// approximates; a decor's the box it claims, the hit's own. On the other
    /// axes, it takes the cell the hit point falls in.
    pub(super) fn adjacent_corner(&self, zoom: Zoom) -> Q3 {
        let hit = &self.hit;
        let new_size = units(zoom);
        let flush = match hit.kind {
            HitKind::Object(_) => self.object().map(|o| object_aabb(&o)),
            _ => None,
        };
        let Aabb {
            min: box_min,
            max: box_max,
        } = flush.unwrap_or(hit.cube);
        let mut out: V3 = [0.0; 3];

        for axis in 0..3 {
            let n = hit.normal[axis];
            out[axis] = if n > 0.0 {
                grid_ceil(box_max[axis], new_size)
            } else if n < 0.0 {
                grid_floor(box_min[axis], new_size) - new_size
            } else {
                grid_floor(hit.point[axis], new_size)
            };
        }

        world_to_canonical(&out)
    }

    /// The corner of the cell of `zoom` struck, for the eraser's dig into
    /// terrain. It follows `adjacent_corner` but stays inside the struck box
    /// on the normal's axis.
    #[expect(
        clippy::needless_range_loop,
        reason = "an axis indexes the hit's normal, its box and the corner at once"
    )]
    pub(super) fn struck_corner(&self, zoom: Zoom) -> Q3 {
        let hit = &self.hit;
        let size = units(zoom);
        let mut out: V3 = [0.0; 3];
        for axis in 0..3 {
            let n = hit.normal[axis];
            out[axis] = if n > 0.0 {
                grid_ceil(hit.cube.max[axis], size) - size
            } else if n < 0.0 {
                grid_floor(hit.cube.min[axis], size)
            } else {
                grid_floor(hit.point[axis], size)
            };
        }
        world_to_canonical(&out)
    }
}

/// What the hover picks at `ndc_x`, `ndc_y`. The ray is cast from the
/// camera's pose as it is now, which a door event or a controller may have
/// moved since the turn composed the view.
pub(crate) fn pick_at(ndc_x: f64, ndc_y: f64) -> Option<Pick> {
    let pose = controller::pose()?;
    let (mut origin, mut dir) = ([0.0; 3], [0.0; 3]);
    ray_of(pose, ndc_x, ndc_y, &mut origin, &mut dir);
    let mut p = Pick {
        hit: ray_pick(origin, dir)?,
        ray_zoom: None,
    };
    p.ray_zoom = eye().map(|eye| {
        let v = v3::sub(&p.cell().center(), &eye);
        query::by_dist().at(v3::dot(&v, &v) as f32)
    });
    Some(p)
}

/// The first thing the ray from the eye at `origin` along `dir` strikes
/// within `PICK_DIST_U`, in the root frame. Decor is struck as the frame
/// draws it (`World::striking_decor`). The world asked is the stored one
/// (`World::now`), not the one the eraser's preview draws, because what is
/// erased is what is stored.
pub fn ray_pick(origin: V3, dir: V3) -> Option<Hit> {
    let ray = Ray {
        origin,
        dir,
        t_max: PICK_DIST_U,
    };
    // Only an exact answer counts. A coarser tile's answer is a summary, not
    // what is held.
    World::now()
        .striking_decor(&decor_zooms(settings().decor))
        .first_hit(&ray, finest())
        .exact()
        .flatten()
}

#[cfg(test)]
#[path = "pick.test.rs"]
mod test;
