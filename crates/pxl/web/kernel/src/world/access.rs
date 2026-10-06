// pxl-kernel/world/access.rs

//! Queries over what is resident: a layer's value at a point (`at`), the
//! first thing a ray meets (`first_hit`), and a conservative bound for descent
//! (`relief`). A query never fetches or holds anything; it returns an exact
//! answer, a coarser one with the tile it lacked, or nothing and that tile.
//!
//! Answers come from the drawn surface (`ground_ele_at`), so picks stand on
//! what is on screen. Whether a ray may skip ground is decided from the index
//! lane's bounds instead, since a coarse drawn surface can sit below a peak.

use pxl_render::mesh::paint::terrain::elevation::{ground_ele_at, GroundLanes};
use pxl_spec::bake::constant::{CellZ, BAND, CELL, CELL_STEPS, OBJECT_FINEST, U_PER_CELL};
use pxl_spec::bake::deck::Occupies;
use pxl_spec::bake::fold::majority;
use pxl_spec::bake::format::{drawn, GroundCell, Terrain};
use pxl_spec::cell::cube_types::sentinel;
use pxl_spec::cell::object::{object_value_is_air, object_value_is_decor, object_value_is_voxel};
use pxl_spec::procedural::{pier_stands, Frame};
use pxl_spec::space::adapter::epsg3857::u_per_m;
use pxl_spec::space::hierarchy::{ancestor_of, child_of, first_descendant_of, offset_in};
use pxl_spec::space::lod::Tau;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pvf3::ivec::TO_XY;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pvf3::{projection, REACH};
use pxl_spec::space::pyramid::{Cell, Tile, EXTENT, GROUND, LEAF_Z, TOP_Z};
use pxl_spec::space::v3::{add3, Aabb, V3};

use crate::io::demand::Category;
use crate::kernel::kernel;
use crate::layer::object::store::{Entry, EntryAt, Store};
use crate::world::pyramid::{Layer as _, Pyramid};
use crate::world::residency::Residency;
use pxl_spec::space::zoom::idx;
use pxl_spec::space::zoom::{base_of, count, cube, index_of, units, Zoom};
use xpute_core::math::scalar::round;

/// Probe distance past a boundary, in units: far below the lattice's finest
/// step and far above f64 error at world coordinates.
const NUDGE: f64 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Detail {
    Zoom(Zoom),
    /// The zoom the LOD rule wants at each distance from the origin, so a
    /// ray's walk takes about as many steps per zoom whatever the zoom.
    Distance(Tau),
}

impl Detail {
    fn at(self, t: f64) -> (Zoom, f64) {
        match self {
            Detail::Zoom(z) => (z, f64::INFINITY),
            Detail::Distance(tau) => {
                let zoom = tau.z_at(t);
                let until = if zoom == TOP_Z { f64::INFINITY } else { tau.demand_reach(zoom) };
                (zoom, until)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lack {
    pub layer: Category,
    pub tile: Tile,
}

/// A coarser answer carries what it lacked, so none passes for exact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Answer<T> {
    Exact(T),
    Coarser(T, Lack),
    Nothing(Lack),
}

impl<T> Answer<T> {
    pub fn value(self) -> Option<T> {
        match self {
            Answer::Exact(v) | Answer::Coarser(v, _) => Some(v),
            Answer::Nothing(_) => None,
        }
    }

    pub fn exact(self) -> Option<T> {
        match self {
            Answer::Exact(v) => Some(v),
            _ => None,
        }
    }

    pub fn lack(&self) -> Option<Lack> {
        match self {
            Answer::Exact(_) => None,
            Answer::Coarser(_, l) | Answer::Nothing(l) => Some(*l),
        }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Answer<U> {
        match self {
            Answer::Exact(v) => Answer::Exact(f(v)),
            Answer::Coarser(v, l) => Answer::Coarser(f(v), l),
            Answer::Nothing(l) => Answer::Nothing(l),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Relief {
    /// Lowest and highest ground, in units; read from a coarser tile this is
    /// wider, never narrower.
    pub extent: (f64, f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    /// The surface's height as drawn, which a deck is measured against
    /// whether or not the cell has ground.
    pub height: f64,
    pub cell: GroundCell,
}

impl Ground {
    /// None where the terrain gives no surface, a void cell.
    pub fn top(&self) -> Option<f64> {
        (self.cell != sentinel::void).then_some(self.height)
    }
}

#[derive(Default)]
struct Lines {
    boxes: Vec<[f64; 2]>,
    air: Option<[f64; 2]>,
}

/// A leaf column as composed for both rays and footing, so ground, bores and
/// decks are combined in one place.
struct Composed {
    cell: [i64; 2],
    top: Option<f64>,
    lines: Lines,
    col_idx: IVec,
    local: [i64; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub origin: V3,
    pub direction: V3,
    pub reach: f64,
}

impl Ray {
    pub fn at(&self, t: f64) -> V3 {
        std::array::from_fn(|k| self.origin[k] + self.direction[k] * t)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Struck {
    Ground,
    Object(EntryAt),
    /// Objects folded into a cell above the leaves; no single object is met.
    Objects,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub t: f64,
    pub point: V3,
    pub normal: V3,
    /// The box struck.
    pub cube: Aabb,
    pub struck: Struck,
}

impl Hit {
    pub fn moved(self, by: V3) -> Hit {
        Hit {
            point: add3(&self.point, &by),
            cube: self.cube.moved(by),
            ..self
        }
    }
}

#[derive(Clone, Copy)]
pub struct Access<'a> {
    terrain: &'a Residency<Terrain>,
    objects: &'a Store,
    visits: Option<&'a std::cell::Cell<u64>>,
}

impl Access<'static> {
    pub fn now() -> Access<'static> {
        let pyramid: &'static Pyramid<Terrain> = Terrain::pyramid();
        let objects: &'static Store = &kernel().objects;
        Access::new(pyramid.residency(), objects)
    }
}

/// A square across which neither the tile nor the index cell changes.
struct Region {
    base: [f64; 2],
    side: f64,
    tile: Tile,
    /// Index bounds and whether read from the tile asked; none if nothing is resident.
    terrain: Option<((f64, f64), bool)>,
}

/// Folds sub-cell fill to the cell by majority, a zoom at a time, with the
/// objects' tie rule (`majority`).
fn filled_cell(filled: impl Fn([i64; 3]) -> bool) -> bool {
    const N: usize = CELL_STEPS as usize;
    let mut v = [false; N * N * N];
    for (k, f) in v.iter_mut().enumerate() {
        *f = filled([k % N, k / N % N, k / (N * N)].map(|c| c as i64));
    }
    let (mut m, mut cell_zoom) = (N, OBJECT_FINEST);
    while cell_zoom > CELL {
        cell_zoom -= 1;
        let h = m / 2;
        for k in 0..h * h * h {
            let parent = [k % h, k / h % h, k / (h * h)].map(|c| c as i64);
            let eight: [bool; 8] = core::array::from_fn(|slot| {
                let [x, y, z] = child_of(parent, slot).map(|c| c as usize);
                v[(z * m + y) * m + x]
            });
            v[k] = majority(eight, cell_zoom);
        }
        m = h;
    }
    v[0]
}

impl<'a> Access<'a> {
    pub fn new(terrain: &'a Residency<Terrain>, objects: &'a Store) -> Access<'a> {
        Access { terrain, objects, visits: None }
    }

    pub fn counting(self, visits: &'a std::cell::Cell<u64>) -> Access<'a> {
        Access { visits: Some(visits), ..self }
    }

    fn visit(&self) {
        if let Some(v) = self.visits {
            v.set(v.get() + 1);
        }
    }

    pub fn at(&self, at: &Pvf3, zoom: Zoom) -> Answer<Ground> {
        self.ground(GROUND.cell_of(zoom, at))
    }

    /// Down to the first solid's top, or up at most `rise` cells; taller gives none.
    pub fn support(&self, at: &Pvf3, rise: i64) -> Answer<Option<f64>> {
        let [ix, iy, iz] = index_of(CELL, at);
        self.stand_climbing([ix, iy], Some(iz), Some(rise))
    }

    /// `support` from the drawn surface, ignoring `at`'s height (a fix, a search
    /// result); none where nothing stands, a column with no surface and nothing placed.
    pub fn surface_support(&self, at: &Pvf3) -> Answer<Option<f64>> {
        let [ix, iy, _] = index_of(CELL, at);
        self.stand_climbing([ix, iy], None, None)
    }

    /// `support` rising any height; none where nothing stands under `at`.
    pub fn floor_under(&self, at: &Pvf3) -> Answer<Option<f64>> {
        let [ix, iy, iz] = index_of(CELL, at);
        self.stand_climbing([ix, iy], Some(iz), None)
    }

    /// First solid cell over `at`; none under open sky.
    pub fn ceiling_over(&self, at: &Pvf3) -> Answer<Option<i64>> {
        let [ix, iy, iz] = index_of(CELL, at);
        self.cells([ix, iy], |_, ceiling, solid| (iz + 1..ceiling).find(|&cz| solid(cz)))
    }

    /// First open cell at or over `at`.
    pub fn open_from(&self, at: &Pvf3) -> Answer<i64> {
        let [ix, iy, iz] = index_of(CELL, at);
        self.cells([ix, iy], |_, ceiling, solid| (iz..ceiling).find(|&cz| !solid(cz)).unwrap_or(iz.max(ceiling)))
    }

    fn stand_climbing(&self, [ix, iy]: [i64; 2], from: Option<i64>, rise: Option<i64>) -> Answer<Option<f64>> {
        self.cells([ix, iy], |top, ceiling, solid| {
            let side = units(CELL);
            let [lo, hi] = [REACH.min[2] / side, REACH.max[2] / side].map(|z| z as i64);
            // With no surface, the search starts at the highest solid, or at the reach's floor.
            let start = from.or(top).unwrap_or(ceiling).max(lo);
            let climb = rise.unwrap_or(hi - start);
            let stand = if start < ceiling && solid(start) {
                let over = (start + 1..=start + climb).find(|&cz| !solid(cz));
                if rise.is_none() {
                    over.or(Some(start))
                } else {
                    over
                }
            } else {
                (lo..start.min(ceiling)).rev().find(|&cz| solid(cz)).map(|cz| cz + 1).or(top.map(|_| start))
            };
            stand.map(|cz| cz as f64 * side)
        })
    }

    /// Ground top, ceiling and solidity of a column, in cells; sub-cells fold by majority.
    fn cells<R>(&self, [ix, iy]: [i64; 2], f: impl FnOnce(Option<i64>, i64, &dyn Fn(i64) -> bool) -> R) -> Answer<R> {
        self.composed([ix, iy]).map(|c| {
            let side = units(CELL);
            let top = c.top.map(|t| round(t / side) as i64);
            let made = |z: f64| {
                let inside = |[lo, hi]: [f64; 2]| z >= lo && z < hi;
                (top.is_some_and(|t| z < t as f64 * side) && !c.lines.air.is_some_and(inside)) || c.lines.boxes.iter().any(|&b| inside(b))
            };
            let n = CELL_STEPS as i64;
            let fine = units(OBJECT_FINEST);
            let solid = |cz: i64| {
                if !self.objects.may_hold([ix, iy, cz]) {
                    return made((cz as f64 + 0.5) * side);
                }
                filled_cell(|[x, y, z]| {
                    let v = u32::from(self.objects.folded_value(OBJECT_FINEST, [ix * n + x, iy * n + y, cz * n + z]));
                    match object_value_is_voxel(v) {
                        true => !object_value_is_air(v),
                        false => made(((cz * n + z) as f64 + 0.5) * fine),
                    }
                })
            };
            let lines_top = c.lines.boxes.iter().map(|b| (b[1] / side).ceil() as i64).max().unwrap_or(i64::MIN);
            let ceiling = top.unwrap_or(i64::MIN).max(lines_top).max(self.objects.ceiling([ix, iy]).unwrap_or(i64::MIN));
            f(top, ceiling, &solid)
        })
    }

    fn composed(&self, cell: [i64; 2]) -> Answer<Composed> {
        let [ix, iy] = cell;
        let base = base_of(CELL, [ix, iy, 0]);
        let ground = self.ground(GROUND.cell_of(LEAF_Z, &base));
        let [kx, ky, _] = index_of(BAND, &base);
        let col_idx = idx(BAND, TO_XY(base.i));
        let local = [ix - kx * cells_across(), iy - ky * cells_across()];
        ground.map(|g| Composed {
            cell,
            top: g.top(),
            lines: self.lines_over(GROUND.cell_of(LEAF_Z, &base), g.height),
            col_idx,
            local,
        })
    }

    /// Whether ground is there where it reaches: not bore air, and not taken by
    /// an object's voxel entry (air entries included).
    fn holds(&self, c: &Composed, cz: i64) -> bool {
        let z = cz as f64 * units(CELL);
        !c.lines.air.is_some_and(|[lo, hi]| z >= lo && z < hi) && !self.taken(c.col_idx, c.local, cz)
    }

    /// Height at the cell's center in the drawn-from tile, sampled as
    /// `ChunkSample::read` samples a chunk drawn from a coarser tile.
    fn ground(&self, cell: Cell) -> Answer<Ground> {
        let tile = cell.tile();
        let Some((src, body)) = self.drawn_from(tile) else { return Answer::Nothing(lack(tile)) };
        let [tx, ty] = sample_in(cell, src);
        let cell = body.ground.cells[GROUND.cell_index(tx as u32, ty as u32) as usize];
        let height = ground_ele_at(&body.ele, Some(GroundLanes::of(&body.ground)), tx, ty) as f64 * U_PER_CELL;
        self.answer(tile, Ground { height, cell })
    }

    /// The zoom the ground under `at` is known at; a step there is bounded by
    /// that zoom's cell.
    pub fn known_z(&self, at: &Pvf3) -> Option<Zoom> {
        let tile = GROUND.cell_of(LEAF_Z, at).tile();
        match self.terrain.source(tile) {
            Some(_) => Some(LEAF_Z),
            None => self.drawn_from(tile).map(|(src, _)| src.zoom),
        }
    }

    fn drawn_from(&self, tile: Tile) -> Option<(Tile, &'a Terrain)> {
        let src = self.terrain.drawn_from(tile)?;
        Some((src, self.terrain.get(src)?))
    }

    fn answer<T>(&self, tile: Tile, v: T) -> Answer<T> {
        match self.terrain.source(tile) {
            Some(_) => Answer::Exact(v),
            None => Answer::Coarser(v, lack(tile)),
        }
    }

    /// Bound on the ground over a node of `tile`'s zoom and every zoom finer than
    /// it, from one block of the index lane. For deciding descent, never for
    /// what is drawn.
    pub fn relief(&self, tile: Tile, (side, at): (Zoom, [i64; 2])) -> Answer<Relief> {
        let Some((src, body)) = self.drawn_from(tile) else {
            return Answer::Nothing(lack(tile));
        };
        let index = EXTENT.cell_zoom(src.zoom);
        let first = EXTENT.cell_of(src.zoom, &base_of(side, [at[0], at[1], 0]));
        let across = if side < index { count(index, side) } else { 1 };
        let b = body.index.over(first.in_tile(), across);
        self.answer(
            tile,
            Relief {
                extent: (f64::from(b.lo) * U_PER_CELL, f64::from(b.hi) * U_PER_CELL),
            },
        )
    }

    /// The first thing `ray` meets within its reach. Objects (resident whole)
    /// are found first; ground is walked only up to them, so an object is never
    /// exact over ground not known to lie behind it. What the ray starts inside
    /// is not met, so an eye under the ground is not stopped where it stands.
    ///
    /// Ground is walked region by region: skipped where the ray stays above the
    /// index bound, walked cell by cell otherwise. A coarse or missing region
    /// the ray may enter is the lack. Resident coarser tiles are tried first
    /// (`clear`) so far-off ground is skipped in one test per zoom.
    pub fn first_hit(&self, ray: &Ray, detail: Detail) -> Answer<Option<Hit>> {
        let mut lack: Option<Lack> = None;
        let mut answered = false;
        let mut t = 0.0;
        while t < ray.reach {
            let (zoom, until) = detail.at(t);
            let end = until.min(ray.reach);
            let objects = self.objects(ray, (t, end), zoom);
            let limit = objects.map_or(end, |o| o.t);
            let ground = self.ground_first(ray, (t, limit), zoom, &mut lack, &mut answered).filter(|g| g.t <= limit);
            if let Some(hit) = ground.or(objects) {
                return match lack {
                    Some(l) => Answer::Coarser(Some(hit), l),
                    None => Answer::Exact(Some(hit)),
                };
            }
            t = end;
        }
        match (lack, answered) {
            (None, _) => Answer::Exact(None),
            (Some(l), true) => Answer::Coarser(None, l),
            (Some(l), false) => Answer::Nothing(l),
        }
    }

    fn ground_first(&self, ray: &Ray, (t0, t1): (f64, f64), zoom: Zoom, lack: &mut Option<Lack>, answered: &mut bool) -> Option<Hit> {
        let mut t = t0;
        while t < t1 {
            self.visit();
            if let Some(out) = self.clear(ray, t, zoom, t1) {
                *answered = true;
                t = out;
                continue;
            }
            let r = self.region(&ray.at(t + NUDGE), zoom);
            let out = exit(ray, r.base, r.side).min(t1).max(t + NUDGE).min(ray.reach);
            let low = ray.at(t)[2].min(ray.at(out)[2]);
            let under = r.terrain.is_some_and(|((_, hi), _)| low <= hi);
            match r.terrain {
                Some((_, exact)) => {
                    *answered = true;
                    if under && !exact {
                        lack.get_or_insert(self::lack(r.tile));
                    }
                }
                None => {
                    lack.get_or_insert(self::lack(r.tile));
                }
            }
            if under {
                if let Some(hit) = self.walk(ray, &r, t, out, zoom) {
                    return Some(hit);
                }
            }
            t = out;
        }
        None
    }

    /// Far edge of the largest resident index cell coarser than `z` the ray
    /// passes over entirely, or none. Never past `until`, where another zoom applies.
    fn clear(&self, ray: &Ray, t: f64, zoom: Zoom, until: f64) -> Option<f64> {
        let from = ray.at(t + NUDGE);
        let at = on_ground(&from);
        (TOP_Z..zoom).find_map(|coarser| {
            let body = self.terrain.get(GROUND.cell_of(coarser, &at).tile())?;
            let e = EXTENT.cell_of(coarser, &at);
            let [_, hi] = drawn(body.index.extent()[e.index() as usize]);
            let out = exit(ray, corner(e.zoom(), &from), units(e.zoom())).min(until).max(t + NUDGE);
            let (za, zb) = (ray.at(t)[2], ray.at(out)[2]);
            (za.min(zb) > hi as f64 * U_PER_CELL).then_some(out)
        })
    }

    fn region(&self, p: &V3, zoom: Zoom) -> Region {
        let at = on_ground(p);
        let tile = GROUND.cell_of(zoom, &at).tile();
        let (cell_zoom, terrain) = match self.drawn_from(tile) {
            Some((src, body)) => {
                // The drawn-from tile's index cell, or the asked tile if smaller: one bound
                // and one tile across it.
                let e = EXTENT.cell_of(src.zoom, &at);
                let [lo, hi] = drawn(body.index.extent()[e.index() as usize]);
                let exact = self.terrain.source(tile).is_some();
                (e.zoom().max(zoom), Some(((lo as f64 * U_PER_CELL, hi as f64 * U_PER_CELL), exact)))
            }
            None => (zoom, None),
        };
        Region {
            base: corner(cell_zoom, p),
            side: units(cell_zoom),
            tile,
            terrain,
        }
    }

    /// Walks cell columns at `z` inside `r` (Amanatides–Woo). Stops on the
    /// region's cells, not on `t1`: the two distances round differently, and a
    /// column past the edge would be read as this region's.
    fn walk(&self, ray: &Ray, r: &Region, t0: f64, t1: f64, zoom: Zoom) -> Option<Hit> {
        let side = units(GROUND.cell_zoom(zoom));
        let across = (r.side / side) as i64;
        let first = r.base.map(|v| (v / side).floor() as i64);
        let inside = |c: [i64; 2]| (0..2).all(|k| (first[k]..first[k] + across).contains(&c[k]));
        let [dx, dy, _] = ray.direction;
        let start = ray.at(t0 + NUDGE);
        let mut cell = [(start[0] / side).floor() as i64, (start[1] / side).floor() as i64];
        let axis = |d: f64, c: i64, o: f64| -> (f64, f64) {
            if d.abs() < 1e-12 {
                (f64::INFINITY, f64::INFINITY)
            } else {
                (side / d.abs(), ((c + i64::from(d > 0.0)) as f64 * side - o) / d)
            }
        };
        let (delta_x, mut next_x) = axis(dx, cell[0], ray.origin[0]);
        let (delta_y, mut next_y) = axis(dy, cell[1], ray.origin[1]);
        let mut from = t0;
        while from < t1 && inside(cell) {
            self.visit();
            let to = next_x.min(next_y).min(t1);
            let hit = if zoom == LEAF_Z {
                self.column(ray, cell, from, to)
            } else {
                self.folded(ray, cell, from, to, zoom)
            };
            if hit.is_some() {
                return hit;
            }
            if next_x < next_y {
                cell[0] += if dx > 0.0 { 1 } else { -1 };
                next_x += delta_x;
            } else {
                cell[1] += if dy > 0.0 { 1 } else { -1 };
                next_y += delta_y;
            }
            from = to;
        }
        None
    }

    /// Decks and bore air over a cell, as the chunk paints them (`Over::occupies`).
    fn lines_over(&self, cell: Cell, ground_height: f64) -> Lines {
        let mut lines = Lines::default();
        let Some((src, body)) = self.drawn_from(cell.tile()) else { return lines };
        let (at, side) = (sample_in(cell, src), 1.0 / count(GROUND.cell_zoom(cell.tile_zoom()), GROUND.cell_zoom(src.zoom)) as f64);
        let ground = round(ground_height / U_PER_CELL) as i32;
        let units = |[lo, hi]: [i32; 2]| [f64::from(lo) * U_PER_CELL, f64::from(hi) * U_PER_CELL];
        let tile = Frame::of(src);
        let frame = Some(tile);
        let cell_per_m = u_per_m(tile.corner[1]) / U_PER_CELL;
        let [tx, ty] = at.map(|v| v as u32);
        let look = body.ground.cells[GROUND.cell_index(tx, ty) as usize];
        for o in body.decks.over_point(frame, at, side) {
            match o.occupies(ground, cell_per_m) {
                Occupies::Grade => {}
                Occupies::Above { slab, pier } => {
                    lines.boxes.push(units(slab));
                    if let Some(pier) = pier.and_then(|p| pier_stands(p, look, body.decks.over_point(frame, at, side), ground, cell_per_m)) {
                        lines.boxes.push(units(pier));
                    }
                }
                Occupies::Through { air } => lines.air = Some(units(air)),
            }
        }
        lines
    }

    fn with_decks(&self, ray: &Ray, boxes: &[[f64; 2]], (corner, side): ([f64; 2], f64), (t0, t1): (f64, f64), ground: Option<Hit>) -> Option<Hit> {
        let mut best = ground;
        for &[lo, hi] in boxes {
            let cube = Aabb {
                min: [corner[0], corner[1], lo],
                max: [corner[0] + side, corner[1] + side, hi],
            };
            let Some((t, normal)) = ray_box(ray, cube) else { continue };
            if t < t0 || t > t1 || best.is_some_and(|b| b.t <= t) {
                continue;
            }
            best = Some(hit(ray, t, normal, cube, Struck::Ground));
        }
        best
    }

    fn column(&self, ray: &Ray, [ix, iy]: [i64; 2], t0: f64, t1: f64) -> Option<Hit> {
        let side = units(CELL);
        let c = self.composed([ix, iy]).value()?;
        let ground = self.ground_column(ray, &c, t0, t1);
        self.with_decks(ray, &c.lines.boxes, ([ix as f64 * side, iy as f64 * side], side), (t0, t1), ground)
    }

    /// Ground hit in a leaf column: a cell under the top not taken by a voxel
    /// entry and not in bore air. Steps by cell only once under the top.
    fn ground_column(&self, ray: &Ray, c: &Composed, t0: f64, t1: f64) -> Option<Hit> {
        let ([ix, iy], top) = (c.cell, c.top?);
        let side = units(CELL);
        let (za, zb) = (ray.at(t0)[2], ray.at(t1)[2]);
        let dz = ray.direction[2];
        let at_z = |z: f64| (z - ray.origin[2]) / dz;
        let mut from = t0;
        if za >= top {
            if zb >= top {
                return None;
            }
            from = at_z(top);
        }
        let mut cz = (ray.at(from)[2] / side).floor() as i64;
        if from <= 0.0 && self.holds(c, cz) {
            return None;
        }
        loop {
            let leave = if dz.abs() < 1e-12 {
                t1
            } else {
                let face = if dz > 0.0 { (cz + 1) as f64 * side } else { cz as f64 * side };
                at_z(face).min(t1)
            };
            if (cz as f64) * side < top && from > 0.0 && self.holds(c, cz) {
                // Through the column's side on the entry cell, else through floor or ceiling.
                let normal = if from == t0 {
                    wall_normal(ray, from, side)
                } else {
                    [0.0, 0.0, if dz > 0.0 { -1.0 } else { 1.0 }]
                };
                let min = [ix as f64 * side, iy as f64 * side, cz as f64 * side];
                return Some(hit(ray, from, normal, Aabb { min, max: min.map(|v| v + side) }, Struck::Ground));
            }
            if leave >= t1 {
                return None;
            }
            cz += if dz > 0.0 { 1 } else { -1 };
            from = leave;
        }
    }

    fn folded(&self, ray: &Ray, [ix, iy]: [i64; 2], t0: f64, t1: f64, zoom: Zoom) -> Option<Hit> {
        let cell_zoom = GROUND.cell_zoom(zoom);
        let side = units(cell_zoom);
        let cell = GROUND.cell_of(zoom, &base_of(cell_zoom, [ix, iy, 0]));
        let g = self.ground(cell).value()?;
        let ground = g.top().and_then(|top| self.folded_ground(ray, [ix, iy], top, t0, t1, zoom));
        let lines = self.lines_over(cell, g.height);
        self.with_decks(ray, &lines.boxes, ([ix as f64 * side, iy as f64 * side], side), (t0, t1), ground)
    }

    fn folded_ground(&self, ray: &Ray, [ix, iy]: [i64; 2], top: f64, t0: f64, t1: f64, zoom: Zoom) -> Option<Hit> {
        let cell_zoom = GROUND.cell_zoom(zoom);
        let side = units(cell_zoom);
        let (za, zb) = (ray.at(t0)[2], ray.at(t1)[2]);
        let (t, normal) = if za <= top {
            (t0 > 0.0).then(|| (t0, wall_normal(ray, t0, side)))?
        } else if zb <= top {
            ((top - ray.origin[2]) / ray.direction[2], [0.0, 0.0, 1.0])
        } else {
            return None;
        };
        let cz = ((ray.at(t)[2] - normal[2] * NUDGE) / side).floor();
        let min = [ix as f64 * side, iy as f64 * side, cz * side];
        Some(hit(ray, t, normal, Aabb { min, max: min.map(|v| v + side) }, Struck::Ground))
    }

    /// First object the ray meets, nearest first (`Store::along_ray`). On the leaves
    /// voxel entries are hit at their own boxes; above the leaves the folded cell is
    /// hit whole, matching what that zoom draws.
    fn objects(&self, ray: &Ray, (t0, t1): (f64, f64), zoom: Zoom) -> Option<Hit> {
        let cell_zoom = GROUND.cell_zoom(zoom);
        let side = units(cell_zoom);
        let mut met = None;
        self.objects.along_ray(ray.origin, ray.direction, (t0, t1), cell_zoom, &mut |[x, y, h]| {
            met = if zoom == LEAF_Z {
                let steps = u32::from(CELL - BAND);
                let [cx, cy] = ancestor_of([x, y], steps);
                let col_idx = crate::layer::object::store::column_at([cx, cy]);
                let entries = self.cell_entries(col_idx, offset_in([x, y], steps), h);
                let corner = first_descendant_of([cx, cy], steps).map(|v| v as f64 * side);
                object_hit(ray, entries, corner)
            } else {
                let v = u32::from(self.objects.folded_value(cell_zoom, [x, y, h]));
                (object_value_is_voxel(v) && !object_value_is_air(v)).then(|| cell_hit(ray, [x, y, h], cell_zoom, t0)).flatten()
            };
            met.is_some()
        });
        met
    }

    fn taken(&self, col_idx: IVec, [cx, cy]: [i64; 2], cz: i64) -> bool {
        let Some(run) = self.objects.cell(col_idx, cx as u32, cy as u32, cz as CellZ) else { return false };
        let Some(e) = self.objects.entries(col_idx) else { return false };
        (run.start as usize..(run.start + run.count) as usize).any(|k| !object_value_is_decor(e.state[k]))
    }

    /// Decor paints no voxel, so a cell of decor alone is the ground's.
    fn cell_entries(&self, col_idx: IVec, [cx, cy]: [i64; 2], cz: i64) -> impl Iterator<Item = Entry> + '_ {
        let run = self.objects.cell(col_idx, cx as u32, cy as u32, cz as CellZ);
        let e = run.and(self.objects.entries(col_idx));
        let ks = match (run, e) {
            (Some(run), Some(_)) => run.start as usize..(run.start + run.count) as usize,
            _ => 0..0,
        };
        ks.filter_map(move |k| {
            let e = e?;
            (!object_value_is_decor(e.state[k])).then(|| {
                let mut cursor = Entry::default();
                cursor.at_column(col_idx);
                cursor.load(e.band[k], e.pos[k], e.state[k]);
                cursor
            })
        })
    }
}

fn cells_across() -> i64 {
    count(CELL, BAND) as i64
}

fn exit(ray: &Ray, base: [f64; 2], side: f64) -> f64 {
    (0..2)
        .map(|k| {
            let d = ray.direction[k];
            if d > 1e-12 {
                (base[k] + side - ray.origin[k]) / d
            } else if d < -1e-12 {
                (base[k] - ray.origin[k]) / d
            } else {
                f64::INFINITY
            }
        })
        .fold(f64::INFINITY, f64::min)
}

fn wall_normal(ray: &Ray, t: f64, side: f64) -> V3 {
    let p = ray.at(t);
    let off = |v: f64| {
        let r = v.rem_euclid(side);
        r.min(side - r)
    };
    let k = if off(p[0]) <= off(p[1]) { 0 } else { 1 };
    let mut n = [0.0; 3];
    n[k] = if ray.direction[k] > 0.0 { -1.0 } else { 1.0 };
    n
}

fn object_hit(ray: &Ray, entries: impl Iterator<Item = Entry>, [x0, y0]: [f64; 2]) -> Option<Hit> {
    let mut best: Option<Hit> = None;
    for e in entries.filter(|e| !object_value_is_air(e.state)) {
        let [lx, ly, lz] = e.local();
        let min = [x0 + lx, y0 + ly, lz];
        let cube = Aabb { min, max: min.map(|v| v + e.size()) };
        let Some((t, normal)) = ray_box(ray, cube) else { continue };
        if best.is_some_and(|b| b.t <= t) {
            continue;
        }
        best = Some(hit(ray, t, normal, cube, Struck::Object(e.at())));
    }
    best
}

fn ray_box(ray: &Ray, cube: Aabb) -> Option<(f64, V3)> {
    let (enter, _, k) = cube.span_along(ray.origin, ray.direction)?;
    let mut normal = [0.0; 3];
    normal[k] = if ray.direction[k] > 0.0 { -1.0 } else { 1.0 };
    (enter >= 0.0 && enter <= ray.reach).then_some((enter, normal))
}

/// Where the ray meets the whole cell; if already inside at `from`, there,
/// through the nearest side. None if it starts inside.
fn cell_hit(ray: &Ray, cell: [i64; 3], cell_zoom: Zoom, from: f64) -> Option<Hit> {
    let cube = cube(cell_zoom, cell);
    let (t, normal) = match ray_box(ray, cube) {
        Some((t, normal)) if t >= from => (t, normal),
        _ => (cube.holds(ray.at(from)) && from > 0.0).then(|| (from, wall_normal(ray, from, units(cell_zoom))))?,
    };
    Some(hit(ray, t, normal, cube, Struck::Objects))
}

fn hit(ray: &Ray, t: f64, normal: V3, cube: Aabb, struck: Struck) -> Hit {
    Hit {
        t,
        point: ray.at(t),
        normal,
        cube,
        struck,
    }
}

fn lack(tile: Tile) -> Lack {
    Lack { layer: Category::terrain, tile }
}

/// A point's place on the ground lattice, its height dropped.
fn on_ground(p: &V3) -> Pvf3 {
    projection::project(p[0], p[1], 0.0)
}

/// The corner of `cell_zoom`'s cell under `p`, in the frame `p` is in: a ray
/// across the antimeridian walks on, where the cell's stored X wraps.
fn corner(cell_zoom: Zoom, p: &V3) -> [f64; 2] {
    let side = units(cell_zoom);
    [(p[0] / side).floor() * side, (p[1] / side).floor() * side]
}

/// Where `cell`'s center falls in the tile `src` it is drawn from, in that tile's cells.
fn sample_in(cell: Cell, src: Tile) -> [f64; 2] {
    let n = count(GROUND.cell_zoom(cell.tile_zoom()), GROUND.cell_zoom(src.zoom)) as f64;
    cell.in_tile_above(cell.tile_zoom() - src.zoom).map(|o| (o as f64 + 0.5) / n)
}

#[cfg(test)]
#[path = "access.test.rs"]
mod test;
