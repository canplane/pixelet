// pxl-kernel/world/observation.rs

//! The kernel's side of an observation (pxl-spec space/observation.rs): the
//! eye as the view draws it, what it looks at, and what the kernel knows of
//! the heights over the ground — the bounds an estimator of demand is
//! handed. Nothing here knows how what is observed is drawn.

use std::cmp::Ordering;

use pxl_spec::bake::format::Terrain;
use pxl_spec::space::lod::{self, fault_radius};
use pxl_spec::space::observation::{Bounds, Frustum, Observation, Paged, Rect, Requirement, Storage as _};
use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::pyramid::{LevelPage, TOP_LEVEL};

use crate::kernel::kernel;
use crate::layer::environment::fog::{eye_height, reach_at};
use crate::layer::environment::state::globe;
use crate::layer::object::store::objects_revision;
use crate::layer::terrain::extent::extent_under;
use crate::shell::map_store::map_store;
use crate::world::frame::view::view;
use crate::world::paging::floor;
use crate::world::pyramid::Layer as _;

/// Where the eye is as the view draws it, in units, once there is a map to
/// stand over.
pub fn eye() -> Option<[f64; 3]> {
    map_store().map_center.as_ref()?;
    let v = view();
    Some([
        ivec::to_real(ivec::X(v.origin)) + v.eye_local[0],
        ivec::to_real(ivec::Y(v.origin)) + v.eye_local[1],
        ivec::to_real(ivec::Z(v.origin)) + v.eye_local[2],
    ])
}

/// The level the ground under the eye wants, z16 at the finest: what the
/// floating origin is floored to (world/frame/coord.rs `origin_for`), so a
/// rebase is as far on screen at every height.
pub fn ground_level() -> u32 {
    eye().map_or(0, |_| lod::level(eye_height()).clamp(0, TOP_LEVEL as i32) as u32)
}

/// The view's frustum in world units. The view's planes are in its own
/// frame, which reaches world units as `root · (x − origin)`; a plane
/// `n · x + d` there is `(Rᵀn) · x + d + n · t − (Rᵀn) · origin` here, the
/// root being a rotation R and a translation t.
pub fn frustum_of_view() -> Frustum {
    let v = view();
    let r = v.root;
    let o = [ivec::to_real(ivec::X(v.origin)), ivec::to_real(ivec::Y(v.origin)), ivec::to_real(ivec::Z(v.origin))];
    let mut planes = [0.0; 24];
    for (k, p) in v.planes.as_chunks::<4>().0.iter().enumerate() {
        let n = [
            r[0] * p[0] + r[1] * p[1] + r[2] * p[2],
            r[4] * p[0] + r[5] * p[1] + r[6] * p[2],
            r[8] * p[0] + r[9] * p[1] + r[10] * p[2],
        ];
        let d = p[3] + p[0] * r[12] + p[1] * r[13] + p[2] * r[14] - (n[0] * o[0] + n[1] * o[1] + n[2] * o[2]);
        planes[k * 4..k * 4 + 4].copy_from_slice(&[n[0], n[1], n[2], d]);
    }
    Frustum { planes }
}

/// The eye's observation as the view makes it: what the frustum looks at,
/// out to where the air ends the ground or the end of the coarsest level the
/// tile source supplies whole, whichever is nearer. None before the source
/// has declared that level (world/paging/floor.rs): nothing is observed of a
/// source whose floor is not known.
pub fn observation() -> Option<Observation> {
    let eye = eye()?;
    let top = floor::level()?;
    let height = eye_height();
    Some(Observation {
        eye,
        frustum: Some(frustum_of_view()),
        far: reach_at(height).min(fault_radius(top)),
        top,
        planet: globe().radius_u(),
        ground: eye[2] - height,
    })
}

/// What the kernel knows of the heights over the ground: the terrain's index
/// lanes, from each page or the nearest resident page over it
/// (layer/terrain/extent.rs), and the objects standing there, which are all
/// resident. None where no terrain page over the ground is held.
pub struct Known;

impl Bounds for Known {
    fn heights(&self, level: u32, ground: Rect) -> Option<(f64, f64)> {
        let mut out = Some((f64::INFINITY, f64::NEG_INFINITY));
        Paged.pages(&Requirement { level, ground }, &mut |p| {
            let under = extent_under(p);
            let objects = kernel().objects.reach_over(p.node());
            out = match (out, under) {
                (Some(a), Some(g)) => {
                    let (lo, hi) = objects.map_or(g, |o| (g.0.min(o.0), g.1.max(o.1)));
                    Some((a.0.min(lo), a.1.max(hi)))
                }
                _ => None,
            };
        });
        out
    }
}

/// Moves whenever what the kernel knows of the heights does — a terrain
/// page came or went, and its lanes with it, or an object was written — so
/// whatever was measured against them before is known to be stale.
pub fn heights_revision() -> (u32, u32) {
    (Terrain::pyramid().revision(), objects_revision())
}

/// Where a piece of what is observed falls in its one order: what the view
/// sees before what it does not, and within each the nearer the eye first.
/// Every layer's work is taken in it: a page fetched, a chunk mounted and
/// built, and, the other way round, a record or a slot given up.
#[derive(Clone, Copy, Debug)]
pub struct Order {
    pub seen: bool,
    /// How near the eye its box comes, in units.
    pub near: f64,
}

impl Order {
    /// Nothing is ordered after this: what no view measured.
    pub const LAST: Order = Order { seen: false, near: f64::INFINITY };

    /// Where a box of ground and the heights it spans falls for `o`.
    pub fn of(o: &Observation, ground: Rect, z: Option<(f64, f64)>) -> Order {
        let m = o.measure(ground, z);
        Order { seen: m.seen, near: m.near }
    }

    /// Where page `p` falls for `o`, by its whole ground and what is known of
    /// its heights.
    pub fn of_page(o: &Observation, p: LevelPage) -> Order {
        Order::of(o, p.rect(), Known.heights(p.level, p.rect()))
    }
}

impl Ord for Order {
    fn cmp(&self, other: &Order) -> Ordering {
        other.seen.cmp(&self.seen).then(self.near.total_cmp(&other.near))
    }
}

impl PartialOrd for Order {
    fn partial_cmp(&self, other: &Order) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Order {
    fn eq(&self, other: &Order) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Order {}

#[cfg(test)]
#[path = "observation.test.rs"]
mod test;
