// pxl-kernel/world/observation.rs

//! The kernel's side of an observation (pxl-spec space/observation.rs): the
//! eye as the view draws it, what it looks at, and what the kernel knows of
//! what a box of the world holds — the bounds an estimator of demand and the
//! view's domain are handed. Nothing here knows how what is observed is
//! drawn.

use std::cmp::Ordering;

use pxl_spec::bake::format::Terrain;
use pxl_spec::space::hierarchy::{ancestor_of, Scale};
use pxl_spec::space::indexer::{L2, L3};
use pxl_spec::space::lod::Tau;
use pxl_spec::space::lod::{self, fault_radius};
use pxl_spec::space::observation::{band, Bounds, Contents, DemandEstimator, Frustum, Observation, Requirement};
use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::pvf3::REACH;
use pxl_spec::space::pyramid::{span_of, LevelPage, GROUND, TOP_LEVEL};
use pxl_spec::space::v3::Aabb;

use crate::kernel::kernel;
use crate::layer::environment::fog::{eye_height, reach_at};
use crate::layer::environment::state::globe;
use crate::layer::object::store::objects_revision;
use crate::shell::map_store::map_store;
use crate::world::access::Access;
use crate::world::frame::view::view;
use crate::world::observer::Sight;
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

/// What the kernel knows of the world: the terrain's index lanes, from each
/// page or the nearest resident page over it (world/access.rs
/// `Access::relief`), and the objects, which are all resident. The ground is
/// stored flat and read by the node of its heightfield under a box; the
/// objects are asked their 3D bound in each node's box
/// (layer/object/store.rs `Store::bound_in`), so what asks sees only the
/// box's answer on every axis. Unknown where no terrain page over some of
/// the box is held.
///
/// A piece of ground drawn in a level's cells is off from what it stands for
/// by a cell across and as far as it rises over one up; an object folded
/// away is off by all of itself, which a far pole is and what a sheer face
/// is taken to be.
pub struct Known;

impl Bounds for Known {
    fn contents(&self, level: u32, region: Aabb) -> Contents {
        let (z0, z1) = (region.min[2], region.max[2]);
        let cut = |(lo, hi): (f64, f64)| (lo <= z1 && hi >= z0).then(|| (lo.max(z0), hi.min(z1)));
        let cell = GROUND.scale(level).side();
        let mut known = true;
        let mut bound: Option<Aabb> = None;
        let mut rise = 0.0f64;
        under(level, region, &mut |node, square| {
            let Some(n) = read(level, node) else {
                known = false;
                return;
            };
            let mut hold = |(lo, hi): (f64, f64)| {
                let b = Aabb {
                    min: [square.min[0], square.min[1], lo],
                    max: [square.max[0], square.max[1], hi],
                };
                bound = Some(bound.map_or(b, |a| a.join(b)));
            };
            if let Some(ground) = cut(n.ground) {
                hold(ground);
                rise = rise.max(n.slope * cell);
            }
            if let Some(objects) = kernel().objects.bound_in(&square) {
                bound = Some(bound.map_or(objects, |a| a.join(objects)));
                rise = rise.max(objects.max[2] - objects.min[2]);
            }
        });
        match (known, bound) {
            (false, _) => Contents::Unknown,
            (true, None) => Contents::Empty,
            (true, Some(bound)) => {
                let extent = region.extent();
                let error = [cell, cell, rise];
                Contents::Holds {
                    bound,
                    error: std::array::from_fn(|k| error[k].clamp(0.0, extent[k])),
                }
            }
        }
    }
}

/// What the ground's store says of a node of its own: its bound and its
/// steepest rise over a run.
#[derive(Clone, Copy)]
struct Node {
    ground: (f64, f64),
    slope: f64,
}

/// What is known of `node` at `level`: none where no terrain page over it is
/// resident. Read afresh each time from the block of the index lane the node
/// spans (pxl-spec bake/format.rs `Index`), made as its page arrived and gone
/// with it.
fn read(level: u32, node: (Scale, [i64; 2])) -> Option<Node> {
    let page = L3.up(level as i32);
    let [x, y] = ancestor_of(node.1, page.above(node.0) as u32);
    let page = LevelPage::new(level, ivec::gather(x, y, 0));
    let relief = Access::now().relief(page, node).value()?;
    Some(Node {
        ground: relief.extent,
        slope: relief.slope,
    })
}

/// Each node of the stores' flat lattice at `level` under `region`, with the
/// square of it the region covers: the region's own square where it is one
/// node — a chunk's column up to a page — and its columns otherwise.
fn under(level: u32, region: Aabb, each: &mut dyn FnMut((Scale, [i64; 2]), Aabb)) {
    let column = L2.up(level as i32);
    let side = region.max[0] - region.min[0];
    let node = (0..=L3.above(L2)).map(|k| column.up(k)).find(|s| s.side() == side && region.max[1] - region.min[1] == side);
    let square = |s: Scale, [x, y]: [i64; 2]| {
        let (x0, y0) = (x as f64 * s.side(), y as f64 * s.side());
        let covered = Aabb {
            min: [x0, y0, region.min[2]],
            max: [x0 + s.side(), y0 + s.side(), region.max[2]],
        };
        covered.meet(region).unwrap_or(covered)
    };
    if let Some(s) = node.filter(|s| region.min[0] % s.side() == 0.0 && region.min[1] % s.side() == 0.0) {
        let at = [(region.min[0] / s.side()) as i64, (region.min[1] / s.side()) as i64];
        return each((s, at), square(s, at));
    }
    let rows = |k: usize| (region.min[k] / column.side()).floor() as i64..=((region.max[k] / column.side()).ceil() as i64 - 1);
    for y in rows(1) {
        for x in rows(0) {
            each((column, [x, y]), square(column, [x, y]));
        }
    }
}

/// The estimator from what the screen's tiles saw (world/observer.rs
/// `Sight`), at a quality `tau`: the ground their footprints touch, at each
/// level whose band some of it lies in, and what their rays lacked. Ground
/// no tile's rays reach — behind what they met, or behind the eye — is
/// required of no level; the floor answers it (world/paging/floor.rs).
///
/// A footprint is what the resident records say a tile meets. Where they
/// said it coarser than asked, or not at all, the ray lacked the ground at
/// the level it asked, and that lack is required; when it comes, the tiles
/// over it are cast again (`Sight::observe`), so the requirements grow
/// toward what the exact records would make them and never rest short of
/// it. What is required is the ground and the level — the lack as a
/// region, never as a page, which each layer's storage names for itself.
///
/// The bands are every estimator's (pxl-spec space/observation.rs `band`),
/// so nothing is required nearer than its level allows or past the band's
/// end, the fault bound holds as it does of `ByBounds`, and a lack asked at
/// a finer quality than `tau` is required at the level `tau` puts its
/// ground at, never finer than what is drawn.
pub struct ByFootprints<'a> {
    pub tau: Tau,
    pub sight: &'a Sight,
}

impl DemandEstimator for ByFootprints<'_> {
    fn requirements(&self, o: &Observation, bounds: &impl Bounds, each: &mut dyn FnMut(Requirement)) {
        let column = |x0: f64, y0: f64, x1: f64, y1: f64| Aabb {
            min: [x0, y0, REACH.min[2]],
            max: [x1, y1, REACH.max[2]],
        };
        let required = |level: u32, region: Aabb| {
            let (from, to) = band(o, self.tau, level)?;
            let held = bounds.contents(level, region).within(region)?;
            let m = o.measure(&held);
            (m.seen && m.near < to && m.near <= o.far && m.far >= from).then_some(Requirement { level, region })
        };
        for lack in self.sight.lacks() {
            let r = lack.region;
            let region = column(r.min[0], r.min[1], r.max[0], r.max[1]);
            if let Some(r) = (lack.level..=o.top).find_map(|level| required(level, region)) {
                each(r);
            }
        }
        for f in self.sight.footprints() {
            let b = f.aabb();
            for level in 0..=o.top {
                let Some((_, to)) = band(o, self.tau, level) else { break };
                // Only what lies within the band's end along the ground.
                let reach = to.min(o.far);
                let lo = [b.min[0].max(o.eye[0] - reach), b.min[1].max(o.eye[1] - reach)];
                let hi = [b.max[0].min(o.eye[0] + reach), b.max[1].min(o.eye[1] + reach)];
                if lo[0] > hi[0] || lo[1] > hi[1] {
                    continue;
                }
                let span = span_of(level);
                let rows = |k: usize| (lo[k] / span).floor() as i64..=(hi[k] / span).floor() as i64;
                for y in rows(1) {
                    for x in rows(0) {
                        let (x0, y0) = (x as f64 * span, y as f64 * span);
                        let square = column(x0, y0, x0 + span, y0 + span);
                        if f.touches(&square) {
                            if let Some(r) = required(level, square) {
                                each(r);
                            }
                        }
                    }
                }
            }
        }
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

    /// Where box `b` falls for `o`.
    pub fn of(o: &Observation, b: &Aabb) -> Order {
        let m = o.measure(b);
        Order { seen: m.seen, near: m.near }
    }

    /// Where `region` falls for `o`, by what `bounds` say it holds at
    /// `level`: all of it where nothing is known.
    pub fn of_region<B: Bounds + ?Sized>(o: &Observation, bounds: &B, level: u32, region: Aabb) -> Order {
        Order::of(o, &bounds.contents(level, region).within(region).unwrap_or(region))
    }

    /// Where page `p` falls for `o`, by what is known of what stands over
    /// it.
    pub fn of_page(o: &Observation, p: LevelPage) -> Order {
        Order::of_region(o, &Known, p.level, p.region())
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
