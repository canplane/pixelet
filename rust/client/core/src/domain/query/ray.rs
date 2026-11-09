// pxl-core/domain/query/ray.rs

//! The first cell a ray meets, at the zoom its distance asks. The descent is
//! pxl-engine's (`cast::first`), the same one the GPU's cast runs. Here the
//! world answers it from everything it holds, each node handed what its
//! parent found (`Walker`). Decor, which only the CPU answers, rides on the
//! same descent.

use super::change;
use super::decor::DecorRay;
use super::node::{coarsest, At};
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
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::camera::grid;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world::{origin, wrap_cell};
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::octant::Octant;
use pxl_spec::space::layer::quadrant::Quadrant;

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
/// a ray: the decor struck, the first tile lacked, and whether any node was
/// answered.
pub(super) struct Walker<'w, 'a> {
    world: &'w World<'a>,
    lack: Option<Quadrant>,
    answered: bool,
    /// The decor struck along the ray, when the walk strikes decor at all.
    decor_ray: Option<(&'w Ray, DecorRay)>,
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

impl<'a> Query for Walker<'_, 'a> {
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

    /// The world's answer (`World::node`), keeping the first tile lacked. A
    /// node that nothing under fills is air of its size. Decor fills no cell,
    /// so it is handled apart. At each leaf cell the ray passes within the
    /// distance where decor is drawn as held, the ray strikes the decor
    /// standing near it. A node that no cell under fills is still descended
    /// into, down to the leaf cells, where decor may stand in it.
    fn node(
        &mut self,
        at: &mut At,
        _: Zoom,
        _: [i32; 3],
        zooms: [Zoom; 2],
        t0: f32,
    ) -> In<[u32; 2]> {
        let (zoom, c) = at.node;
        let t0 = f64::from(t0);
        // While decor is struck here, this tracks what is placed and where
        // the ground stands around the node, stepped on from what was around
        // its parent.
        if let Some((_, s)) = self.decor_ray.as_mut() {
            s.about.visit(&self.world.object.columns(), at.node);
            if t0 < s.draw_dist && s.about_zoom(zoom) == zoom {
                s.about.read_columns(self.world, (zoom, [c[0], c[1]]));
            }
        }
        if zoom == GROUND.leaf_cell_zoom() {
            if let Some((ray, s)) = self.decor_ray.as_mut().filter(|(_, s)| t0 < s.draw_dist) {
                self.world.intersect_near(s, ray, (zoom, c));
            }
        }
        let answer = match self.world.node(at, zooms, &mut self.path) {
            Answer::Nothing(l) => {
                self.lack.get_or_insert(l);
                return In {
                    kind: Kind::Lacks,
                    asset: answer::AIR,
                };
            }
            Answer::Coarser(i, l) => {
                self.lack.get_or_insert(l);
                i
            }
            Answer::Exact(i) => i,
        };
        self.answered = true;
        let decor = zoom < GROUND.leaf_cell_zoom()
            && self.decor_ray.as_ref().is_some_and(|(_, s)| {
                t0 < s.draw_dist && self.world.may_intersect(s, at.node, at.inheritance.shard)
            });
        match answer.kind {
            // Air coarser than asked is descended into where decor may stand
            // in it. A split is entered whole: its box is the world's, not the
            // box of the decor standing over it.
            Kind::Cell if zoom < zooms[1] && decor => In {
                kind: Kind::Split,
                asset: answer::BOX_WHOLE,
            },
            Kind::Split if decor => In {
                kind: Kind::Split,
                asset: answer::BOX_WHOLE,
            },
            _ => answer,
        }
    }
}

/// A decor struck at `t`, its box entered across `axis` (`decor_box`),
/// as the block it is. The hit is the box it claims and that face, as a
/// cell's hit is, so what is placed on it stands on that box (pxl-spec
/// asset/id.rs `Kind::stood_on`).
fn decor_hit(ray: &Ray, t: f64, axis: usize, i: Instance) -> Hit {
    let claimed = super::decor::claim(&i).unwrap_or_else(|| i.cell());
    let mut normal = [0.0; 3];
    normal[axis] = if ray.dir[axis] > 0.0 { -1.0 } else { 1.0 };
    Hit {
        t,
        point: ray.at(t),
        normal,
        cube: claimed,
        kind: HitKind::Decor(i),
    }
}

impl<'a> World<'a> {
    /// The first filled cell `ray` meets within its `t_max`, at the zoom
    /// `detail` asks where it meets it. What the ray starts inside is not met,
    /// so an eye under the ground is not stopped where it stands. A cell met
    /// coarser than it is held comes with the tile lacked. A cell not held at
    /// all is lacked too, and past it the ray tells nothing.
    pub fn first_hit(&self, ray: &Ray, detail: Detail) -> Answer<Option<Hit>> {
        let mut path = Path::default();
        self.along(ray, detail, Until::Opaque, None, &mut path, &mut None)
            .0
    }

    /// The first thing a viewer sees along `ray`: the first cell, or the
    /// decor struck nearer, as the frame draws it: everything placed and, of
    /// what the ground generates at the leaves, `kinds` (`lens::leaf_kinds`).
    /// Decor is struck at the leaf cells the ray passes.
    pub fn first_seen(&self, ray: &Ray, detail: Detail, kinds: u32) -> Answer<Option<Hit>> {
        let mut path = Path::default();
        let (met, decor_ray) = self.along(
            ray,
            detail,
            Until::Opaque,
            DecorRay::new(ray, kinds),
            &mut path,
            &mut None,
        );
        let decor = decor_ray
            .and_then(|s| s.best)
            .filter(|&(d, ..)| met.val().flatten().is_none_or(|h: Hit| d < h.t));
        match (decor, met) {
            (Some((d, i, k)), Answer::Nothing(l)) => {
                Answer::Coarser(Some(decor_hit(ray, d, k, i)), l)
            }
            (Some((d, i, k)), met) => met.map(|_| Some(decor_hit(ray, d, k, i))),
            (None, met) => met,
        }
    }

    /// The first cell along `ray` that `until` stops at, at the zoom `detail`
    /// asks where it meets it (pxl-engine `cast::first`). The ray starts at
    /// its origin, which may stand off the world, as an eye over it does
    /// (pxl-spec `world::origin`). The tiles' lines step on from `path`, the
    /// ground's lines that a walk beside this ray read over the same tile
    /// cache, and are left there for the next ray. A walk from the origin
    /// `descent` names goes on from where that one ended, its asker holding
    /// what the descent to there lacked, and leaves its own end there.
    pub(super) fn along(
        &self,
        ray: &Ray,
        detail: Detail,
        until: Until,
        decor_ray: Option<DecorRay>,
        path: &mut Path,
        descent: &mut Option<Descent>,
    ) -> (Answer<Option<Hit>>, Option<DecorRay>) {
        let mut walker = Walker {
            world: self,
            lack: None,
            answered: false,
            decor_ray: decor_ray.map(|s| (ray, s)),
            path: *path,
        };
        let o = ray.origin;
        let end = ray.at(ray.t_max);
        change::read([o[0], o[1]], [end[0], end[1]], detail.range[0]);
        let from = cast::Ray::new(ray.dir.map(|v| v as f32), ray.t_max as f32);
        let q = origin(o[0], o[1], o[2]);
        let on = descent
            .filter(|e| e.origin == q)
            .and_then(|e| Some((e.start(&from)?, e.line)));
        let mut keep = KeepDescent(descent, q);
        let trace = match on {
            Some((start, line)) => cast::walk(
                &mut walker,
                q,
                &from,
                detail,
                until,
                start,
                &line,
                &mut keep,
            ),
            None => cast::walk_from_root(&mut walker, q, &from, detail, until, &mut keep),
        };
        *path = walker.path;
        let hit = (trace.end == End::Hit).then(|| self.hit(ray, q, trace.hit));
        let answer = match (hit, walker.lack, walker.answered) {
            (h, None, _) => Answer::Exact(h),
            (Some(h), Some(l), _) => Answer::Coarser(Some(h), l),
            (None, Some(l), true) => Answer::Coarser(None, l),
            (None, Some(l), false) => Answer::Nothing(l),
        };
        (answer, walker.decor_ray.map(|(_, s)| s))
    }

    /// The column surface down column `g` at `cell_zoom` (pxl-engine
    /// `support::column_surface`). The first value is the top of the cell
    /// met, as drawn, with what the world lacked on the way, or None where
    /// the column meets nothing opaque. The second is the column surface
    /// itself, off whose line the cells around it are asked (`asset_about`).
    pub(super) fn column_surface(
        &self,
        cell_zoom: Zoom,
        g: [i32; 2],
    ) -> (Answer<Option<(i32, f32)>>, ColumnSurface<At>) {
        let mut walker = self.walker();
        let side = units(cell_zoom);
        let middle = g.map(|v| (f64::from(v) + 0.5) * side);
        change::read(middle, middle, cell_zoom);
        let f = support::column_surface(&mut walker, cell_zoom, g);
        let met = (f.end == End::Hit).then_some(f.top);
        let answer = match (met, walker.lack, walker.answered) {
            (m, None, _) => Answer::Exact(m),
            (Some(m), Some(l), _) => Answer::Coarser(Some(m), l),
            (None, Some(l), true) => Answer::Coarser(None, l),
            (None, Some(l), false) => Answer::Nothing(l),
        };
        (answer, f)
    }

    /// What is in the cell `c` at `zoom`, asked off the line `l`
    /// (pxl-engine `support::asset`). The descent starts from the deepest node
    /// on the line that holds the cell, never again from the top. Where the
    /// world lacks the cell, the answer is nothing with the missing tile.
    pub(super) fn asset_about(&self, l: &Line<At>, zoom: Zoom, c: [i32; 3]) -> Answer<u32> {
        let mut walker = self.walker();
        let val = support::asset(&mut walker, l, zoom, c);
        match walker.lack {
            None => Answer::Exact(val),
            Some(t) => Answer::Nothing(t),
        }
    }

    fn walker(&self) -> Walker<'_, 'a> {
        Walker {
            world: self,
            lack: None,
            answered: false,
            decor_ray: None,
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
