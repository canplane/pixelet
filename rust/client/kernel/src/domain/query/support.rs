// pxl-kernel/domain/query/support.rs

//! Where something stands: the support, floor and ceiling over a point,
//! climbing as a walker would. Each is asked of the point's cell as its
//! columns, a ray straight up or down through each (`CellColumns`), the ray
//! every reader casts (`World::along`): the first cell that fills, or out of
//! what fills the first that does not, in any of them. At the leaf the cell
//! is cut to the finest zoom anything is placed at in it, the world read as
//! finely as it is held (query/ray.rs `finest`), so a walker, a cell wide,
//! stands on what is placed whatever its size; a coarser zoom's cell is one
//! column, read as that zoom's cells are (`at_zoom`), what is placed finer
//! folded into it. Either way a cell kept short of its cube stands to the
//! top of the box it is drawn as (pxl-engine `cast::top`).

use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::world::{BOUNDS, ROUND};
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use xpute_kit::math::scalar::wrap;

use pxl_spec::space::coord::aabb::Aabb;

use super::ray::{at_zoom, cell_zoom_at, finest, Ray, Until};
use super::world::World;
use crate::domain::layer::tile_cache::Lines;
use crate::domain::layer::*;
use pxl_engine::cast::{fill_bounds, Detail};
use pxl_spec::space::layer::tile::Tile;

/// The center's height over a floor whose top is `top`: half a cell up, as a
/// cell's center stands over its ground.
pub fn over(top: f64) -> f64 {
    top + units(GROUND.cell()) / 2.0
}

/// The floor under a center at `z`: what `over` lifts it from.
pub fn under(z: f64) -> f64 {
    z - units(GROUND.cell()) / 2.0
}

/// Whether a walker at `at`, seen from `eye`, walks: the camera grid draws
/// the ground there at its own cells (`cell_zoom_at`), X the short way round.
/// Else the world is a map to it (`step_slide`).
pub fn walks(at: &Q3, eye: &Q3) -> bool {
    let (a, e) = (q3::unpack(at), q3::unpack(eye));
    cell_zoom_at(e, [world::nearest(a[0], e[0]), a[1], a[2]]) == GROUND.cell()
}

/// A point's place on the ground, in units.
fn xy(at: &Q3) -> [f64; 2] {
    [q3::to_real(i64::from(at[0])), q3::to_real(i64::from(at[1]))]
}

/// The cell over `xy` at the tile zoom `zoom`'s cells as its columns: at the
/// leaf, `across` a side at the finest zoom anything is placed at in it, a
/// coarser zoom's the one column (module doc). What a walker meets straight
/// up and down each, a ray through its middle; and the first tile an answer
/// lacked. Its rays' tiles' lines each stepped from the last ray's
/// (`lines`), handed on from the column it was made beside, as they climb
/// the same tiles over the same tile cache.
struct CellColumns<'w, 'a> {
    world: &'w World<'a>,
    corner: [f64; 2],
    side: f64,
    across: u32,
    detail: Detail,
    lack: Option<Tile>,
    zoom: Zoom,
    lines: Lines,
}

impl<'w, 'a> CellColumns<'w, 'a> {
    fn new(world: &'w World<'a>, xy: [f64; 2], zoom: Zoom, lines: Lines) -> CellColumns<'w, 'a> {
        let cell = GROUND.cell_zoom(zoom);
        let side = units(cell);
        let corner = xy.map(|v| libm::floor(v / side) * side);
        let column = Aabb {
            min: [corner[0], corner[1], BOUNDS.min[2]],
            max: [corner[0] + side, corner[1] + side, BOUNDS.max[2]],
        };
        let read = if zoom == LEAF_ZOOM {
            finest()
        } else {
            at_zoom(zoom)
        };
        let finer = world
            .finest_in(&column)
            .map_or(0, |z| z.min(read.range[1]).saturating_sub(cell));
        CellColumns {
            world,
            corner,
            side: side / f64::from(1u32 << finer),
            across: 1 << finer,
            detail: at_zoom(zoom + finer),
            lack: None,
            zoom,
            lines,
        }
    }

    /// The box of the first cell each column's ray from the height `z` to
    /// the height `to`, up or down, stops at (`until`).
    fn rays(
        &mut self,
        z: f64,
        to: f64,
        until: Until,
    ) -> impl Iterator<Item = Aabb> + use<'_, 'w, 'a> {
        (0..self.across * self.across)
            .filter_map(move |k| self.meet(k, z, to, until).val().flatten())
    }

    /// Column `k`'s ray, with what the world held of the way: a ray that
    /// meets nothing before a lack tells nothing past it.
    fn meet(&mut self, k: u32, z: f64, to: f64, until: Until) -> Answer<Option<Aabb>> {
        let [x, y] = self.middle(k);
        let ray = Ray {
            origin: [x, y, z],
            dir: [0.0, 0.0, if to > z { 1.0 } else { -1.0 }],
            t_max: (to - z).abs(),
        };
        let (answer, _) = self
            .world
            .along(&ray, self.detail, until, None, &mut self.lines);
        if let Some(l) = answer.lack() {
            self.lack.get_or_insert(l);
        }
        answer.map(|h| h.map(|h| h.cube))
    }

    /// Column `k`'s middle on the ground's two axes.
    fn middle(&self, k: u32) -> [f64; 2] {
        let at = |i: u32, c: f64| c + (f64::from(i) + 0.5) * self.side;
        [
            at(k % self.across, self.corner[0]),
            at(k / self.across, self.corner[1]),
        ]
    }

    /// The top of column `k`'s cell under the height `open`, at the zoom its
    /// rays read, as it is drawn (pxl-engine `cast::top`): its cube's top
    /// where it keeps its cube whole, else its fill box's; `open` where what
    /// it holds is not known.
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
            let (_, hi) = fill_bounds(([0.0; 3], [1.0; 3]), asset);
            (f64::from(cell[2]) + f64::from(hi[2])) * side
        })
    }

    /// The top of what fills at the height `z` in any column: out of each
    /// one's fill in turn, until none fills where the walker stands. A
    /// column's fill tops where its first cell that does not fill from `z`
    /// up begins, at the drawn top of the cell under it (`top_under`), `z`
    /// where its own does not fill; filled to the reach's top only where the
    /// world holds it so: a way into tiles not held fills nothing, as what a
    /// zoom does not hold is absent there.
    fn lift(&mut self, mut z: f64) -> f64 {
        loop {
            let top = (0..self.across * self.across)
                .map(
                    |k| match self.meet(k, z, BOUNDS.max[2], Until::Transparent) {
                        Answer::Exact(None) => BOUNDS.max[2],
                        Answer::Exact(Some(b)) | Answer::Coarser(Some(b), _) if b.min[2] > z => {
                            self.top_under(k, b.min[2]).max(z)
                        }
                        _ => z,
                    },
                )
                .fold(z, f64::max);
            if top == z {
                return z;
            }
            z = top;
        }
    }

    /// The top of the highest solid under `z` in any column, over the
    /// reach's floor: the rays down from `z`, which start in the cell over
    /// it.
    fn below(&mut self, z: f64) -> Option<f64> {
        self.rays(z, BOUNDS.min[2], Until::Opaque)
            .map(|b| b.max[2])
            .filter(|&t| t > BOUNDS.min[2])
            .reduce(f64::max)
    }

    /// The bottom of the lowest solid over `z` and under `to` in any column:
    /// the rays up from `z`, which start in the cell at `z` and so meet
    /// none at it.
    fn above(&mut self, z: f64, to: f64) -> Option<f64> {
        self.rays(z, to, Until::Opaque)
            .map(|b| b.min[2])
            .filter(|&b| b < to)
            .reduce(f64::min)
    }

    /// The top of the highest solid in any column, within the reach: each
    /// column's surface at its zoom, the one support every reader asks
    /// (`World::column_surface`, pxl-engine `support::column_surface`).
    fn highest(&mut self) -> Option<f64> {
        let cell = self.detail.range[0];
        let unit = units(cell);
        let mut most: Option<f64> = None;
        for k in 0..self.across * self.across {
            let middle = |i: u32, c: f64| c + (f64::from(i) + 0.5) * self.side;
            let g = [
                middle(k % self.across, self.corner[0]),
                middle(k / self.across, self.corner[1]),
            ]
            .map(|v| libm::floor(v / unit) as i32);
            let (answer, _) = self.world.column_surface(cell, g);
            if let Some(l) = answer.lack() {
                self.lack.get_or_insert(l);
            }
            if let Some(Some((at, rise))) = answer.val() {
                let top = ((f64::from(at) + f64::from(rise)) * unit).min(BOUNDS.max[2]);
                most = Some(most.map_or(top, |m| m.max(top)));
            }
        }
        most
    }

    /// The cell's ground's top, in units.
    fn surface(&mut self) -> Option<f64> {
        let cell = GROUND.cell_zoom(self.zoom);
        let side = units(cell);
        let answer = self.world.surface_of(
            cell,
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

impl<'a> World<'a> {
    /// What is in the leaf cell at `at`, as the cast draws it: its value
    /// (pxl-engine `world::node`'s), void where none.
    pub fn asset_of(&self, at: &Q3) -> Answer<u32> {
        let side = units(GROUND.cell());
        let p = [i64::from(at[0]), i64::from(at[1]), i64::from(at[2])].map(q3::to_real);
        self.asset((GROUND.cell(), p.map(|v| (v / side).floor() as i32)))
    }

    /// Where a walker stands from the drawn surface, ignoring `at`'s height
    /// (a fix, a search result): up out of what fills it, else down onto
    /// the first solid under it; none where nothing stands, a cell with no
    /// surface and nothing placed.
    pub fn surface_support(&self, at: &Q3) -> Answer<Option<f64>> {
        let mut c = CellColumns::new(self, xy(at), LEAF_ZOOM, Lines::default());
        let [lo, hi] = [BOUNDS.min[2], BOUNDS.max[2]];
        let surface = c.surface();
        // With no surface, the search starts at the highest solid, or at the reach's floor.
        let start = surface.or_else(|| c.highest()).unwrap_or(lo).max(lo);
        let stands = match c.lift(start) {
            z if z > start => Some(z.min(hi)),
            _ => c.below(start).or(surface.map(|_| start)),
        };
        c.answer(stands)
    }

    /// From the center `at` down to the first solid's top, or up out of what
    /// fills it, as the frame draws it at the tile zoom `zoom`; none where
    /// nothing stands under `at`.
    pub fn floor_under(&self, at: &Q3, zoom: Zoom) -> Answer<Option<f64>> {
        let start = q3::to_real(i64::from(at[2]));
        let mut c = CellColumns::new(self, xy(at), zoom, Lines::default());
        let stands = match c.lift(start) {
            z if z > start => Some(z.min(BOUNDS.max[2])),
            z => c.below(z),
        };
        c.answer(stands)
    }

    /// The top of the highest solid over `at` or holding it, in any column, as
    /// many as are stacked with air between; none where the sky is open over
    /// it.
    pub fn top_over(&self, at: &Q3) -> Answer<Option<f64>> {
        let start = q3::to_real(i64::from(at[2]));
        let mut c = CellColumns::new(self, xy(at), LEAF_ZOOM, Lines::default());
        let hi = BOUNDS.max[2];
        let mut top = c.lift(start);
        while let Some(bottom) = c.above(top, hi) {
            let next = c.lift(bottom);
            if next <= top {
                break;
            }
            top = next;
        }
        c.answer((top > start).then_some(top.min(hi)))
    }

    /// The floor under `at` nearest its height: the top of a solid with
    /// room over it, under `at` or over it, or the cell's surface; none
    /// where nothing stands.
    pub fn support_nearest(&self, at: &Q3) -> Answer<Option<f64>> {
        let z = q3::to_real(i64::from(at[2]));
        let mut c = CellColumns::new(self, xy(at), LEAF_ZOOM, Lines::default());
        let down = match c.lift(z) {
            t if t > z => Some(t.min(BOUNDS.max[2])),
            t => c.below(t),
        };
        let up = c.above(z, BOUNDS.max[2]).map(|a| c.lift(a));
        let nearest = [down, up, c.surface()]
            .into_iter()
            .flatten()
            .min_by(|a, b| (a - z).abs().total_cmp(&(b - z).abs()));
        c.answer(nearest)
    }

    /// Where a walker standing at the center `from` ends going toward `to`,
    /// a leaf cell at a time. Our rule after Quake III's PM_StepSlideMove
    /// (id Software, code/game/bg_slidemove.c), which steps up and slides a
    /// blocked axis along the wall as PM_ClipVelocity clips the velocity:
    /// here the step up has no height but the ceiling's, it gives up where its
    /// own cell's ceiling stops the climb, and it settles on the floor where it
    /// `falls`; with `falls` off it keeps its height wherever nothing lifts it,
    /// as the orbit's eye does. Where the
    /// camera grid draws `from` coarser than the ground's own cells, seen from
    /// `eye`, this is a map, not a walk: it goes to `to` and stands there.
    /// That is decided where it starts, by the grid's angle.
    /// Where it is `walled`, as what walks in the world is, it never leaves
    /// the service area (`within`), sliding along its edge, and one already
    /// past it is not walled until it is back in; the camera is not, and goes anywhere a position
    /// can be, past the area over the Earth, which holds nothing above sea
    /// level.
    /// It stops at the edge of a leaf on the way not held, keeping its walk up
    /// to there; none where the leaf under `from`, or a map's `to`, is not
    /// held, when its caller goes on with the height unknown. `from` is a
    /// center whose height is known.
    pub fn step_slide(
        &self,
        from: &Q3,
        to: &Q3,
        eye: &Q3,
        falls: bool,
        walled: bool,
    ) -> Option<Q3> {
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
        if !walks(from, eye) {
            let at = world::at(i64::from(to[0]), i64::from(to[1]), i64::from(from[2]));
            if walled && !self.within(&at) && self.within(from) {
                return Some(*from);
            }
            let stands = if falls {
                self.surface_support(&at).exact()?.unwrap_or(floor)
            } else {
                self.floor_under(&at, LEAF_ZOOM)
                    .exact()?
                    .filter(|&z| z > floor)
                    .unwrap_or(floor)
            };
            return Some(gather(tx, ty, stands));
        }
        // The short way round the world, across the antimeridian too.
        let way = [wrap(tx - sx, -ROUND / 2.0, ROUND / 2.0), ty - sy];
        let s = units(GROUND.cell());
        let center = |c: [f64; 2]| [(c[0] + 0.5) * s, (c[1] + 0.5) * s];
        let mut end = [sx + way[0], sy + way[1]];
        let mut blocked = [false; 2];
        // Where the walker is at the share `t` of the way, a slid axis held where it stopped.
        let point = |t: f64, end: [f64; 2], blocked: [bool; 2]| {
            [0, 1].map(|k| {
                if blocked[k] {
                    end[k]
                } else {
                    [sx, sy][k] + way[k] * t
                }
            })
        };
        let mut at = [sx, sy].map(|v| libm::floor(v / s));
        // Out of what fills its own cell first: only what is over that is a ceiling.
        let mut lines = Lines::default();
        floor = {
            let mut c = CellColumns::new(self, center(at), LEAF_ZOOM, lines);
            let f = c.lift(floor);
            lines = c.lines;
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
            // Short of the edge by less than a position's fraction, so the cell it ends in is this one.
            let short = edge(k) - way[k].signum() * s * 1e-6;
            match self.step(center(at), floor, center(into), (falls, walled), &mut lines) {
                Some(Some(top)) => (at, floor) = (into, top),
                // Slid along: this axis stays in its cell, the other goes on.
                Some(None) => (end[k], blocked[k]) = (short, true),
                None => {
                    end = point(t, end, blocked);
                    end[k] = short;
                    break;
                }
            }
        }
        Some(gather(end[0], end[1], floor))
    }

    /// From the leaf cell at `from` into the one at `to`, the feet at
    /// `floor`: up over what fills the walker's cell, a cell tall, while
    /// what is over `floor` in the cell left does not stop it there, then
    /// down onto the floor if it `falls`; the floor there, none where the
    /// cell left's ceiling stops it, the outer none where `to` or the cell
    /// left is not held at the leaf. The ceiling is asked only as high as the
    /// climb, never past it. Its columns' rays step their tiles' lines from
    /// `lines`, the walk's last, and leave theirs there.
    fn step(
        &self,
        from: [f64; 2],
        floor: f64,
        to: [f64; 2],
        (falls, walled): (bool, bool),
        lines: &mut Lines,
    ) -> Option<Option<f64>> {
        // Past the service area is a wall, slid along as any, to what is
        // `walled` and within it; one already past it walks out of there.
        let within =
            |p: [f64; 2]| self.within(&world::at(q3::from_real(p[0]), q3::from_real(p[1]), 0));
        if walled && !within(to) && within(from) {
            return Some(None);
        }
        let cell = units(GROUND.cell());
        let mut c = CellColumns::new(self, to, LEAF_ZOOM, *lines);
        let mut z = c.lift(floor);
        let moved = if z > floor {
            let mut left = CellColumns::new(self, from, LEAF_ZOOM, c.lines);
            let stopped = left.above(floor, z + cell).is_some();
            (*lines, c.lines) = (left.lines, left.lines);
            left.answer(()).exact()?;
            (!stopped).then_some(z)
        } else {
            if falls {
                z = c.below(z).unwrap_or(z);
            }
            Some(z)
        };
        *lines = c.lines;
        c.answer(moved).exact()
    }
}
