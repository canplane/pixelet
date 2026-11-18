// pxl-core/domain/query/ray.rs

//! The first cell a ray meets, each node asked at the zooms the walk is
//! given: one size for a body's cell, the eye's at each node's distance for
//! a ray across what the eye sees. The descent is pxl-engine's
//! (`cast::first`), the same one the GPU's cast runs. Here the
//! world answers it from everything it holds, each node handed what its
//! parent found (`CastWorld`). A viewer's walk answers the decor it sees too
//! (`Viewer`), and a body's the layers alone.

use super::change;
use super::decor::claim;
use super::detail::{detail, leaf_at};
use super::node::At;
use super::world::World;
use crate::domain::layer::Answer;
use crate::domain::layer::instance::Instance;
use crate::domain::layer::tile_cache::Path;
use pxl_engine::answer::{self, In, Kind};
pub(super) use pxl_engine::cast::Until;
use pxl_engine::cast::{self, DEPTH, Detail, End, Keep, Line, Query, Trace};
use pxl_engine::object;
use pxl_engine::support;
use pxl_engine::support::Surface;
use pxl_spec::asset::id::{self, kind_of};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::{nearest_box, origin, wrap_cell};
use pxl_spec::space::coord::zoom::{Zoom, units};
use pxl_spec::space::hierarchy::tree::child_idx;
use pxl_spec::space::layer::octant::Octant;
use pxl_spec::space::layer::quadrant::Quadrant;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Ray {
    pub origin: V3,
    pub dir: V3,
    pub t_max: f64,
}

impl Ray {
    pub(crate) fn at(&self, t: f64) -> V3 {
        std::array::from_fn(|k| self.origin[k] + self.dir[k] * t)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum HitKind {
    Ground,
    Object(Octant),
    /// Objects folded into a cell above the leaves; no single object is met.
    Objects,
    /// A decor thing, struck as the box it claims, whether the ground
    /// generated it or it was placed.
    Decor(Instance),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Hit {
    pub t: f64,
    pub point: V3,
    pub normal: V3,
    /// The box struck.
    pub cube: Aabb,
    pub kind: HitKind,
}

/// The world as the descent asks it, with what only the CPU carries along
/// a ray: the first tile lacked, and whether any node was answered.
pub(super) struct CastWorld<'w> {
    world: &'w World,
    /// Who is looking, where the walk is a viewer's; a body sees no decor.
    viewer: Option<Viewer<'w>>,
    lack: Option<Quadrant>,
    answered: bool,
    /// The ground's lines the walk read. Each node's line steps on from them
    /// (`World::node`).
    path: Path,
}

/// A viewer along a ray: what it sees of the decor.
pub(super) struct Viewer<'w> {
    pub(super) ray: &'w Ray,
    /// Of what the ground generates, the decors it sees, one bit a decor in
    /// `DECORS`' order; everything placed it sees.
    pub(super) decors: u32,
    /// The decor it last saw, struck as the box it is drawn as, at the face
    /// the ray enters, so what is placed on it stands on that box (pxl-spec
    /// asset/id.rs `Kind::stood_on`).
    pub(super) met: Option<Hit>,
}

impl Viewer<'_> {
    /// Whether it sees `i`: a decor it is shown, whose box its ray enters.
    /// Holds the strike for the walk's end.
    pub(super) fn sees(&mut self, i: &Instance) -> bool {
        let Ray { origin, dir, .. } = *self.ray;
        let shown = i.generated_in().is_none() || self.decors >> id::idx_of(i.id) & 1 == 1;
        let met = claim(i)
            .filter(|_| shown)
            .map(|b| nearest_box(&b, origin[0]))
            .and_then(|cube| {
                let (t, k) = cube.enters(origin, dir)?;
                let mut normal = [0.0; 3];
                normal[k] = if dir[k] > 0.0 { -1.0 } else { 1.0 };
                Some(Hit {
                    t,
                    point: self.ray.at(t),
                    normal,
                    cube,
                    kind: HitKind::Decor(*i),
                })
            });
        self.met = met.or(self.met);
        met.is_some()
    }
}

/// Where a walk from `origin` ended, for the next walk from that origin to go
/// on from there and not from the top (`World::along`, layer/README.md's
/// cursor).
#[derive(Clone, Copy)]
pub(super) struct Descent {
    origin: [i32; 3],
    node: cast::Node,
    line: Line<At>,
}

impl Descent {
    /// The node of the line that holds where `ray` starts, with what was
    /// handed it, or None where the walk ended past the origin's root.
    fn start(&self, ray: &cast::Ray) -> Option<(cast::Node, At, f32)> {
        let (over, found) = self.node.over(cast::point_at(self.origin, ray, 0.0));
        found.then(|| {
            let n = cast::holding(self.origin, ray, over, 0.0);
            (n, self.line.ancestor(n.zoom), 0.0)
        })
    }
}

/// Leaves where a walk from an origin ended.
struct KeepDescent<'e>(&'e mut Option<Descent>, [i32; 3]);

impl Keep<At> for KeepDescent<'_> {
    fn keep(&mut self, trace: &Trace<At>, stack: &[At; DEPTH]) {
        let (origin, node) = (self.1, trace.hit.node);
        *self.0 = Some(Descent {
            origin,
            node,
            line: Line::of_walk(origin, &node, trace.hit.at, stack),
        });
    }
}

impl CastWorld<'_> {
    /// `v`, what the walk met or none, as what the walk held says: exact where
    /// it lacked nothing, a lack with the first tile lacked where it met
    /// nothing and no node answered, and otherwise coarser with that tile.
    fn answer<T>(&self, v: Option<T>) -> Answer<Option<T>> {
        match (v, self.lack, self.answered) {
            (v, None, _) => Answer::Exact(v),
            (None, Some(l), false) => Answer::Lack(l),
            (v, Some(l), _) => Answer::Coarser(v, l),
        }
    }
}

impl CastWorld<'_> {
    /// `answer` of a walk that ended as `end`: one that ended at what no
    /// node answers is a lack, whatever answered before it, since it did not
    /// go its reach.
    fn ended<T>(&self, v: Option<T>, end: End) -> Answer<Option<T>> {
        match (end, self.lack) {
            (End::Lack, Some(l)) => Answer::Lack(l),
            _ => self.answer(v),
        }
    }
}

impl Query for CastWorld<'_> {
    type At = At;

    fn root(&mut self, node: [i32; 3]) -> At {
        self.world.start((0, node))
    }

    /// The child is the one named by its place in the tree's cut
    /// (`object::path`).
    fn child(&mut self, at: &At, zoom: Zoom, node: [i32; 3]) -> At {
        let p = object::path(zoom, node);
        self.world.child(at, child_idx(p))
    }

    /// The world's answer, keeping the first tile lacked: with the decor a
    /// viewer sees, or for a body the layers alone. A node that nothing under
    /// fills is air of its size.
    fn node(
        &mut self,
        at: &mut At,
        _: Zoom,
        _: [i32; 3],
        range: [Zoom; 2],
        _: f32,
    ) -> In<[u32; 2]> {
        let answer = match &mut self.viewer {
            Some(v) => self
                .world
                .node(at, range, &mut self.path, &mut |i| v.sees(i)),
            None => self.world.layer_node(at, range, &mut self.path),
        };
        self.answered |= !matches!(answer, Answer::Lack(_));
        match answer {
            Answer::Lack(l) => {
                self.lack.get_or_insert(l);
                In {
                    kind: Kind::Lack,
                    asset: answer::EMPTY,
                }
            }
            Answer::Coarser(i, l) => {
                self.lack.get_or_insert(l);
                i
            }
            Answer::Exact(i) => i,
        }
    }
}

impl World {
    /// The first cell that stops a ray from the eye within its `t_max`
    /// (`seen`): the layers alone, so no decor is met. The orbit camera asks
    /// it of the lines its eye would go along and of the line from its pivot
    /// to its eye, and the profiler of the pointer's ray. What the ray
    /// starts inside is not met, so the eye under the ground is not stopped
    /// where it stands. A cell met coarser than it is held comes with
    /// the tile lacked. A cell not held at all is lacked too, and past it the
    /// ray tells nothing.
    pub(crate) fn first_hit(&self, ray: &Ray) -> Answer<Option<Hit>> {
        self.seen(ray, None)
    }

    /// The first thing the eye sees along `ray`, a ray from it as the
    /// pointer's pick casts (`seen`): everything placed and, of what the
    /// ground generates at the leaves, `decors` (`lens::leaf_decors`).
    pub(crate) fn first_seen(&self, ray: &Ray, decors: u32) -> Answer<Option<Hit>> {
        self.seen(ray, Some(decors))
    }

    /// `along` for a ray from the eye or along the eye's way, the pointer's
    /// and the orbit's lines: each node is asked at the
    /// zoom its own distance from the eye asks (`detail`), as a pixel's ray
    /// asks it, so the answer is of the eye where it stands (`change::eye`).
    /// With no eye nothing is observed, and it answers a lack.
    fn seen(&self, ray: &Ray, decors: Option<u32>) -> Answer<Option<Hit>> {
        let o = ray.origin;
        let Some(eye) = change::eye_at() else {
            return Answer::Lack(leaf_at(&origin(o[0], o[1], o[2])));
        };
        let mut path = Path::default();
        self.along(
            ray,
            detail(eye, o),
            Until::Opaque,
            decors,
            &mut path,
            &mut None,
        )
    }

    /// The first cell along `ray` that `until` stops at, each node asked at
    /// the zooms `detail` gives (pxl-engine `cast::first`): the eye's at the
    /// node's distance (`detail`), or one size wherever the ray goes
    /// (`one_zoom`), which is the one thing a body's ray and the eye's
    /// differ in. The ray starts at its origin, which may stand off the
    /// world, as the eye over it does (pxl-spec `world::origin`). With
    /// `decors` it is a viewer's (`Viewer`), and without them a body's. The
    /// tiles' lines step on from `path`, the ground's lines that a walk
    /// beside this ray read over the same tile cache, and are left there for
    /// the next ray. A walk from the origin `descent` names goes on from
    /// where that one ended, its asker holding what the descent to there
    /// lacked, and leaves its own end there.
    pub(super) fn along(
        &self,
        ray: &Ray,
        detail: Detail,
        until: Until,
        decors: Option<u32>,
        path: &mut Path,
        descent: &mut Option<Descent>,
    ) -> Answer<Option<Hit>> {
        let o = ray.origin;
        let mut query = CastWorld {
            viewer: decors.map(|decors| Viewer {
                ray,
                decors,
                met: None,
            }),
            path: *path,
            ..self.cast_world()
        };
        let from = cast::Ray::new(ray.dir.map(|v| v as f32), ray.t_max as f32);
        let q = origin(o[0], o[1], o[2]);
        let on = descent
            .filter(|e| e.origin == q)
            .and_then(|e| Some((e.start(&from)?, e.line)));
        let mut keep = KeepDescent(descent, q);
        let trace = match on {
            Some((start, line)) => {
                cast::walk(&mut query, q, &from, detail, until, start, &line, &mut keep)
            }
            None => cast::walk_from_root(&mut query, q, &from, detail, until, &mut keep),
        };
        *path = query.path;
        // What the walk read: the cells on its way as far as it went and the
        // one it ended in, the coarsest of them the one its detail asks at
        // the end of its way farther from the eye.
        let went = f64::from(trace.hit.t).clamp(0.0, ray.t_max);
        let zoom_at = |t: f64| {
            let e: [f32; 3] = core::array::from_fn(|k| detail.eye[k] - (ray.dir[k] * t) as f32);
            detail.at(e[0] * e[0] + e[1] * e[1] + e[2] * e[2])
        };
        let coarsest = zoom_at(0.0).min(zoom_at(went));
        let end = ray.at(went + units(coarsest) / 2.0);
        // The zooms asked are the walk's own, each node at one of them. A
        // viewer's walk reads the ground its decor stands on beside its own
        // nodes, which its lack does not tell of, so it counts as not exact.
        change::report_read(
            [o[0], o[1]],
            [end[0], end[1]],
            [coarsest, detail.range[1]],
            query.lack.is_none() && query.viewer.is_none(),
        );
        let met = query.viewer.as_ref().and_then(|v| v.met);
        let is_decor = kind_of(answer::val(trace.hit.asset[0])) == Some(id::Kind::Decor);
        let hit = (trace.end == End::Hit).then(|| {
            met.filter(|_| is_decor)
                .unwrap_or_else(|| self.hit(ray, q, trace.hit))
        });
        query.ended(hit, trace.end)
    }

    /// The surface over cell `c` at `cell_zoom` (pxl-engine
    /// `support::surface`), of what the layers compose: what a decor
    /// or a body stands on lies under the decor. The first value is the top of the cell
    /// met, as drawn, with what the world lacked on the way, or None where
    /// the ray down meets nothing opaque. The second is the surface
    /// itself, off whose line the cells around it are asked (`asset_about`).
    pub(super) fn surface(
        &self,
        cell_zoom: Zoom,
        c: [i32; 2],
    ) -> (Answer<Option<(i32, f32)>>, Surface<At>) {
        let mut query = self.cast_world();
        let side = units(cell_zoom);
        let middle = c.map(|v| (f64::from(v) + 0.5) * side);
        let f = support::surface(&mut query, cell_zoom, c);
        change::report_read(middle, middle, [cell_zoom; 2], query.lack.is_none());
        (query.ended((f.end == End::Hit).then_some(f.top), f.end), f)
    }

    /// What is in the cell `c` at `zoom`, asked off the line `l`
    /// (pxl-engine `support::asset`). The descent starts from the deepest node
    /// on the line that holds the cell, never again from the top. A coarser
    /// clip level that holds the cell answers it, coarser with the tile it
    /// lacked; where none holds it, the answer is a lack, as the GPU's
    /// `world_cell` answers `KIND_LACK`.
    pub(super) fn asset_about(&self, l: &Line<At>, zoom: Zoom, c: [i32; 3]) -> Answer<u32> {
        let mut query = self.cast_world();
        let val = support::asset(&mut query, l, zoom, c);
        query
            .answer(query.answered.then_some(val))
            .map(|v| v.unwrap_or(val))
    }

    /// A query of what the layers compose under the decor.
    fn cast_world(&self) -> CastWorld<'_> {
        CastWorld {
            world: self,
            viewer: None,
            lack: None,
            answered: false,
            path: Path::default(),
        }
    }

    /// The hit where the descent from `q` stopped, in units; `q` is the ray's
    /// origin as a position names it. The node's box is placed relative to
    /// the unit cell holding the origin, so it lies on the origin's lap, and
    /// is inset to the box its cell fills.
    fn hit(&self, ray: &Ray, q: Q3, s: cast::Hit<At>) -> Hit {
        let (zoom, c) = s.at.node;
        let side = units(zoom);
        let unit = ray.origin.map(libm::floor);
        // The corner's offset from the origin's unit cell, in exact Q3 steps:
        // the distance between the roots, plus the corner's place in its
        // root, less the origin's unit cell's place there.
        let n = s.node;
        let grid = cast::grid_of(q);
        let steps = |k: usize| {
            (i64::from(n.roots[k]) << 32) + i64::from(n.corner[k]) - i64::from(grid[k] & !0xff)
        };
        let min: V3 = std::array::from_fn(|k| unit[k] + q3::to_real(steps(k)));
        let axis = s.axis as usize;
        let mut normal = [0.0; 3];
        normal[axis] = if ray.dir[axis] > 0.0 { -1.0 } else { 1.0 };
        let kind = match answer::is_placed(s.asset[0]) {
            true => self
                .object
                .entry_at(zoom, wrap_cell(zoom, c), s.at.inheritance.object)
                .map_or(HitKind::Objects, |e| HitKind::Object(e.id())),
            false => HitKind::Ground,
        };
        // A cell filled short of its cube is hit as the box it is drawn as,
        // laid about its cube's center (pxl-engine `cast::fill_bound`).
        let (lo, hi) = cast::fill_bound(([0.0; 3], [1.0; 3]), s.asset[0]);
        let t = f64::from(s.t);
        Hit {
            t,
            point: ray.at(t),
            normal,
            cube: Aabb {
                min: std::array::from_fn(|k| min[k] + side * f64::from(lo[k])),
                max: std::array::from_fn(|k| min[k] + side * f64::from(hi[k])),
            },
            kind,
        }
    }
}
