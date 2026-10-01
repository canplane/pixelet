// pxl-kernel/world/access.rs

//! The access: one query of what is resident, in two forms — a layer's value
//! at a point at a level (`at`), and the first thing a ray meets with how far
//! along (`first_hit`) — and the conservative bound a descent is decided by
//! (`relief`). Every reader of the ground and the objects asks here: the
//! pointer's pick, the camera's clearance, the map center's footing, the
//! puck, the fog, the view's measure of how near ground is
//! (world/pyramid/domain.rs), and what the eye's cells see
//! (world/observer.rs).
//!
//! **A query answers and says what it lacked; it does nothing else.** It
//! reads what residency says is resident (world/paging.rs `drawn_from`,
//! `source`) and the object store, which is resident whole, and returns an
//! exact answer, a coarser one with the page it would have needed, or
//! nothing and that page. It never fetches, asks, prefetches or holds,
//! and keeps nothing between calls: it borrows residency shared, so it cannot
//! change it. Turning a lack into a request is demand's, at the caller, and
//! only where the caller relies on the page coming.
//!
//! **Two surfaces, kept apart.** What a query answers with is the surface
//! the records draw at the level asked — a level's cell, its height the
//! page's interpolant at the cell's center plus its shift, read from the
//! page the chunk there is painted from, by the paint's own function
//! (pxl-render mesh/paint/terrain/elevation.rs `ground_ele_at`) — so a pick,
//! a snap and a marker stand on what is on the screen. Whether to go down
//! into a piece of ground at all is decided on the page's index lane
//! (pxl-spec bake/format.rs `Index`), which holds the lowest and highest
//! anything under it stands at every level: a ray that passes over it passes
//! over everything finer there. The drawn surface averages, and a peak under
//! a coarse page can stand above it, so it never decides that a ray missed:
//! a coarse page whose bound the ray enters is a lack.
//!
//! Objects are the store's own records at their own tiers, sub-cell ones
//! included, as the paint lays them: a cell holding a voxel entry is that
//! entry's, the ground's voxel there replaced, and air takes it out. A placed
//! decor paints no voxel and is hit as it is drawn (layer/decor/hit.rs).
//! Placing and editing act on the records the leaf says, and a leaf that is
//! not resident is a lack they see (`Answer::exact`); a consumer that cannot
//! take a coarser answer is the one that would hold a page hard, and none
//! does yet (world/paging.rs `HARD_PAGES`).

use pxl_render::mesh::paint::terrain::elevation::{ground_ele_at, GroundLanes};
use pxl_spec::bake::constant::{CellZ, CELL, CELL_STEPS, OBJECT_FINEST, U_PER_CELL};
use pxl_spec::bake::deck::Occupies;
use pxl_spec::bake::fold::majority;
use pxl_spec::bake::format::{drawn, Terrain};
use pxl_spec::bake::pack::GroundCell;
use pxl_spec::cell::object::{object_value_is_air, object_value_is_decor, object_value_is_voxel};
use pxl_spec::space::hierarchy::Scale;
use pxl_spec::space::indexer::{L2, L2_IDX, L3};
use pxl_spec::space::lod::Tau;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pvf3::projection;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::pvf3::REACH;
use pxl_spec::space::pyramid::{Cell, LevelPage, EXTENT, GROUND, TOP_LEVEL};
use pxl_spec::space::v3::{Aabb, V3};
use pxl_spec::space::xy::TO_XY;

use crate::io::demand::Category;
use crate::kernel::kernel;
use crate::layer::object::store::{objects_revision, EntryAt, EntryCursor, Store};
use crate::param::STEP_CELLS;
use crate::world::paging::Paging;
use crate::world::pyramid::{Layer as _, Pyramid};

/// Terrain's finest level, z16: what a point observer — the pick, the map
/// center, the puck, the camera's clearance — asks the ground at, since
/// within their reach of the eye it is the level the ground is drawn at, and
/// all of them then stand on the one height.
pub const FINEST: u32 = 0;

/// How far past a boundary a ray is probed to say which piece of ground it
/// has entered, in units: far under the lattice's finest step (a 256th) and
/// far over a double's error at the coordinates the world spans.
const NUDGE: f64 = 1e-6;

/// The level a ray reads the ground at, along it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Detail {
    /// One level all along: a point observer's, which stands on what is
    /// drawn about it.
    Level(u32),
    /// The level the criterion wants at each distance from the ray's origin,
    /// at a quality (pxl-spec space/lod.rs `Tau::level`): a sample of the
    /// eye's, whose origin is the eye, sees far ground as coarse as it is
    /// drawn, in the cells it is drawn in, so a ray's walk takes about as
    /// many steps a level whatever the level.
    Distance(Tau),
}

impl Detail {
    /// The level at `t` along the ray, and how far along it holds: to where
    /// the next level's band begins.
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

/// What a query needed and was not resident: the layer's page at the level
/// asked — enough for demand to make it a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lack {
    pub layer: Category,
    pub page: LevelPage,
}

/// What a query answers. A coarser answer carries what it lacked, so none
/// can be taken for exact without saying so.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Answer<T> {
    /// From the page at the level asked, or the page that says its ground.
    Exact(T),
    /// From a page over it, drawn coarser, and the page it lacked.
    Coarser(T, Lack),
    /// Nothing resident answers it.
    Nothing(Lack),
}

impl<T> Answer<T> {
    /// What it answers with, exact or coarser: what is drawn there.
    pub fn value(self) -> Option<T> {
        match self {
            Answer::Exact(v) | Answer::Coarser(v, _) => Some(v),
            Answer::Nothing(_) => None,
        }
    }

    /// Only an exact answer: for a consumer that cannot act on a coarser one.
    pub fn exact(self) -> Option<T> {
        match self {
            Answer::Exact(v) => Some(v),
            _ => None,
        }
    }

    /// What it lacked, if anything.
    pub fn lack(&self) -> Option<Lack> {
        match self {
            Answer::Exact(_) => None,
            Answer::Coarser(_, l) | Answer::Nothing(l) => Some(*l),
        }
    }

    /// The same answer of what `f` makes of its value.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Answer<U> {
        match self {
            Answer::Exact(v) => Answer::Exact(f(v)),
            Answer::Coarser(v, l) => Answer::Coarser(f(v), l),
            Answer::Nothing(l) => Answer::Nothing(l),
        }
    }
}

/// What a node says of the ground over it (`Access::relief`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Relief {
    /// The lowest and highest the ground stands, in units. A page over the
    /// node holds the bound of everything under it, so read from one this
    /// is wider, never narrower, than the page's own will be.
    pub extent: (f64, f64),
    /// The steepest it can be, as a rise over a run along the ground: the
    /// largest span of any one index cell under the node over the cell's
    /// side. The surface is interpolated from elevation samples an index
    /// cell apart (pxl-spec bake/constant.rs `EXTENT_SPAN`), and an index
    /// cell's span bounds the window its interpolation reads, so no point of
    /// it rises faster than about that between two samples.
    pub slope: f64,
}

/// The ground a cell draws: the top of its column in units, and the ground
/// cell whose types it shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    pub top: f64,
    pub cell: GroundCell,
}

/// What the lines over a cell make of its column, in units: the boxes of
/// the decks over it, and the air a bore takes out of its ground.
#[derive(Default)]
struct Lines {
    boxes: Vec<[f64; 2]>,
    air: Option<[f64; 2]>,
}

/// A z16 cell column as the world composes it: the ground drawn there, the
/// lines over it, and where its objects are filed — what a ray through the
/// column and a footing on it both read, so the ground, a bore and a deck
/// are composed once. The objects are read at the resolution each asks: a
/// ray at their own boxes, a footing as they fold to the cell.
struct Composed {
    cell: [i64; 2],
    top: f64,
    lines: Lines,
    col_idx: IVec,
    /// The cell's place in its column of objects.
    local: [i64; 2],
}

/// A ray, in units: where it starts, a unit direction, and how far along it
/// is asked.
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

/// What a ray met.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Struck {
    Ground,
    /// A placed object, by where the store files it.
    Object(EntryAt),
    /// The objects under a cell of a level over z16, met at the cell they
    /// fold to there: a coarse level draws them folded, so none of them in
    /// particular is met.
    Objects,
}

/// Where a ray met something, in the ray's own frame: how far along, the
/// point, the face's outward normal, and the box struck — a ground voxel, or
/// the object's own box, exact.
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
    /// The hit with every point moved by `by`: into another frame.
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

/// What the queries read: terrain's residency and the object store, shared.
#[derive(Clone, Copy)]
pub struct Access<'a> {
    terrain: &'a Paging<Terrain>,
    objects: &'a Store,
    /// Where the rays cast through it count what they visit, where someone
    /// counts: each region read and each cell walked — the cost `first_hit`
    /// says a ray has, counted as it is paid.
    visits: Option<&'a std::cell::Cell<u64>>,
}

impl Access<'static> {
    /// What is resident now.
    pub fn now() -> Access<'static> {
        let pyramid: &'static Pyramid<Terrain> = Terrain::pyramid();
        let objects: &'static Store = &kernel().objects;
        Access::new(pyramid.paging(), objects)
    }
}

/// A piece of ground a ray is decided over at once: the square no page or
/// index cell it is read from changes across, and what the terrain there is
/// drawn from.
struct Region {
    /// The square's corner and side, in units.
    base: [f64; 2],
    side: f64,
    /// The page at the level asked.
    page: LevelPage,
    /// The index lane's lowest and highest over it, and whether it is read
    /// from the page asked; none where nothing over it is resident.
    terrain: Option<((f64, f64), bool)>,
}

/// Changes whenever what `Access::support` answers may: the terrain held
/// and the objects. What keeps a support asks again when it does.
pub fn support_revision() -> [u32; 2] {
    [Terrain::pyramid().revision(), objects_revision()]
}

/// Whether a cell whose sub-cells at the objects' finest are `filled` is:
/// the eight under each parent by majority, a level at a time up to the
/// cell, the tie by the level as the objects' fold has it (pxl-spec
/// bake/fold.rs `majority`).
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

    /// The same, counting into `visits` the regions and cells its rays
    /// visit.
    pub fn counting(self, visits: &'a std::cell::Cell<u64>) -> Access<'a> {
        Access { visits: Some(visits), ..self }
    }

    fn visit(&self) {
        if let Some(v) = self.visits {
            v.set(v.get() + 1);
        }
    }

    /// The ground `at` stands on, drawn at `level`: the level's cell it lies
    /// in, as the chunk there paints it (`Ground`).
    pub fn at(&self, at: &Pvf3, level: u32) -> Answer<Ground> {
        self.ground(GROUND.cell_of(level, at))
    }

    /// Where a thing stands at `at`: what a walker, the location puck and a
    /// move's destination stand on. It is asked on the 1u lattice, the cell
    /// the ground and a carve are made in, and a cell is solid where it is
    /// filled. What fills it is composed at the objects' finest, sub-cell by
    /// sub-cell, the authored over what is made: an object's leaf where one
    /// says a cube or air; under no leaf, the ground — under its top and out
    /// of a bore's air — or a deck. Then the filled sub-cells fold to the
    /// cell by majority, a level at a time as the objects' do
    /// (layer/object/store.rs `fold_cell`), but of filled against empty and
    /// not of which cube, so a cell seven eighths filled with three kinds of
    /// cube is filled, and a small cube or a small bite is what is seen and
    /// picked and not what is stood on. Decor is passed through: it is drawn
    /// and hit, and stood on nowhere.
    ///
    /// It starts from the cell `at` lies in, at its own height, and from
    /// there goes down through the empty cells under it to the first solid's
    /// top — a cave's floor, a carve, the ground — or, where that cell is
    /// solid, up onto what fills it if that is a step (param.rs
    /// `STEP_CELLS`). Taller, it is a wall, and the answer is none: the step
    /// is not taken. So a walker in a cave stays in it and stops at its
    /// walls; nothing sends it to the surface but the surface being what is
    /// under it.
    pub fn support(&self, at: &Pvf3) -> Answer<Option<f64>> {
        let [ix, iy, iz] = CELL.index_of(at);
        self.stand([ix, iy], Some(iz))
    }

    /// `support` from the surface drawn at `at`'s X and Y, its height
    /// discarded: where only a place is named and not how high — a fix, a
    /// search result — what stands there is what stands on the surface, or in
    /// a carve or a bore come out at it, or on a stack on it however tall,
    /// since nothing was walked into.
    pub fn surface_support(&self, at: &Pvf3) -> Answer<f64> {
        let [ix, iy, _] = CELL.index_of(at);
        // A stand from the surface meets no wall (`stand`), so it is never
        // none.
        self.stand([ix, iy], None).map(Option::unwrap_or_default)
    }

    /// `support` with no step to it: from the cell `at` lies in, down to the
    /// floor under it, or up over whatever fills it however tall — what an
    /// eye that walks nowhere has under it, in a cave as over a hill.
    pub fn floor_under(&self, at: &Pvf3) -> Answer<f64> {
        let [ix, iy, iz] = CELL.index_of(at);
        // Climbing a position's whole reach meets no wall (`stand`), so it
        // is never none.
        self.stand_climbing([ix, iy], Some(iz), true).map(Option::unwrap_or_default)
    }

    /// Where a thing stands in z16 column `cell`, starting from cell `from`
    /// of it and climbing a step at most, or from the surface and climbing
    /// any height where none; none where a step from `from` meets a wall.
    fn stand(&self, cell: [i64; 2], from: Option<i64>) -> Answer<Option<f64>> {
        self.stand_climbing(cell, from, from.is_none())
    }

    /// `stand`, climbing any height where `any_height` and a step otherwise.
    fn stand_climbing(&self, [ix, iy]: [i64; 2], from: Option<i64>, any_height: bool) -> Answer<Option<f64>> {
        self.composed([ix, iy]).map(|c| {
            let side = CELL.side();
            let top = (c.top / side).round() as i64;
            // What is made at height `z`: the ground and the lines over it.
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
            // As far as a position reaches, so a shaft or a stack of any
            // height is followed to its end.
            let [lo, hi] = [REACH.min[2] / side, REACH.max[2] / side].map(|z| z as i64);
            let start = from.unwrap_or(top);
            let climb = if any_height { hi - start } else { STEP_CELLS };
            // Nothing is filled over the ground's top, the lines' highest and
            // the objects' highest band: a start over all three goes down
            // from there, not a cell at a time from a height far over them.
            let lines_top = c.lines.boxes.iter().map(|b| (b[1] / side).ceil() as i64).max().unwrap_or(i64::MIN);
            let ceiling = top.max(lines_top).max(self.objects.ceiling([ix, iy]).unwrap_or(i64::MIN));
            let stand = if start < ceiling && solid(start) {
                let over = (start + 1..=start + climb).find(|&cz| !solid(cz));
                // Climbing any height, a stack filling a position's whole
                // reach is stood in.
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

    /// The z16 column `cell`, composed.
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

    /// Whether the column's cell at height `cz` is ground where the ground
    /// reaches it: neither in the air a bore takes out nor taken by an
    /// object's voxel entry, air as much as a solid.
    fn holds(&self, c: &Composed, cz: i64) -> bool {
        let z = cz as f64 * CELL.side();
        !c.lines.air.is_some_and(|[lo, hi]| z >= lo && z < hi) && !self.taken(c.col_idx, c.local, cz)
    }

    /// The ground a cell draws, from its page or the nearest resident page
    /// over it: the height at the cell's center in that page's texels, as
    /// pxl-render mesh/paint/terrain/adapter.rs `ChunkSample::read` samples a
    /// chunk read from a page over it, and the type of the page cell it lies
    /// in.
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

    /// The level the ground under `at` is known at: z16's where its own
    /// page says it exactly, a coarser one's where only a page over it is
    /// resident, none where nothing over it is. What a step there can mean is
    /// bounded by it: a cell of that level is the finest thing known.
    pub fn known_level(&self, at: &Pvf3) -> Option<u32> {
        let page = GROUND.cell_of(FINEST, at).page();
        match self.terrain.source(page) {
            Some(_) => Some(FINEST),
            None => self.drawn_from(page).map(|(src, _)| src.level),
        }
    }

    /// The resident page `page`'s ground is drawn from, and its body.
    fn drawn_from(&self, page: LevelPage) -> Option<(LevelPage, &'a Terrain)> {
        let src = self.terrain.drawn_from(page)?;
        Some((src, self.terrain.get(src)?))
    }

    /// `v`, read from what `page` is drawn from: exact where that says
    /// `page`'s ground (`Paging::source`), and lacking `page` where it is a
    /// page over it.
    fn answer<T>(&self, page: LevelPage, v: T) -> Answer<T> {
        match self.terrain.source(page) {
            Some(_) => Answer::Exact(v),
            None => Answer::Coarser(v, Lack { layer: Category::terrain, page }),
        }
    }

    /// What the ground over a node of `page`'s level can be, at that level
    /// and every level under it, from the index lane of the page it is drawn
    /// from over the node's footprint: one block of it (pxl-spec
    /// bake/format.rs `Index::over`), whatever the node's size. For deciding
    /// what a piece of the view holds, how near it comes and how far it can
    /// be from the ground it stands for, never for what is drawn.
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

    /// The first thing `ray` meets within its reach, at the level `detail`
    /// says along it: the ground and the objects, each by a walk of its own
    /// shape, the nearer met. The objects are resident whole and their
    /// cells are found down their index along the ray (`objects`); the
    /// ground is paged, and is walked only as far as the objects' nearest,
    /// since what lies past that is behind it. So the lack is the ground's
    /// nearest one before what is met: an object is never answered exactly
    /// over ground not known to lie behind it. What the ray starts inside is
    /// not met: the object or the cell it is in, and the ground of the
    /// column it is in, whose ground it meets again only in the next column
    /// it enters, so an eye under the ground is not stopped where it stands.
    ///
    /// The ground is walked region by region: a region is passed over whole
    /// where the ray stays above everything its index lane says can stand
    /// there, and walked cell column by cell column of its level where it
    /// does not. A region read from a page coarser than asked, whose bound
    /// the ray enters, is the lack — the nearest one, since what is under it
    /// could be met before anything past it — and a region nothing over is
    /// resident for is one too, whatever the ray's height, since nothing
    /// bounds it.
    ///
    /// The level is one along each band of distance, and a region ends where
    /// the band does, so a walk is in the cells of the level at its start,
    /// which is the finest over it.
    ///
    /// Before a region of the level asked is read, the pages over it that are
    /// resident are asked, coarsest first, whether the ray passes over all
    /// they hold (`clear`): an index cell of a page `k` levels up is `2^k`
    /// of the level's across, so ground far under the ray is passed in as
    /// many tests as there are levels over it, not a region at a time. A
    /// page over it that is not resident proves nothing and is passed by; it
    /// never lets the ray pass over what it would have held. What a ray
    /// costs is the regions and cells it visits, which is fewest far over
    /// the ground and most along a slope it grazes.
    pub fn first_hit(&self, ray: &Ray, detail: Detail) -> Answer<Option<Hit>> {
        let mut lack: Option<Lack> = None;
        let mut answered = false;
        let mut t = 0.0;
        while t < ray.reach {
            let (level, until) = detail.at(t);
            let end = until.min(ray.reach);
            let objects = self.objects(ray, (t, end), level);
            let limit = objects.map_or(end, |o| o.t);
            // The ground where it is met no farther than the objects' nearest.
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

    /// The first of the ground the ray meets between `t0` and `t1` at
    /// `level`, noting the nearest lack and whether any of it was answered.
    fn ground_first(&self, ray: &Ray, (t0, t1): (f64, f64), level: u32, lack: &mut Option<Lack>, answered: &mut bool) -> Option<Hit> {
        let mut t = t0;
        while t < t1 {
            self.visit();
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

    /// Where the ray, at `t` and asking at `level` until `until`, is proved
    /// to pass over all the ground under an index cell of a resident page
    /// over `level`: the far edge of the largest such cell, or none. The
    /// ground is solid under its surface, so it is passed over only above
    /// its highest. An index cell bounds what is drawn at its page's level
    /// and every level under it, and the ray asks at `level` up to `until`
    /// only, so no cell carries it past there.
    fn clear(&self, ray: &Ray, t: f64, level: u32, until: f64) -> Option<f64> {
        let p = ray.at(t + NUDGE);
        let mut at = Pvf3::default();
        projection::project(&mut at, p[0], p[1], 0.0);
        (level + 1..=TOP_LEVEL).rev().find_map(|lv| {
            let body = self.terrain.get(GROUND.cell_of(lv, &at).page())?;
            let e = EXTENT.cell_of(lv, &at);
            let [_, hi] = drawn(body.index.extent()[e.index() as usize]);
            let scale = e.scale();
            let [ix, iy, _] = scale.index_of(&at);
            let base = scale.base_of([ix, iy, 0]);
            let base = [pvf3::to_real(pvf3::X(&base)), pvf3::to_real(pvf3::Y(&base))];
            let out = exit(ray, base, scale.side()).min(until).max(t + NUDGE);
            let (za, zb) = (ray.at(t)[2], ray.at(out)[2]);
            (za.min(zb) > hi as f64 * U_PER_CELL).then_some(out)
        })
    }

    /// The region a point lies in, for a ray asking at `level`.
    fn region(&self, p: &V3, level: u32) -> Region {
        let mut at = Pvf3::default();
        projection::project(&mut at, p[0], p[1], 0.0);
        let page = GROUND.cell_of(level, &at).page();
        let page_scale = L3.up(level as i32);
        let (scale, terrain) = match self.drawn_from(page) {
            Some((src, body)) => {
                // The index cell of the page it is drawn from, or the page
                // asked where that is the smaller: either way one bound and
                // one page across it.
                let e = EXTENT.cell_of(src.level, &at);
                let [lo, hi] = drawn(body.index.extent()[e.index() as usize]);
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

    /// The first of the ground met between `t0` and `t1` along the ray
    /// inside region `r`, cell column by cell column of `level` in the order
    /// the ray crosses them (Amanatides–Woo). The walk stops at the region's edge by
    /// the cells it holds and not by `t1`: a step's distance and the edge's
    /// are summed two ways, and a column past the edge taken for inside would
    /// be read as the region's, exact or not. A region is at least a cell of
    /// its level across: its index cell is the page's it is read from, at
    /// that level or over it.
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
            self.visit();
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

    /// What the lines over a cell make of its column, as the chunk there
    /// paints them (pxl-spec bake/deck.rs `Over::occupies`), in units, read
    /// from the page the cell is drawn from at the cell's center: each
    /// deck's slab and pier, a box to meet, and the air a bore takes out of
    /// the column's ground.
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

    /// The nearest of the decks over a cell column the ray meets while it
    /// is over the column, between `t0` and `t1`, beside `ground`, the
    /// column's own hit: each slab or pier a box the column's width.
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

    /// Where the ray first meets the ground or a deck in z16 cell column
    /// `cell` while it is over it, between `t0` and `t1`.
    fn column(&self, ray: &Ray, [ix, iy]: [i64; 2], t0: f64, t1: f64) -> Option<Hit> {
        let side = CELL.side();
        let c = self.composed([ix, iy]).value()?;
        let ground = self.ground_column(ray, &c, t0, t1);
        self.with_decks(ray, &c.lines.boxes, ([ix as f64 * side, iy as f64 * side], side), (t0, t1), ground)
    }

    /// Where the ray first meets the ground in z16 cell column `cell` while
    /// it is over it, between `t0` and `t1`: a cell under the column's drawn
    /// top that no voxel entry takes, since a cell holding one is its
    /// objects' and not the ground's — air as much as a solid. The ray is
    /// stepped a cell of height at a time only once it is under the top.
    /// What the ray starts inside is not met, and nor is the air a bore
    /// takes out of the column.
    fn ground_column(&self, ray: &Ray, c: &Composed, t0: f64, t1: f64) -> Option<Hit> {
        let ([ix, iy], top) = (c.cell, c.top);
        let side = CELL.side();
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
            // It starts inside the ground.
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
                // In through the column's side where this is the cell the ray
                // came into the column at, and through the cell's floor or
                // ceiling past it.
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

    /// Where the ray first meets the ground or a deck in cell column `cell`
    /// of `level`, over z16, while it is over it, between `t0` and `t1`.
    fn folded(&self, ray: &Ray, [ix, iy]: [i64; 2], t0: f64, t1: f64, level: u32) -> Option<Hit> {
        let scale = GROUND.scale(level);
        let side = scale.side();
        let cell = GROUND.cell_of(level, &scale.base_of([ix, iy, 0]));
        let top = self.ground(cell).value()?.top;
        let ground = self.folded_ground(ray, [ix, iy], top, t0, t1, level);
        let lines = self.lines_over(cell, top);
        self.with_decks(ray, &lines.boxes, ([ix as f64 * side, iy as f64 * side], side), (t0, t1), ground)
    }

    /// Where the ray first meets the ground in cell column `cell` of
    /// `level`, over z16, while it is over it, between `t0` and `t1`: the
    /// column's cells under the top the level draws. What the ray starts
    /// inside is not met.
    fn folded_ground(&self, ray: &Ray, [ix, iy]: [i64; 2], top: f64, t0: f64, t1: f64, level: u32) -> Option<Hit> {
        let scale = GROUND.scale(level);
        let side = scale.side();
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

    /// The first of the objects the ray meets between `t0` and `t1` at
    /// `level`, from the cells their index says the ray passes, nearest
    /// first (layer/object/store.rs `Store::along_ray`). At z16 a cell's voxel
    /// entries are met at their own boxes, sub-cell ones included, air
    /// meeting nothing. Over z16 the level draws its objects folded into its
    /// own cells, as the store keeps them (`fold_cell`), so a cell that
    /// paints a material is met at its own box — the ground's cells and the
    /// objects' at one resolution, the one the mesher then merges 2 × 2 × 2
    /// by what each bucket covers, whichever layer it came from. A cell the
    /// ray is already in where the band of its level begins is met there,
    /// through the side it is nearest, as the ground's is; what the ray
    /// starts inside is not met.
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
                object_hit(ray, entries, corner)
            } else {
                let v = u32::from(self.objects.folded_value(scale, [x, y, z]));
                (object_value_is_voxel(v) && !object_value_is_air(v)).then(|| cell_hit(ray, [x, y, z], scale, t0)).flatten()
            };
            met.is_some()
        });
        met
    }

    /// Whether z16 cell `cz` of column-local (cx, cy) holds a voxel entry,
    /// which takes the cell from the ground.
    fn taken(&self, col_idx: IVec, [cx, cy]: [i64; 2], cz: i64) -> bool {
        let Some(run) = self.objects.cell(col_idx, cx as u32, cy as u32, cz as CellZ) else { return false };
        let Some(e) = self.objects.entries(col_idx) else { return false };
        (run.start as usize..(run.start + run.count) as usize).any(|k| !object_value_is_decor(e.state[k]))
    }

    /// The voxel entries of the cell at column-local (cx, cy) and height
    /// `cz`: each placed thing that paints a voxel, with where it is filed.
    /// A decor paints none, so a cell of decor alone is the ground's.
    fn cell_entries(&self, col_idx: IVec, [cx, cy]: [i64; 2], cz: i64) -> impl Iterator<Item = EntryCursor> + '_ {
        let run = self.objects.cell(col_idx, cx as u32, cy as u32, cz as CellZ);
        let e = run.and(self.objects.entries(col_idx));
        let ks = match (run, e) {
            (Some(run), Some(_)) => run.start as usize..(run.start + run.count) as usize,
            _ => 0..0,
        };
        ks.filter_map(move |k| {
            let e = e?;
            (!object_value_is_decor(e.state[k])).then(|| {
                let mut cursor = EntryCursor::default();
                cursor.at_column(col_idx);
                cursor.load(e.band[k], e.pos[k], e.state[k]);
                cursor
            })
        })
    }
}

/// Cells along a column's side.
fn cells_across() -> i64 {
    CELL.count_in(L2) as i64
}

/// Where along the ray it leaves the square at `base` of `side`, in XY.
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

/// The face a ray entering a cell column `side` wide at `t` came through:
/// the side it lies nearest to, facing back along the ray.
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

/// The nearest solid of a cell's voxel entries the ray enters, or none: air
/// is nothing to meet, and the cell is empty past what is placed in it.
/// `corner` is the column's, in units, which the entries' corners are from.
fn object_hit(ray: &Ray, entries: impl Iterator<Item = EntryCursor>, [x0, y0]: [f64; 2]) -> Option<Hit> {
    let mut best: Option<Hit> = None;
    for e in entries.filter(|e| !object_value_is_air(e.state)) {
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

/// Where the ray enters a box and through which face; none where it misses
/// the box, leaves it before its reach begins or starts inside it.
fn ray_box(ray: &Ray, min: &V3, max: &V3) -> Option<(f64, V3)> {
    let (enter, _, k) = Aabb { min: *min, max: *max }.span_along(ray.origin, ray.direction)?;
    let mut normal = [0.0; 3];
    normal[k] = if ray.direction[k] > 0.0 { -1.0 } else { 1.0 };
    (enter >= 0.0 && enter <= ray.reach).then_some((enter, normal))
}

/// Where the ray meets the cell of `scale` at `cell`, whole, and through
/// which face: where it enters it, or `from` where it is inside it there,
/// through the side it is nearest; none where it misses it, or starts
/// inside it.
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
