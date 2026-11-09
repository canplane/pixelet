// pxl-core/domain/query/ray.rs

//! The first cell a ray meets, at the zoom its distance asks. The descent is
//! pxl-engine's (`cast::first`), the same one the GPU's cast runs. Here the
//! world answers it from everything it holds, each node handed what its
//! parent found (`CastWorld`), the decor standing in a cell among it. What stops
//! a ray is the asker's: a body passes a decor and an eye stops at it
//! (`Until`).

use super::change;
use super::decor::cell_box;
use super::node::{coarsest, At, NodeFn};
use super::world::World;
use crate::domain::layer::instance::Instance;
use crate::domain::layer::tile_cache::Path;
use crate::domain::layer::{Answer, Base};
use pxl_engine::answer::{self, In, Kind};
pub(super) use pxl_engine::cast::Until;
use pxl_engine::cast::{self, Detail, End, Keep, Line, Query, Spacing, Trace, DEPTH};
use pxl_engine::object;
use pxl_engine::support;
use pxl_engine::support::ColumnSurface;
use pxl_spec::asset::decor::DECORS;
use pxl_spec::asset::id::{self, kind_of};
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::camera::grid;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world::{nearest_box, origin, wrap_cell};
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::octant::Octant;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

/// The zooms a cell is asked at: no coarser than the cells a tile draws, and
/// no finer than what is placed is held.
pub fn zoom_range() -> [Zoom; 2] {
    [coarsest(), OBJECT_FINEST]
}

/// The level of detail (pxl-engine `cast::Detail`) that reads the cells of
/// tiles at zoom `tile` at every distance: a range of that one cell zoom,
/// held within `zoom_range`.
pub fn at_zoom(tile: Zoom) -> Detail {
    let [lo, hi] = zoom_range();
    let z = GROUND.cell_zoom(tile).clamp(lo, hi);
    Detail {
        range: [z, z],
        spacing: Spacing::NONE,
    }
}

/// The level of detail that reads what is held as finely as it is held: the
/// ground at its cells, which answer the same when asked finer, and what is
/// placed at its own zoom. Where the eye draws something coarser, the asker
/// judges that by `by_dist` (domain/control/edit/pick.rs `Pick`).
pub fn finest() -> Detail {
    let [_, finest] = zoom_range();
    Detail {
        range: [finest, finest],
        spacing: Spacing::NONE,
    }
}

/// The camera grid's level of detail: at each distance from the ray's
/// origin, the cell zoom whose cell, drawn whole, spans at least one grid
/// cell from any direction (pxl-spec space/camera/grid.rs `cell_zoom_at`).
/// This is the rule that sizes every drawn cell.
pub fn by_dist() -> Detail {
    Detail {
        range: zoom_range(),
        spacing: Spacing::NONE,
    }
}

/// The cell zoom the eye's rays draw at `at`: the camera grid's rule at its
/// distance from `eye` (pxl-spec space/camera/grid.rs `cell_zoom_at`), no
/// finer than the ground's leaf cells and, once the source declares its top,
/// no coarser than the cells a tile draws.
pub fn cell_zoom_at(eye: V3, at: V3) -> Zoom {
    let v = v3::sub(&at, &eye);
    let cell_zoom = grid::cell_zoom_at(v3::dot(&v, &v) as f32).min(GROUND.leaf_cell_zoom());
    TerrainTile::min_zoom().map_or(cell_zoom, |min| cell_zoom.max(GROUND.grid_zoom(min)))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub origin: V3,
    pub dir: V3,
    pub t_max: f64,
}

impl Ray {
    pub fn at(&self, t: f64) -> V3 {
        std::array::from_fn(|k| self.origin[k] + self.dir[k] * t)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HitKind {
    Ground,
    Object(Octant),
    /// Objects folded into a cell above the leaves; no single object is met.
    Objects,
    /// A decor thing, struck as the box it claims, whether the ground
    /// generated it or it was placed.
    Decor(Instance),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
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
    node_fn: NodeFn,
    /// Of what the ground generates, the decors the asker sees, one bit a
    /// decor in `DECORS`' order. Any other is air to it.
    decors: u32,
    /// An eye's ray. What it starts inside is not met, and a decor is its
    /// whole box, so a decor whose box the ray does not enter is air to it.
    eye_ray: Option<&'w Ray>,
    lack: Option<Quadrant>,
    answered: bool,
    /// The ground's lines the walk read. Each node's line steps on from them
    /// (`World::node`).
    path: Path,
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
    /// it lacked nothing, nothing with the first tile lacked where it met
    /// nothing and no node answered, and otherwise coarser with that tile.
    fn answer<T>(&self, v: Option<T>) -> Answer<Option<T>> {
        match (v, self.lack, self.answered) {
            (v, None, _) => Answer::Exact(v),
            (None, Some(l), false) => Answer::Nothing(l),
            (v, Some(l), _) => Answer::Coarser(v, l),
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
        self.world
            .child(at, (p[0] & 1 | (p[1] & 1) << 1 | (p[2] & 1) << 2) as usize)
    }

    /// The world's answer (`ask`), keeping the first tile lacked. A node that
    /// nothing under fills is air of its size. A decor the asker does not
    /// see, one it leaves out (`decors`) or whose box it does not enter
    /// (`eye`), is air to it.
    fn node(
        &mut self,
        at: &mut At,
        _: Zoom,
        _: [i32; 3],
        zooms: [Zoom; 2],
        _: f32,
    ) -> In<[u32; 2]> {
        let answer = match (self.node_fn)(self.world, at, zooms, &mut self.path) {
            Answer::Nothing(l) => {
                self.lack.get_or_insert(l);
                return In {
                    kind: Kind::Lacks,
                    asset: answer::EMPTY,
                };
            }
            Answer::Coarser(i, l) => {
                self.lack.get_or_insert(l);
                i
            }
            Answer::Exact(i) => i,
        };
        self.answered = true;
        let Some(k) = DECORS
            .iter()
            .position(|d| d.id == answer::val(answer.asset[0]))
            .filter(|_| answer.kind == Kind::Cell)
        else {
            return answer;
        };
        let unseen = !answer::is_placed(answer.asset[0]) && self.decors >> k & 1 == 0;
        if unseen
            || self
                .eye_ray
                .is_some_and(|ray| !self.world.ray_enters_decor(ray, at.node))
        {
            return In {
                kind: Kind::Cell,
                asset: answer::EMPTY,
            };
        }
        answer
    }
}

impl World {
    /// The first cell that stops a body along `ray` within its `t_max`, at
    /// the zoom `detail` asks where it meets it (`Until::Solid`): no decor or
    /// marker is met. What the ray starts inside is not met, so an eye under
    /// the ground is not stopped where it stands. A cell met coarser than it
    /// is held comes with the tile lacked. A cell not held at all is lacked
    /// too, and past it the ray tells nothing.
    pub fn first_hit(&self, ray: &Ray, detail: Detail) -> Answer<Option<Hit>> {
        let mut path = Path::default();
        self.along(ray, detail, Until::Solid, u32::MAX, &mut path, &mut None)
    }

    /// The first thing a viewer sees along `ray`, the same walk stopping at a
    /// decor as the frame draws it: everything placed and, of what the ground
    /// generates at the leaves, `decors` (`lens::leaf_decors`).
    pub fn first_visible(&self, ray: &Ray, detail: Detail, decors: u32) -> Answer<Option<Hit>> {
        let mut path = Path::default();
        self.along(ray, detail, Until::Opaque, decors, &mut path, &mut None)
    }

    /// The first cell along `ray` that `until` stops at, at the zoom `detail`
    /// asks where it meets it (pxl-engine `cast::first`). The ray starts at
    /// its origin, which may stand off the world, as an eye over it does
    /// (pxl-spec `world::origin`). Of what the ground generates, it sees
    /// `decors` (`CastWorld::decors`). The tiles' lines step on from `path`, the
    /// ground's lines that a walk beside this ray read over the same tile
    /// cache, and are left there for the next ray. A walk from the origin
    /// `descent` names goes on from where that one ended, its asker holding
    /// what the descent to there lacked, and leaves its own end there.
    pub(super) fn along(
        &self,
        ray: &Ray,
        detail: Detail,
        until: Until,
        decors: u32,
        path: &mut Path,
        descent: &mut Option<Descent>,
    ) -> Answer<Option<Hit>> {
        let mut query = CastWorld {
            world: self,
            node_fn: if until == Until::Opaque {
                World::node
            } else {
                World::layer_node
            },
            decors,
            eye_ray: (until == Until::Opaque).then_some(ray),
            lack: None,
            answered: false,
            path: *path,
        };
        let o = ray.origin;
        let end = ray.at(ray.t_max);
        change::report_read([o[0], o[1]], [end[0], end[1]], detail.range[0]);
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
        let hit = (trace.end == End::Hit).then(|| self.hit(ray, q, until, trace.hit));
        query.answer(hit)
    }

    /// The column surface down column `g` at `cell_zoom` (pxl-engine
    /// `support::column_surface`), of what the layers compose: what a decor
    /// or a body stands on lies under the decor. The first value is the top of the cell
    /// met, as drawn, with what the world lacked on the way, or None where
    /// the column meets nothing opaque. The second is the column surface
    /// itself, off whose line the cells around it are asked (`asset_about`).
    pub(super) fn column_surface(
        &self,
        cell_zoom: Zoom,
        g: [i32; 2],
    ) -> (Answer<Option<(i32, f32)>>, ColumnSurface<At>) {
        let mut query = self.cast_world();
        let side = units(cell_zoom);
        let middle = g.map(|v| (f64::from(v) + 0.5) * side);
        change::report_read(middle, middle, cell_zoom);
        let f = support::column_surface(&mut query, cell_zoom, g);
        (query.answer((f.end == End::Hit).then_some(f.top)), f)
    }

    /// What is in the cell `c` at `zoom`, asked off the line `l`
    /// (pxl-engine `support::asset`). The descent starts from the deepest node
    /// on the line that holds the cell, never again from the top. Where the
    /// world lacks the cell, the answer is nothing with the missing tile.
    pub(super) fn asset_about(&self, l: &Line<At>, zoom: Zoom, c: [i32; 3]) -> Answer<u32> {
        let mut query = self.cast_world();
        let val = support::asset(&mut query, l, zoom, c);
        match query.lack {
            None => Answer::Exact(val),
            Some(t) => Answer::Nothing(t),
        }
    }

    /// A query of what the layers compose under the decor.
    fn cast_world(&self) -> CastWorld<'_> {
        CastWorld {
            world: self,
            node_fn: World::layer_node,
            decors: u32::MAX,
            eye_ray: None,
            lack: None,
            answered: false,
            path: Path::default(),
        }
    }

    /// The hit where the descent from `q` stopped, in units; `q` is the ray's
    /// origin as a position names it. The node's box is placed relative to
    /// the unit cell holding the origin, so it lies on the origin's lap, and
    /// is inset to the box its cell fills.
    fn hit(&self, ray: &Ray, q: Q3, until: Until, s: cast::Hit<At>) -> Hit {
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
        // A decor seen is struck as the box it claims, at the face the ray
        // enters, so what is placed on it stands on that box (pxl-spec
        // asset/id.rs `Kind::stood_on`). A body's climb out of what fills it
        // stops at the cell of a decor it may stand inside.
        if until == Until::Opaque && kind_of(answer::val(s.asset[0])) == Some(id::Kind::decor) {
            let i = self.decor_over((zoom, c)).or_bug(Errno::ENOTRECOVERABLE);
            let cube = nearest_box(&cell_box(&i).or_bug(Errno::ENOTRECOVERABLE), ray.origin[0]);
            let (t, k) = cube
                .enters(ray.origin, ray.dir)
                .unwrap_or((f64::from(s.t), axis));
            let mut normal = [0.0; 3];
            normal[k] = if ray.dir[k] > 0.0 { -1.0 } else { 1.0 };
            return Hit {
                t,
                point: ray.at(t),
                normal,
                cube,
                kind: HitKind::Decor(i),
            };
        }
        let kind = match answer::is_placed(s.asset[0]) {
            true => self
                .object
                .entry_at(zoom, wrap_cell(zoom, c), s.at.inheritance.object)
                .map_or(HitKind::Objects, |e| HitKind::Object(e.id())),
            false => HitKind::Ground,
        };
        // A cell filled short of its cube is hit as the box it is drawn as,
        // laid about its cube's center (pxl-engine `cast::fill_bounds`).
        let (lo, hi) = cast::fill_bounds(([0.0; 3], [1.0; 3]), s.asset[0]);
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
