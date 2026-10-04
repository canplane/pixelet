// pxl-kernel/world/access.rs

//! Queries over what is resident: a layer's value at a point (`at`), the
//! first thing a ray meets (`first_hit`), and a conservative bound for descent
//! (`relief`). A query never fetches or holds anything; it returns an exact
//! answer, a coarser one with the page it lacked, or nothing and that page.
//!
//! Answers come from the drawn surface (`ground_ele_at`), so picks stand on
//! what is on screen. Whether a ray may skip ground is decided from the index
//! lane's bounds instead, since a coarse drawn surface can sit below a peak.

use pxl_render::mesh::paint::terrain::elevation::{ground_ele_at, GroundLanes};
use pxl_spec::bake::constant::{CellZ, CELL, U_PER_CELL};
use pxl_spec::bake::format::Terrain;
use pxl_spec::bake::pack::GroundCell;
use pxl_spec::cell::object::{object_value_is_air, object_value_is_decor, object_value_is_voxel};
use pxl_spec::space::hierarchy::Scale;
use pxl_spec::space::indexer::{L2, L2_IDX, L3};
use pxl_spec::space::lod::Tau;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pvf3::projection;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::pyramid::{Cell, LevelPage, EXTENT, GROUND, TOP_LEVEL};
use pxl_spec::space::v3::{Aabb, V3};
use pxl_spec::space::xy::TO_XY;

use crate::io::demand::Category;
use crate::kernel::kernel;
use crate::layer::object::store::{EntryAt, EntryCursor, Store};
use crate::world::paging::Paging;
use crate::world::pyramid::{Layer as _, Pyramid};

/// Terrain's finest level, z16, where every point observer stands.
pub const FINEST: u32 = 0;

/// Probe distance past a boundary, in units: far below the lattice's finest
/// step and far above f64 error at world coordinates.
const NUDGE: f64 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Detail {
    Level(u32),
    /// The level the LOD rule wants at each distance from the origin, so a
    /// ray's walk takes about as many steps per level whatever the level.
    Distance(Tau),
}

impl Detail {
    fn at(self, t: f64) -> (u32, f64) {
        match self {
            Detail::Level(level) => (level, f64::INFINITY),
            Detail::Distance(tau) => {
                let level = tau.level(t).clamp(0, TOP_LEVEL as i32) as u32;
                let until = if level == TOP_LEVEL { f64::INFINITY } else { tau.split_distance(level as i32 + 1) };
                (level, until)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lack {
    pub layer: Category,
    pub page: LevelPage,
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
    /// Lowest and highest ground, in units; read from a coarser page this is
    /// wider, never narrower.
    pub extent: (f64, f64),
    /// Steepest rise over run: the largest span of an index cell over its side,
    /// which bounds the interpolated surface between samples.
    pub slope: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    pub top: f64,
    pub cell: GroundCell,
}

#[derive(Default)]
struct Lines {
    boxes: Vec<[f64; 2]>,
    air: Option<[f64; 2]>,
}

/// A z16 column as composed for both rays and footing, so ground, bores and
/// decks are combined in one place.
struct Composed {
    cell: [i64; 2],
    top: f64,
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
    fn at(&self, t: f64) -> V3 {
        std::array::from_fn(|k| self.origin[k] + self.direction[k] * t)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Struck {
    Ground,
    Object(EntryAt),
    /// Objects folded into a cell above z16; no single object is met.
    Objects,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub t: f64,
    pub point: V3,
    pub normal: V3,
    pub box_min: V3,
    pub box_max: V3,
    pub struck: Struck,
}

impl Hit {
    pub fn moved(self, by: V3) -> Hit {
        let add = |v: V3| std::array::from_fn(|k| v[k] + by[k]);
        Hit {
            point: add(self.point),
            box_min: add(self.box_min),
            box_max: add(self.box_max),
            ..self
        }
    }
}

#[derive(Clone, Copy)]
pub struct Access<'a> {
    terrain: &'a Paging<Terrain>,
    objects: &'a Store,
    visits: Option<&'a std::cell::Cell<u64>>,
}

impl Access<'static> {
    pub fn now() -> Access<'static> {
        let pyramid: &'static Pyramid<Terrain> = Terrain::pyramid();
        let objects: &'static Store = &kernel().objects;
        Access::new(pyramid.paging(), objects)
    }
}

/// A square across which neither the page nor the index cell changes.
struct Region {
    base: [f64; 2],
    side: f64,
    page: LevelPage,
    /// Index bounds and whether read from the page asked; none if nothing is resident.
    terrain: Option<((f64, f64), bool)>,
}

pub fn support_revision() -> [u32; 2] {
    [Terrain::pyramid().revision(), objects_revision()]
}

/// Folds sub-cell fill to the cell by majority, a level at a time, with the
/// objects' tie rule (`majority`).
fn filled_cell(filled: impl Fn([i64; 3]) -> bool) -> bool {
    const N: usize = CELL_STEPS as usize;
    let mut v = [false; N * N * N];
    for (k, f) in v.iter_mut().enumerate() {
        *f = filled([k % N, k / N % N, k / (N * N)].map(|c| c as i64));
    }
    let (mut m, mut scale) = (N, OBJECT_FINEST);
    while scale < CELL {
        scale = scale.up(1);
        let h = m / 2;
        for k in 0..h * h * h {
            let [x, y, z] = [k % h, k / h % h, k / (h * h)].map(|c| c * 2);
            let eight: [bool; 8] = core::array::from_fn(|c| v[((z + (c >> 2)) * m + y + (c >> 1 & 1)) * m + x + (c & 1)]);
            v[k] = majority(eight, scale.above(CELL));
        }
        m = h;
    }
    v[0]
}

impl<'a> Access<'a> {
    pub fn new(terrain: &'a Paging<Terrain>, objects: &'a Store) -> Access<'a> {
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

    pub fn at(&self, at: &Pvf3, level: u32) -> Answer<Ground> {
        self.ground(GROUND.cell_of(level, at))
    }

    /// Down to the first solid's top, or up at most `rise` cells; taller gives none.
    pub fn support(&self, at: &Pvf3, rise: i64) -> Answer<Option<f64>> {
        let [ix, iy, iz] = CELL.index_of(at);
        self.stand([ix, iy], Some(iz))
    }

    /// `support` from the drawn surface, ignoring `at`'s height (a fix, a search result).
    pub fn surface_support(&self, at: &Pvf3) -> Answer<f64> {
        let [ix, iy, _] = CELL.index_of(at);
        // From the surface there is no wall, so it is never none.
        self.stand([ix, iy], None).map(Option::unwrap_or_default)
    }

    /// `support` rising any height.
    pub fn floor_under(&self, at: &Pvf3) -> Answer<f64> {
        let [ix, iy, iz] = CELL.index_of(at);
        // Climbing the whole reach meets no wall, so it is never none.
        self.stand_climbing([ix, iy], Some(iz), true).map(Option::unwrap_or_default)
    }

    /// First solid cell over `at`; none under open sky.
    pub fn ceiling_over(&self, at: &Pvf3) -> Answer<Option<i64>> {
        let [ix, iy, iz] = CELL.index_of(at);
        self.cells([ix, iy], |_, ceiling, solid| (iz + 1..ceiling).find(|&cz| solid(cz)))
    }

    /// First open cell at or over `at`.
    pub fn open_from(&self, at: &Pvf3) -> Answer<i64> {
        let [ix, iy, iz] = CELL.index_of(at);
        self.cells([ix, iy], |_, ceiling, solid| (iz..ceiling).find(|&cz| !solid(cz)).unwrap_or(iz.max(ceiling)))
    }

    fn stand_climbing(&self, [ix, iy]: [i64; 2], from: Option<i64>, rise: Option<i64>) -> Answer<Option<f64>> {
        self.cells([ix, iy], |top, ceiling, solid| {
            let side = CELL.side();
            let [lo, hi] = [REACH.min[2] / side, REACH.max[2] / side].map(|z| z as i64);
            let start = from.unwrap_or(top);
            let climb = rise.unwrap_or(hi - start);
            let stand = if start < ceiling && solid(start) {
                let over = (start + 1..=start + climb).find(|&cz| !solid(cz));
                if rise.is_none() {
                    over.or(Some(start))
                } else {
                    over
                }
            } else {
                Some((lo..start.min(ceiling)).rev().find(|&cz| solid(cz)).map_or(start, |cz| cz + 1))
            };
            stand.map(|cz| cz as f64 * side)
        })
    }

    /// Ground top, ceiling and solidity of a column, in cells; sub-cells fold by majority.
    fn cells<R>(&self, [ix, iy]: [i64; 2], f: impl FnOnce(i64, i64, &dyn Fn(i64) -> bool) -> R) -> Answer<R> {
        self.composed([ix, iy]).map(|c| {
            let side = CELL.side();
            let top = (c.top / side).round() as i64;
            let made = |z: f64| {
                let inside = |[lo, hi]: [f64; 2]| z >= lo && z < hi;
                (z < top as f64 * side && !c.lines.air.is_some_and(inside)) || c.lines.boxes.iter().any(|&b| inside(b))
            };
            let n = CELL_STEPS as i64;
            let fine = OBJECT_FINEST.side();
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
            let ceiling = top.max(lines_top).max(self.objects.ceiling([ix, iy]).unwrap_or(i64::MIN));
            let stand = if start < ceiling && solid(start) {
                let over = (start + 1..=start + climb).find(|&cz| !solid(cz));
                if any_height {
                    over.or(Some(start))
                } else {
                    over
                }
            } else {
                Some((lo..start.min(ceiling)).rev().find(|&cz| solid(cz)).map_or(start, |cz| cz + 1))
            };
            stand.map(|cz| cz as f64 * side)
        })
    }

    fn composed(&self, cell: [i64; 2]) -> Answer<Composed> {
        let [ix, iy] = cell;
        let base = CELL.base_of([ix, iy, 0]);
        let ground = self.ground(GROUND.cell_of(0, &base));
        let [kx, ky, _] = L2.index_of(&base);
        let col_idx = L2_IDX(TO_XY(base.i));
        let local = [ix - kx * cells_across(), iy - ky * cells_across()];
        ground.map(|g| Composed {
            cell,
            top: g.top,
            lines: self.lines_over(GROUND.cell_of(0, &base), g.top),
            col_idx,
            local,
        })
    }

    /// Whether ground is there where it reaches: not bore air, and not taken by
    /// an object's voxel entry (air entries included).
    fn holds(&self, c: &Composed, cz: i64) -> bool {
        let z = cz as f64 * CELL.side();
        !c.lines.air.is_some_and(|[lo, hi]| z >= lo && z < hi) && !self.taken(c.col_idx, c.local, cz)
    }

    /// Height at the cell's center in the drawn-from page, sampled as
    /// `ChunkSample::read` samples a chunk drawn from a coarser page.
    fn ground(&self, cell: Cell) -> Answer<Ground> {
        let page = cell.page();
        let lack = Lack { layer: Category::terrain, page };
        let Some((src, body)) = self.drawn_from(page) else { return Answer::Nothing(lack) };
        let n = GROUND.scale(page.level).count_in(GROUND.scale(src.level)) as f64;
        let [ox, oy] = cell.in_page_above(src.level - page.level);
        let (tx, ty) = ((ox as f64 + 0.5) / n, (oy as f64 + 0.5) / n);
        let top = ground_ele_at(&body.ele, Some(GroundLanes::of(&body.ground)), tx, ty) as f64 * U_PER_CELL;
        let cell = body.ground.cells[GROUND.cell_index(tx as u32, ty as u32) as usize];
        self.answer(page, Ground { top, cell })
    }

    /// The level the ground under `at` is known at; a step there is bounded by
    /// that level's cell.
    pub fn known_level(&self, at: &Pvf3) -> Option<u32> {
        let page = GROUND.cell_of(FINEST, at).page();
        match self.terrain.source(page) {
            Some(_) => Some(FINEST),
            None => self.drawn_from(page).map(|(src, _)| src.level),
        }
    }

    fn drawn_from(&self, page: LevelPage) -> Option<(LevelPage, &'a Terrain)> {
        let src = self.terrain.drawn_from(page)?;
        Some((src, self.terrain.get(src)?))
    }

    fn answer<T>(&self, page: LevelPage, v: T) -> Answer<T> {
        match self.terrain.source(page) {
            Some(_) => Answer::Exact(v),
            None => Answer::Coarser(v, Lack { layer: Category::terrain, page }),
        }
    }

    /// Bound on the ground over a node of `page`'s level and every level under
    /// it, from one block of the index lane. For deciding descent, never for
    /// what is drawn.
    pub fn relief(&self, page: LevelPage, (side, at): (Scale, [i64; 2])) -> Answer<Relief> {
        let Some((src, body)) = self.drawn_from(page) else {
            return Answer::Nothing(Lack { layer: Category::terrain, page });
        };
        let index = EXTENT.scale(src.level);
        let first = EXTENT.cell_of(src.level, &side.base_of([at[0], at[1], 0]));
        let across = if side > index { index.count_in(side) } else { 1 };
        let b = body.index.over(first.in_page(), across);
        self.answer(
            page,
            Relief {
                extent: (f64::from(b.lo) * U_PER_CELL, f64::from(b.hi) * U_PER_CELL),
                slope: f64::from(b.span) * U_PER_CELL / index.side(),
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
    /// the ray may enter is the lack. Resident coarser pages are tried first
    /// (`clear`) so far-off ground is skipped in one test per level.
    pub fn first_hit(&self, ray: &Ray, detail: Detail) -> Answer<Option<Hit>> {
        let mut lack: Option<Lack> = None;
        let mut answered = false;
        let mut t = 0.0;
        while t < ray.reach {
            let (level, until) = detail.at(t);
            let end = until.min(ray.reach);
            let objects = self.objects(ray, (t, end), level);
            let limit = objects.map_or(end, |o| o.t);
            let ground = self.ground_first(ray, (t, limit), level, &mut lack, &mut answered).filter(|g| g.t <= limit);
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

    fn ground_first(&self, ray: &Ray, (t0, t1): (f64, f64), level: u32, lack: &mut Option<Lack>, answered: &mut bool) -> Option<Hit> {
        let mut t = t0;
        while t < t1 {
            if let Some(out) = self.clear(ray, t, level, t1) {
                *answered = true;
                t = out;
                continue;
            }
            let r = self.region(&ray.at(t + NUDGE), level);
            let out = exit(ray, r.base, r.side).min(t1).max(t + NUDGE).min(ray.reach);
            let low = ray.at(t)[2].min(ray.at(out)[2]);
            let under = r.terrain.is_some_and(|((_, hi), _)| low <= hi);
            match r.terrain {
                Some((_, exact)) => {
                    *answered = true;
                    if under && !exact {
                        lack.get_or_insert(Lack {
                            layer: Category::terrain,
                            page: r.page,
                        });
                    }
                }
                None => {
                    lack.get_or_insert(Lack {
                        layer: Category::terrain,
                        page: r.page,
                    });
                }
            }
            if under {
                if let Some(hit) = self.walk(ray, &r, t, out, level) {
                    return Some(hit);
                }
            }
            t = out;
        }
        None
    }

    /// Far edge of the largest resident index cell above `level` the ray passes
    /// over entirely, or none. Never past `until`, where another level applies.
    fn clear(&self, ray: &Ray, t: f64, level: u32, until: f64) -> Option<f64> {
        let p = ray.at(t + NUDGE);
        let mut at = Pvf3::default();
        projection::project(&mut at, p[0], p[1], 0.0);
        (level + 1..=TOP_LEVEL).rev().find_map(|lv| {
            let body = self.terrain.get(GROUND.cell_of(lv, &at).page())?;
            let e = EXTENT.cell_of(lv, &at);
            let [_, hi] = body.index.extent()[e.index() as usize];
            let scale = e.scale();
            let [ix, iy, _] = scale.index_of(&at);
            let base = scale.base_of([ix, iy, 0]);
            let base = [pvf3::to_real(pvf3::X(&base)), pvf3::to_real(pvf3::Y(&base))];
            let out = exit(ray, base, scale.side()).min(until).max(t + NUDGE);
            let (za, zb) = (ray.at(t)[2], ray.at(out)[2]);
            (za.min(zb) > hi as f64 * U_PER_CELL).then_some(out)
        })
    }

    fn region(&self, p: &V3, level: u32) -> Region {
        let mut at = Pvf3::default();
        projection::project(&mut at, p[0], p[1], 0.0);
        let page = GROUND.cell_of(level, &at).page();
        let page_scale = L3.up(level as i32);
        let (scale, terrain) = match self.drawn_from(page) {
            Some((src, body)) => {
                // The drawn-from page's index cell, or the asked page if smaller: one bound
                // and one page across it.
                let e = EXTENT.cell_of(src.level, &at);
                let [lo, hi] = body.index.extent()[e.index() as usize];
                let exact = self.terrain.source(page).is_some();
                (e.scale().min(page_scale), Some(((lo as f64 * U_PER_CELL, hi as f64 * U_PER_CELL), exact)))
            }
            None => (page_scale, None),
        };
        let [ix, iy, _] = scale.index_of(&at);
        let base = scale.base_of([ix, iy, 0]);
        let base = [pvf3::to_real(pvf3::X(&base)), pvf3::to_real(pvf3::Y(&base))];
        Region {
            base,
            side: scale.side(),
            page,
            terrain,
        }
    }

    /// Walks cell columns of `level` inside `r` (Amanatides–Woo). Stops on the
    /// region's cells, not on `t1`: the two distances round differently, and a
    /// column past the edge would be read as this region's.
    fn walk(&self, ray: &Ray, r: &Region, t0: f64, t1: f64, level: u32) -> Option<Hit> {
        let side = GROUND.scale(level).side();
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
            let to = next_x.min(next_y).min(t1);
            let hit = if level == 0 { self.column(ray, cell, from, to) } else { self.folded(ray, cell, from, to, level) };
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
    fn lines_over(&self, cell: Cell, ground_top: f64) -> Lines {
        let mut lines = Lines::default();
        let Some((src, body)) = self.drawn_from(cell.page()) else { return lines };
        let n = GROUND.scale(cell.level()).count_in(GROUND.scale(src.level)) as f64;
        let [ox, oy] = cell.in_page_above(src.level - cell.level());
        let ground = (ground_top / U_PER_CELL).round() as i32;
        let units = |[lo, hi]: [i32; 2]| [f64::from(lo) * U_PER_CELL, f64::from(hi) * U_PER_CELL];
        for o in body.decks.over_point([(ox as f64 + 0.5) / n, (oy as f64 + 0.5) / n]) {
            match o.occupies(ground) {
                Occupies::Grade => {}
                Occupies::Above { slab, pier } => lines.boxes.extend([Some(slab), pier].into_iter().flatten().map(units)),
                Occupies::Through { air } => lines.air = Some(units(air)),
            }
        }
        lines
    }

    fn with_decks(&self, ray: &Ray, boxes: &[[f64; 2]], (corner, side): ([f64; 2], f64), (t0, t1): (f64, f64), ground: Option<Hit>) -> Option<Hit> {
        let mut best = ground;
        for &[lo, hi] in boxes {
            let (min, max) = ([corner[0], corner[1], lo], [corner[0] + side, corner[1] + side, hi]);
            let Some((t, normal)) = ray_box(ray, &min, &max) else { continue };
            if t < t0 || t > t1 || best.is_some_and(|b| b.t <= t) {
                continue;
            }
            best = Some(Hit {
                t,
                point: ray.at(t),
                normal,
                box_min: min,
                box_max: max,
                struck: Struck::Ground,
            });
        }
        best
    }

    fn column(&self, ray: &Ray, [ix, iy]: [i64; 2], t0: f64, t1: f64) -> Option<Hit> {
        let side = CELL.side();
        let c = self.composed([ix, iy]).value()?;
        let ground = self.ground_column(ray, &c, t0, t1);
        self.with_decks(ray, &c.lines.boxes, ([ix as f64 * side, iy as f64 * side], side), (t0, t1), ground)
    }

    /// Ground hit in a z16 column: a cell under the top not taken by a voxel
    /// entry and not in bore air. Steps by cell only once under the top.
    fn ground_column(&self, ray: &Ray, c: &Composed, t0: f64, t1: f64) -> Option<Hit> {
        let ([ix, iy], top) = (c.cell, c.top);
        let side = CELL.side();
        let base = CELL.base_of([ix, iy, 0]);
        let top = self.ground(GROUND.cell_of(0, &base)).value()?.top;
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
        let [kx, ky, _] = L2.index_of(&base);
        let col_idx = L2_IDX(TO_XY(base.i));
        let (cx, cy) = (ix - kx * cells_across(), iy - ky * cells_across());
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
                return Some(Hit {
                    t: from,
                    point: ray.at(from),
                    normal,
                    box_min: min,
                    box_max: min.map(|v| v + side),
                    struck: Struck::Ground,
                });
            }
            if leave >= t1 {
                return None;
            }
            cz += if dz > 0.0 { 1 } else { -1 };
            from = leave;
        }
    }

    fn folded(&self, ray: &Ray, [ix, iy]: [i64; 2], t0: f64, t1: f64, level: u32) -> Option<Hit> {
        let scale = GROUND.scale(level);
        let side = scale.side();
        let cell = GROUND.cell_of(level, &scale.base_of([ix, iy, 0]));
        let top = self.ground(cell).value()?.top;
        let ground = self.folded_ground(ray, [ix, iy], top, t0, t1, level);
        let lines = self.lines_over(cell, top);
        self.with_decks(ray, &lines.boxes, ([ix as f64 * side, iy as f64 * side], side), (t0, t1), ground)
    }

    fn folded_ground(&self, ray: &Ray, [ix, iy]: [i64; 2], top: f64, t0: f64, t1: f64, level: u32) -> Option<Hit> {
        let scale = GROUND.scale(level);
        let side = scale.side();
        let base = scale.base_of([ix, iy, 0]);
        let top = self.ground(GROUND.cell_of(level, &base)).value()?.top;
        let (za, zb) = (ray.at(t0)[2], ray.at(t1)[2]);
        let (t, normal) = if za <= top {
            (t0 > 0.0).then(|| (t0, wall_normal(ray, t0, side)))?
        } else if zb <= top {
            ((top - ray.origin[2]) / ray.direction[2], [0.0, 0.0, 1.0])
        } else {
            return None;
        };
        let p = ray.at(t);
        let cz = ((p[2] - normal[2] * NUDGE) / side).floor();
        let corner = [ix as f64 * side, iy as f64 * side];
        Some(Hit {
            t,
            point: p,
            normal,
            box_min: [corner[0], corner[1], cz * side],
            box_max: [corner[0] + side, corner[1] + side, (cz + 1.0) * side],
            struck: Struck::Ground,
        })
    }

    /// First object the ray meets, nearest first (`Store::along_ray`). At z16
    /// voxel entries are hit at their own boxes; above z16 the folded cell is
    /// hit whole, matching what that level draws.
    fn objects(&self, ray: &Ray, (t0, t1): (f64, f64), level: u32) -> Option<Hit> {
        let scale = GROUND.scale(level);
        let side = scale.side();
        let mut met = None;
        self.objects.along_ray(ray.origin, ray.direction, (t0, t1), scale, &mut |[x, y, z]| {
            met = if level == FINEST {
                let across = cells_across();
                let col_idx = pxl_spec::space::pvf3::ivec::gather(x.div_euclid(across), y.div_euclid(across), 0);
                let entries = self.cell_entries(col_idx, [x.rem_euclid(across), y.rem_euclid(across)], z);
                let corner = [(x.div_euclid(across) * across) as f64 * side, (y.div_euclid(across) * across) as f64 * side];
                object_hit(ray, &entries, corner)
            } else {
                let v = u32::from(self.objects.folded_value(scale, [x, y, z]));
                (object_value_is_voxel(v) && !object_value_is_air(v)).then(|| cell_hit(ray, [x, y, z], scale, t0)).flatten()
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
    fn cell_entries(&self, col_idx: IVec, [cx, cy]: [i64; 2], cz: i64) -> impl Iterator<Item = EntryCursor> + '_ {
        let run = self.objects.cell(col_idx, cx as u32, cy as u32, cz as CellZ);
        let e = run.and(self.objects.entries(col_idx));
        let ks = match (run, e) {
            (Some(run), Some(_)) => run.start as usize..(run.start + run.count) as usize,
            _ => 0..0,
        };
        let Some(e) = self.objects.entries(col_idx) else { return Vec::new() };
        (run.start as usize..(run.start + run.count) as usize)
            .filter(|&k| !object_value_is_decor(e.state[k]))
            .map(|k| {
                let mut cursor = EntryCursor::default();
                cursor.at_column(col_idx);
                cursor.load(e.band[k], e.pos[k], e.state[k]);
                cursor
            })
            .collect()
    }
}

fn cells_across() -> i64 {
    CELL.count_in(L2) as i64
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

fn object_hit(ray: &Ray, entries: impl Iterator<Item = EntryCursor>, [x0, y0]: [f64; 2]) -> Option<Hit> {
    let mut best: Option<Hit> = None;
    for e in entries.iter().filter(|e| !object_value_is_air(e.state)) {
        let min = [x0 + e.lx, y0 + e.ly, e.lz];
        let max = min.map(|v| v + e.size);
        let Some((t, normal)) = ray_box(ray, &min, &max) else { continue };
        if best.is_some_and(|b| b.t <= t) {
            continue;
        }
        best = Some(Hit {
            t,
            point: ray.at(t),
            normal,
            box_min: min,
            box_max: max,
            struck: Struck::Object(e.at()),
        });
    }
    best
}

fn ray_box(ray: &Ray, min: &V3, max: &V3) -> Option<(f64, V3)> {
    let (enter, _, k) = Aabb { min: *min, max: *max }.span_along(ray.origin, ray.direction)?;
    let mut normal = [0.0; 3];
    normal[k] = if ray.direction[k] > 0.0 { -1.0 } else { 1.0 };
    (enter >= 0.0 && enter <= ray.reach).then_some((enter, normal))
}

/// Where the ray meets the whole cell; if already inside at `from`, there,
/// through the nearest side. None if it starts inside.
fn cell_hit(ray: &Ray, cell: [i64; 3], scale: Scale, from: f64) -> Option<Hit> {
    let Aabb { min, max } = scale.cube(cell);
    let side = scale.side();
    let (t, normal) = match ray_box(ray, &min, &max) {
        Some((t, normal)) if t >= from => (t, normal),
        _ => {
            let p = ray.at(from);
            let inside = (0..3).all(|k| p[k] >= min[k] && p[k] <= max[k]);
            (inside && from > 0.0).then(|| (from, wall_normal(ray, from, side)))?
        }
    };
    Some(Hit {
        t,
        point: ray.at(t),
        normal,
        box_min: min,
        box_max: max,
        struck: Struck::Objects,
    })
}

#[cfg(test)]
#[path = "access.test.rs"]
mod test;
