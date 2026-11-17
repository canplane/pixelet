// pxl-core/domain/query/support.rs

//! Where something stands: the support, the floor and the ceiling over a
//! point, found by climbing as a walker would. Each is asked of the point's
//! cell, split into columns, with a ray straight up or down through each
//! column (`SplitCell`). That ray is the one every reader casts
//! (`World::along`). In any column it finds the first cell that fills or,
//! starting inside what fills, the first cell that does not.
//!
//! At the leaf, the cell is split at the finest zoom anything is placed at in
//! it, so the world is read as finely as it is held (query/ray.rs `finest`).
//! A walker, one cell wide, then stands on what is placed whatever its size.
//! At a coarser zoom the cell is one column, read as that zoom's cells are
//! (`at_zoom`), with what is placed finer folded into it. Either way, a cell
//! filled short of its cube stands to the top of the box it is drawn as
//! (pxl-engine `cast::top`).

use pxl_spec::space::camera::grid::cell_zoom_at;
use pxl_spec::space::coord::q3::{self, FRAC_W, Q3};
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::world::{BOUND, ROUND};
use pxl_spec::space::coord::zoom::{LEAF_ZOOM, Zoom, units};
use pxl_spec::space::layer::lattice::GROUND;
use xpute_kit::math::scalar::wrap;

use pxl_spec::space::coord::aabb::Aabb;

use super::ray::{Descent, Ray, Until, at_zoom, finest};
use super::world::World;
use crate::domain::layer::tile_cache::Path;
use crate::domain::layer::*;
use pxl_content::scatter::box_stands_at;
use pxl_engine::cast::{Detail, fill_bound};
use pxl_spec::space::layer::quadrant::Quadrant;

/// Whether a walker stands `dist` units from the eye. A walker fills a leaf
/// cell and stands as a decor of that box placed there is drawn (pxl-content
/// `generation::drawn_at`, `scatter::box_stands_at`): where the eye's rays
/// ask the leaves' zoom, or a coarser one whose cell a cube of its box keeps
/// an eighth of. The zoom asked is the camera grid's at that distance,
/// whatever zooms the source holds. A character is drawn, and a walk is
/// walked, only where this holds.
pub(crate) fn walker_stands(dist: f64) -> bool {
    box_stands_at(
        [1, 1, 1],
        GROUND.leaf_cell_zoom(),
        cell_zoom_at((dist * dist) as f32),
    )
}

/// The height of a walker's center standing on a floor whose top is `top`:
/// half a leaf cell up, as a cell's center stands over its ground.
pub(crate) fn over(top: f64) -> f64 {
    top + units(GROUND.leaf_cell_zoom()) / 2.0
}

/// The floor under a walker's center at `z`, the inverse of `over`.
pub(super) fn under(z: f64) -> f64 {
    z - units(GROUND.leaf_cell_zoom()) / 2.0
}

/// The Z, in Q3 steps, that marks a point whose height is not known. It is
/// the bottom of the world (`world::BOUND`), below any ground.
const UNKNOWN_Z: i64 = (BOUND.min[2] as i64) << FRAC_W;

fn with_z(at: &Q3, z: i64) -> Q3 {
    world::at(i64::from(at[0]), i64::from(at[1]), z)
}

pub(crate) fn unknown(at: &Q3) -> Q3 {
    with_z(at, UNKNOWN_Z)
}

pub(crate) fn is_unknown(at: &Q3) -> bool {
    i64::from(at[2]) == UNKNOWN_Z
}

/// The top of the floor under a standing point `at`.
pub(crate) fn floor_of(at: &Q3) -> f64 {
    under(q3::to_real(i64::from(at[2])))
}

pub(crate) fn on_floor(at: &Q3, top: f64) -> Q3 {
    with_z(at, q3::from_real(over(top)))
}

/// How far `eye` is from the farther end of a walk from `from` to over `to`,
/// at `from`'s height, X taken the short way round (`world::between`).
fn farther(eye: &Q3, from: &Q3, to: &Q3) -> f64 {
    let (f, t) = (world::between(eye, from), world::between(eye, to));
    let far = |[x, y, _]: [i64; 3]| {
        let [x, y, z] = [x, y, f[2]].map(q3::to_real);
        libm::hypot(libm::hypot(x, y), z)
    };
    far(f).max(far(t))
}

/// A point's place on the ground, in units.
fn xy(at: &Q3) -> [f64; 2] {
    [q3::to_real(i64::from(at[0])), q3::to_real(i64::from(at[1]))]
}

/// The cell over `xy` among the cells of the tile zoom `zoom`, split into
/// columns as the module doc says: at the leaf, `across` columns a side at
/// the finest zoom anything is placed at in it; at a coarser zoom, one
/// column. A walker meets what a ray straight up or down through a column's
/// middle meets. The columns keep the first tile an answer lacked. Each
/// ray's tile lines step on from the last ray's (`path`), which were handed
/// on from the columns these were made beside, since the rays climb the same
/// tiles over the same tile cache.
struct SplitCell<'w> {
    world: &'w World,
    corner: [f64; 2],
    side: f64,
    across: u32,
    detail: Detail,
    lack: Option<Quadrant>,
    zoom: Zoom,
    path: Path,
    /// Where the last column's ray ended. A ray from the same point goes on
    /// from there, and `lack` holds what the descent to there lacked.
    descent: Option<Descent>,
}

impl<'w> SplitCell<'w> {
    fn new(world: &'w World, xy: [f64; 2], zoom: Zoom, path: Path) -> SplitCell<'w> {
        let cell_zoom = GROUND.cell_zoom(zoom);
        let side = units(cell_zoom);
        let corner = xy.map(|v| libm::floor(v / side) * side);
        let column = Aabb {
            min: [corner[0], corner[1], BOUND.min[2]],
            max: [corner[0] + side, corner[1] + side, BOUND.max[2]],
        };
        let read = if zoom == LEAF_ZOOM {
            finest()
        } else {
            at_zoom(zoom)
        };
        let finer = world
            .finest_in(&column)
            .map_or(0, |z| z.min(read.range[1]).saturating_sub(cell_zoom));
        SplitCell {
            world,
            corner,
            side: side / f64::from(1u32 << finer),
            across: 1 << finer,
            detail: at_zoom(zoom + finer),
            lack: None,
            zoom,
            path,
            descent: None,
        }
    }

    /// For each column, the box of the first cell that its ray from the
    /// height `z` to the height `to`, up or down, stops at (`until`).
    fn rays(&mut self, z: f64, to: f64, until: Until) -> impl Iterator<Item = Aabb> + use<'_, 'w> {
        (0..self.across * self.across)
            .filter_map(move |k| self.meet(k, z, to, until).val().flatten())
    }

    /// What column `k`'s ray meets, with what the world held along the way.
    /// A ray that meets nothing before a lack tells nothing past it.
    fn meet(&mut self, k: u32, z: f64, to: f64, until: Until) -> Answer<Option<Aabb>> {
        let [x, y] = self.middle(k);
        let ray = Ray {
            origin: [x, y, z],
            dir: [0.0, 0.0, if to > z { 1.0 } else { -1.0 }],
            t_max: (to - z).abs(),
        };
        let answer = self.world.along(
            &ray,
            self.detail,
            until,
            None,
            &mut self.path,
            &mut self.descent,
        );
        if let Some(l) = answer.lack() {
            self.lack.get_or_insert(l);
        }
        answer.map(|h| h.map(|h| h.cube))
    }

    fn middle(&self, k: u32) -> [f64; 2] {
        let at = |i: u32, c: f64| c + (f64::from(i) + 0.5) * self.side;
        [
            at(k % self.across, self.corner[0]),
            at(k / self.across, self.corner[1]),
        ]
    }

    /// The drawn top of column `k`'s cell under the height `open`, at the
    /// zoom its rays read (pxl-engine `cast::top`): the cube's top where the
    /// cell fills its cube whole, else the top of its fill box. `open` where
    /// what the cell holds is not known.
    fn top_under(&mut self, k: u32, open: f64) -> f64 {
        let zoom = self.detail.range[1];
        let side = units(zoom);
        let [x, y] = self.middle(k);
        let cell = [x, y, open - side / 2.0].map(|v| libm::floor(v / side) as i32);
        let answer = self.world.asset((zoom, cell));
        if let Some(l) = answer.lack() {
            self.lack.get_or_insert(l);
        }
        answer.val().map_or(open, |asset| {
            let (_, hi) = fill_bound(([0.0; 3], [1.0; 3]), asset);
            (f64::from(cell[2]) + f64::from(hi[2])) * side
        })
    }

    /// The height a walker at `z` rises to out of what fills there in any
    /// column. It climbs out of each column's fill in turn, until no column
    /// fills where the walker stands. A column's fill ends where its first
    /// open cell from `z` up begins, at the drawn top of the cell under that
    /// one (`top_under`); a column whose cell at `z` does not fill leaves `z`
    /// as it is. A column counts as filled to the top of the world's bound
    /// only where the world holds it so: a way into tiles not held fills
    /// nothing, since what a zoom does not hold is absent there.
    ///
    /// With it, where the walker did not rise: the highest drawn top at or
    /// under `z` of a cell the walker stands within, a cell filled short of
    /// its cube. A ray down from `z` to the first solid never stops in the
    /// cell it starts in (pxl-engine `cast::walk`), so `below` takes that
    /// top from here.
    fn lift(&mut self, mut z: f64) -> (f64, Option<f64>) {
        loop {
            let (mut top, mut cell_top) = (z, None::<f64>);
            for k in 0..self.across * self.across {
                match self.meet(k, z, BOUND.max[2], Until::Transparent) {
                    Answer::Exact(None) => top = BOUND.max[2],
                    Answer::Exact(Some(b)) | Answer::Coarser(Some(b), _) if b.min[2] > z => {
                        match self.top_under(k, b.min[2]) {
                            t if t > z => top = top.max(t),
                            t => cell_top = Some(cell_top.map_or(t, |o| o.max(t))),
                        }
                    }
                    _ => {}
                }
            }
            if top == z {
                return (z, cell_top);
            }
            z = top;
        }
    }

    /// The top of the highest solid at or under `z` in any column, above the
    /// bottom of the world's bound: `cell_top`, the top `lift` found within
    /// the cells at `z`, or what the rays down from `z` meet, which start in
    /// the cell over that solid.
    fn below(&mut self, z: f64, cell_top: Option<f64>) -> Option<f64> {
        let met = self
            .rays(z, BOUND.min[2], Until::Opaque)
            .map(|b| b.max[2])
            .reduce(f64::max);
        [cell_top, met]
            .into_iter()
            .flatten()
            .filter(|&t| t > BOUND.min[2])
            .reduce(f64::max)
    }

    /// The bottom of the lowest solid over `z` and under `to` in any column.
    /// The rays go up from `z` and start in the cell at `z`, so they never
    /// meet a solid at `z` itself.
    fn above(&mut self, z: f64, to: f64) -> Option<f64> {
        self.rays(z, to, Until::Opaque)
            .map(|b| b.min[2])
            .filter(|&b| b < to)
            .reduce(f64::min)
    }

    /// The top of the highest solid in any column, within the world's
    /// bound. It is
    /// each column's surface at its zoom, the one support every reader asks
    /// (`World::column_surface`, pxl-engine `support::column_surface`).
    fn highest(&mut self) -> Option<f64> {
        let cell_zoom = self.detail.range[0];
        let unit = units(cell_zoom);
        let mut most: Option<f64> = None;
        for k in 0..self.across * self.across {
            let middle = |i: u32, c: f64| c + (f64::from(i) + 0.5) * self.side;
            let g = [
                middle(k % self.across, self.corner[0]),
                middle(k / self.across, self.corner[1]),
            ]
            .map(|v| libm::floor(v / unit) as i32);
            let (answer, _) = self.world.column_surface(cell_zoom, g);
            if let Some(l) = answer.lack() {
                self.lack.get_or_insert(l);
            }
            if let Some(Some((at, rise))) = answer.val() {
                let top = ((f64::from(at) + f64::from(rise)) * unit).min(BOUND.max[2]);
                most = Some(most.map_or(top, |m| m.max(top)));
            }
        }
        most
    }

    /// The top of the cell's ground, in units.
    fn surface(&mut self) -> Option<f64> {
        let cell_zoom = GROUND.cell_zoom(self.zoom);
        let side = units(cell_zoom);
        let answer = self.world.surface_of(
            cell_zoom,
            self.corner
                .map(|v| libm::floor((v + side / 2.0) / side) as i32),
        );
        if let Some(l) = answer.lack() {
            self.lack.get_or_insert(l);
        }
        answer.val().flatten().map(|s| s.top)
    }

    /// `v`, with the first tile its answers lacked.
    fn answer<T>(&self, v: T) -> Answer<T> {
        match self.lack {
            None => Answer::Exact(v),
            Some(l) => Answer::Coarser(v, l),
        }
    }
}

impl World {
    /// What is in the leaf cell at `at`, as the cast draws it (pxl-engine
    /// `world::node`), or void where nothing is.
    pub(crate) fn asset_of(&self, at: &Q3) -> Answer<u32> {
        let side = units(GROUND.leaf_cell_zoom());
        let p = [i64::from(at[0]), i64::from(at[1]), i64::from(at[2])].map(q3::to_real);
        self.asset((
            GROUND.leaf_cell_zoom(),
            p.map(|v| (v / side).floor() as i32),
        ))
    }

    /// Where the point `at` stands: on the surface where its height is not
    /// known (`surface_support`), and otherwise where it is.
    pub(crate) fn stands(&self, at: &Q3) -> Answer<Option<Q3>> {
        match is_unknown(at) {
            true => self
                .surface_support(at)
                .map(|top| top.map(|top| on_floor(at, top))),
            false => Answer::Exact(Some(*at)),
        }
    }

    /// Where a walker stands when it is put on the drawn surface at `at`
    /// whatever its height, as a fix or a search result is. It climbs out of
    /// what fills the surface, or else drops onto the first solid under it.
    /// None where nothing stands: a cell with no surface and nothing placed.
    pub(crate) fn surface_support(&self, at: &Q3) -> Answer<Option<f64>> {
        let mut c = SplitCell::new(self, xy(at), LEAF_ZOOM, Path::default());
        let [lo, hi] = [BOUND.min[2], BOUND.max[2]];
        let surface = c.surface();
        // With no surface, the search starts at the highest solid, or at the
        // bottom of the world's bound.
        let start = surface.or_else(|| c.highest()).unwrap_or(lo).max(lo);
        let stands = match c.lift(start) {
            (z, _) if z > start => Some(z.min(hi)),
            (_, cell_top) => c.below(start, cell_top).or(surface.map(|_| start)),
        };
        c.answer(stands)
    }

    /// The floor a center at `at` stands on, as the frame draws the world at
    /// the tile zoom `zoom`: up out of what fills `at`, or else down to the
    /// first solid's top. None where nothing stands under `at`.
    pub(crate) fn floor_under(&self, at: &Q3, zoom: Zoom) -> Answer<Option<f64>> {
        let start = q3::to_real(i64::from(at[2]));
        let mut c = SplitCell::new(self, xy(at), zoom, Path::default());
        let stands = match c.lift(start) {
            (z, _) if z > start => Some(z.min(BOUND.max[2])),
            (z, cell_top) => c.below(z, cell_top),
        };
        c.answer(stands)
    }

    /// The top of the highest solid in `at`'s column, whatever `at`'s height.
    /// None where nothing stands there.
    pub(crate) fn column_top(&self, at: &Q3) -> Answer<Option<f64>> {
        let mut c = SplitCell::new(self, xy(at), LEAF_ZOOM, Path::default());
        let top = c.highest();
        c.answer(top)
    }

    /// The floor nearest `at`'s height: the top of a solid with room over
    /// it, under `at` or over it, or the cell's surface. None where nothing
    /// stands.
    pub(crate) fn support_nearest(&self, at: &Q3) -> Answer<Option<f64>> {
        let z = q3::to_real(i64::from(at[2]));
        let mut c = SplitCell::new(self, xy(at), LEAF_ZOOM, Path::default());
        let down = match c.lift(z) {
            (t, _) if t > z => Some(t.min(BOUND.max[2])),
            (t, cell_top) => c.below(t, cell_top),
        };
        let up = c.above(z, BOUND.max[2]).map(|a| c.lift(a).0);
        let nearest = [down, up, c.surface()]
            .into_iter()
            .flatten()
            .min_by(|a, b| (a - z).abs().total_cmp(&(b - z).abs()));
        c.answer(nearest)
    }

    /// Where a walker standing at the center `from` ends when it goes toward
    /// `to`, one leaf cell at a time. A `from` whose height is not known
    /// stands on the surface.
    ///
    /// The rule follows Quake III's PM_StepSlideMove (id Software,
    /// code/game/bg_slidemove.c), which steps up and slides a blocked axis
    /// along the wall as PM_ClipVelocity clips the velocity. Here the step up
    /// has no height limit but the ceiling's: it gives up where its own
    /// cell's ceiling stops the climb, and the walker settles on the floor.
    ///
    /// With `walled`, as for what walks in the world, the walker never leaves
    /// the service area (`within`) and slides along its edge; one already
    /// past the area is not walled until it is back in. The camera is not
    /// walled and goes anywhere a position can be, past the area over the
    /// Earth, which holds nothing above sea level.
    ///
    /// The walk is walked only where a walker stands for `eye` at both its
    /// ends (`walker_stands`), so it is no longer than the eye draws a walker
    /// across; a `from` of unknown height is stood first, the one read such
    /// a move makes. Otherwise, and where `from` cannot be stood or a leaf on
    /// the way is not held, there is no walker for the eye and no wall its
    /// size: it ends at `to` with its height not known, and one `walled`
    /// stays where it is rather than leave the area.
    pub(crate) fn step_slide(&self, from: &Q3, to: &Q3, eye: &Q3, walled: bool) -> Q3 {
        self.stands(from)
            .exact()
            .flatten()
            .filter(|from| walker_stands(farther(eye, from, to)))
            .and_then(|from| self.slide(&from, to, walled))
            .unwrap_or_else(|| match walled && self.leaves_area(xy(from), xy(to)) {
                true => *from,
                false => unknown(to),
            })
    }

    /// Whether going from `from` to `to` leaves the service area. For what is
    /// walled and within the area, its edge is a wall, slid along as any wall
    /// is; one already past the area goes out of there freely.
    fn leaves_area(&self, from: [f64; 2], to: [f64; 2]) -> bool {
        let within =
            |p: [f64; 2]| self.within(&world::at(q3::from_real(p[0]), q3::from_real(p[1]), 0));
        !within(to) && within(from)
    }

    /// `step_slide` from a height known, None where it has no height to end at.
    fn slide(&self, from: &Q3, to: &Q3, walled: bool) -> Option<Q3> {
        let [sx, sy, sz] =
            [i64::from(from[0]), i64::from(from[1]), i64::from(from[2])].map(q3::to_real);
        let mut floor = under(sz);
        let gather = |x: f64, y: f64, floor: f64| {
            world::at(
                q3::from_real(x),
                q3::from_real(y),
                q3::from_real(over(floor)),
            )
        };
        let [tx, ty] = [i64::from(to[0]), i64::from(to[1])].map(q3::to_real);
        // The short way round the world, across the antimeridian too.
        let way = [wrap(tx - sx, -ROUND / 2.0, ROUND / 2.0), ty - sy];
        let s = units(GROUND.leaf_cell_zoom());
        let center = |c: [f64; 2]| [(c[0] + 0.5) * s, (c[1] + 0.5) * s];
        let mut end = [sx + way[0], sy + way[1]];
        let mut blocked = [false; 2];
        let mut at = [sx, sy].map(|v| libm::floor(v / s));
        // The walker first climbs out of what fills its own cell, so only
        // what is over that counts as a ceiling.
        let mut path = Path::default();
        floor = {
            let mut c = SplitCell::new(self, center(at), LEAF_ZOOM, path);
            let (f, _) = c.lift(floor);
            path = c.path;
            c.answer(f).exact()?
        };
        // Amanatides and Woo's traversal: the share of `way` at each axis's next edge.
        loop {
            let edge = |k: usize| {
                if way[k] > 0.0 {
                    (at[k] + 1.0) * s
                } else {
                    at[k] * s
                }
            };
            let share = |k: usize| {
                if blocked[k] || way[k] == 0.0 {
                    f64::INFINITY
                } else {
                    (edge(k) - [sx, sy][k]) / way[k]
                }
            };
            let k = if share(0) <= share(1) { 0 } else { 1 };
            let t = share(k);
            if t > 1.0 {
                break;
            }
            let mut into = at;
            into[k] += way[k].signum();
            // A point short of the edge by less than a position's fraction,
            // so the cell it ends in is this one.
            let short = edge(k) - way[k].signum() * s * 1e-6;
            match self.step(center(at), floor, center(into), walled, &mut path)? {
                Some(top) => (at, floor) = (into, top),
                // Blocked: the walker slides, this axis staying in its cell
                // while the other goes on.
                None => (end[k], blocked[k]) = (short, true),
            }
        }
        Some(gather(end[0], end[1], floor))
    }

    /// One step from the leaf cell at `from` into the one at `to`, with the
    /// feet at `floor`. The walker climbs over what fills the cell it enters,
    /// as long as nothing over `floor` in the cell it leaves stops a walker a
    /// cell tall there, and then drops onto the floor.
    ///
    /// Returns the floor it ends on. The inner None means the ceiling of the
    /// cell it leaves stopped it; the outer None means `to` or the cell it
    /// leaves is not held at the leaf. The ceiling is asked only as high as
    /// the climb, never past it. The columns' rays step their tile lines on
    /// from `path`, the walk's last, and leave theirs there.
    fn step(
        &self,
        from: [f64; 2],
        floor: f64,
        to: [f64; 2],
        walled: bool,
        path: &mut Path,
    ) -> Option<Option<f64>> {
        if walled && self.leaves_area(from, to) {
            return Some(None);
        }
        let cell = units(GROUND.leaf_cell_zoom());
        let mut c = SplitCell::new(self, to, LEAF_ZOOM, *path);
        let (z, cell_top) = c.lift(floor);
        let moved = if z > floor {
            let mut from_cell = SplitCell::new(self, from, LEAF_ZOOM, c.path);
            let stopped = from_cell.above(floor, z + cell).is_some();
            (*path, c.path) = (from_cell.path, from_cell.path);
            from_cell.answer(()).exact()?;
            (!stopped).then_some(z)
        } else {
            Some(c.below(z, cell_top).unwrap_or(z))
        };
        *path = c.path;
        c.answer(moved).exact()
    }
}
