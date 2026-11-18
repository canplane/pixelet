// pxl-core/domain/query/support.rs

//! Where something stands: the support, the floor and the ceiling over a
//! point, found by climbing as a body would. Each is asked of the point's
//! cell, split into columns, with a ray straight up or down through each
//! column (`SplitCell`). That ray is the one every reader casts
//! (`World::along`). In any column it finds the first cell that fills or,
//! starting inside what fills, the first cell that does not.
//!
//! The cell is the one the eye draws the ground with where the point is,
//! split as finely as the eye reads there and anything is placed in it, so a
//! body near the eye stands on what is placed whatever its size, and far
//! from it on what is folded into the cell. A cell coarser than the leaves'
//! gives a coarser answer. A column's ray reads the world by
//! its distance from the eye, as every ray does: what it meets far from the
//! eye is the coarser cell the eye sees there. A cell filled short of its cube
//! stands to the top of the box it is drawn as (pxl-engine `cast::top`).

use pxl_spec::space::camera::grid::cell_zoom_at;
use pxl_spec::space::coord::q3::{self, FRAC_W, Q3};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::world::{BOUND, ROUND};
use pxl_spec::space::coord::zoom::{LEAF_ZOOM, Zoom, units};
use pxl_spec::space::layer::lattice::GROUND;
use xpute_kit::math::scalar::wrap;

use pxl_spec::space::coord::aabb::Aabb;

use super::detail::{cell_zoom_drawn, cell_zoom_read, leaf_at};
use super::ray::{Descent, Ray, Until};
use super::world::World;
use crate::domain::layer::tile_cache::Path;
use crate::domain::layer::*;
use crate::domain::observer;
use pxl_content::scatter::box_shows_at;
use pxl_spec::asset::character::BODY_BOX;
use pxl_spec::space::layer::quadrant::Quadrant;

/// Whether a body shows `dist` units from the eye. A body claims its box
/// (pxl-spec `BODY_BOX`) and shows as a decor of that box placed there is
/// drawn (pxl-content `generation::drawn_at`, `scatter::box_shows_at`):
/// where the eye's rays ask the leaves' zoom, or a coarser one whose cell a
/// cube of its box keeps an eighth of. The zoom asked is the camera grid's at
/// that distance, whatever zooms the source holds. A character is drawn, and
/// a walk is walked, only where this holds.
pub(crate) fn body_shown(dist: f64) -> bool {
    box_shows_at(
        BODY_BOX,
        GROUND.leaf_cell_zoom(),
        cell_zoom_at((dist * dist) as f32),
    )
}

/// How high a body's box stands over its floor, in units.
pub(crate) fn body_height_u() -> f64 {
    f64::from(BODY_BOX[2]) * units(GROUND.leaf_cell_zoom())
}

/// Where a body's eye stands over a floor whose top is `top`: half a cell
/// under the top of its box, as it stands half a cell over the bottom
/// (`over`).
pub(crate) fn eye_over(top: f64) -> f64 {
    top + body_height_u() - units(GROUND.leaf_cell_zoom()) / 2.0
}

/// Where a body stands over a floor whose top is `top`: the center of the
/// leaf cell its feet are in, half a cell up, as a cell's center stands over
/// its ground.
pub(crate) fn over(top: f64) -> f64 {
    top + units(GROUND.leaf_cell_zoom()) / 2.0
}

/// The room a body standing at `at` claims: its box over the leaf cell its
/// feet are in. Nothing placed enters it.
pub(crate) fn body_room(at: &Q3) -> Aabb {
    let s = units(GROUND.leaf_cell_zoom());
    let [x, y] = [0, 1].map(|k| q3::to_real(i64::from(at[k])));
    let min = [libm::floor(x / s) * s, libm::floor(y / s) * s, floor_of(at)];
    Aabb {
        min,
        max: core::array::from_fn(|k| min[k] + f64::from(BODY_BOX[k]) * s),
    }
}

/// The floor under a body standing at `z`, the inverse of `over`.
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

/// The cell over `at` among the cells the eye draws the ground with there,
/// split into columns as the module doc says: at the leaf, `across` columns a side at
/// the finest zoom anything is placed at in it; at a coarser zoom, one
/// column. A body meets what a ray straight up or down through a column's
/// middle meets. The columns keep the first tile an answer lacked. Each
/// ray's tile lines step on from the last ray's (`path`), which were handed
/// on from the columns these were made beside, since the rays climb the same
/// tiles over the same tile cache.
struct SplitCell<'w> {
    world: &'w World,
    corner: [f64; 2],
    side: f64,
    across: u32,
    /// The eye the cell's rays read by, none for an eye standing in the
    /// column (`over`).
    eye: Option<V3>,
    lack: Option<Quadrant>,
    /// The zoom of the tiles the cell is of.
    zoom: Zoom,
    path: Path,
    /// Where the last column's ray ended. A ray from the same point goes on
    /// from there, and `lack` holds what the descent to there lacked.
    descent: Option<Descent>,
}

impl<'w> SplitCell<'w> {
    /// The cell the observer's eye draws at `at`, a point whose height is
    /// known: a body takes its detail from its own distance from the eye, on
    /// a deck as on the ground under it. With no observer it is the cell an
    /// eye standing at `at` draws.
    fn new(world: &'w World, at: V3, path: Path) -> SplitCell<'w> {
        SplitCell::at(world, observer::eye(), at, path)
    }

    /// The cell drawn over `xy` where the height is not known, with the top
    /// of its ground: the cell `eye`
    /// draws the ground with at the ground's own surface there. With no eye
    /// it is the cell an eye standing in the column draws, the leaves': a
    /// place the camera is sent to is read as it will be seen from there,
    /// and before the camera stands anywhere that is the one reading there
    /// is. It starts from the cell drawn at the column's point nearest the
    /// eye, the finest the column can be drawn with, and takes the cell
    /// drawn at each cell's own surface in turn while that one is coarser.
    fn over(
        world: &'w World,
        eye: Option<V3>,
        xy: [f64; 2],
        path: Path,
    ) -> (SplitCell<'w>, Option<f64>) {
        let cell = |z: f64, path: Path| SplitCell::at(world, eye, [xy[0], xy[1], z], path);
        let mut c = cell(eye.map_or(0.0, |e| e[2]), path);
        loop {
            let Some(top) = c.surface() else {
                return (c, None);
            };
            let at = cell(top, c.path);
            if at.zoom >= c.zoom {
                return (c, Some(top));
            }
            c = SplitCell { lack: c.lack, ..at };
        }
    }

    /// The cell the eye `by` draws the ground with at `at`, split as finely as
    /// the eye reads there and anything is placed in it; with no eye, the cell
    /// an eye at `at` itself draws.
    fn at(world: &'w World, by: Option<V3>, at: V3, path: Path) -> SplitCell<'w> {
        let eye = by.unwrap_or(at);
        let zoom = GROUND.read_zoom(cell_zoom_drawn(eye, at));
        let xy = [at[0], at[1]];
        let cell_zoom = GROUND.cell_zoom(zoom);
        let side = units(cell_zoom);
        let corner = xy.map(|v| libm::floor(v / side) * side);
        let column = Aabb {
            min: [corner[0], corner[1], BOUND.min[2]],
            max: [corner[0] + side, corner[1] + side, BOUND.max[2]],
        };
        let read = cell_zoom_read(eye, at).max(cell_zoom);
        let finer = world
            .finest_in(&column)
            .map_or(0, |z| z.min(read).saturating_sub(cell_zoom));
        SplitCell {
            world,
            corner,
            side: side / f64::from(1u32 << finer),
            across: 1 << finer,
            eye: by,
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

    /// What column `k`'s ray meets, with what the world held along the way,
    /// read by its distance from the eye. A ray that meets nothing before a
    /// lack tells nothing past it.
    fn meet(&mut self, k: u32, z: f64, to: f64, until: Until) -> Answer<Option<Aabb>> {
        let [x, y] = self.middle(k);
        let ray = Ray {
            origin: [x, y, z],
            dir: [0.0, 0.0, if to > z { 1.0 } else { -1.0 }],
            t_max: (to - z).abs(),
        };
        let answer = self.world.along_by(
            self.eye,
            &ray,
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

    /// The drawn top of what column `k` holds under its open cell `open`
    /// (pxl-engine `cast::top`): what the column's ray down from the middle
    /// of `open` meets, the cube's top where that cell fills its cube whole,
    /// else the top of its fill box. None where it meets nothing: a cell
    /// filled short of its cube is drawn as a box inset on every side
    /// (pxl-engine `cast::fill_bound`), and a column beside that box has
    /// nothing of the cell under it.
    fn top_under(&mut self, k: u32, open: &Aabb) -> Option<f64> {
        let from = (open.min[2] + open.max[2]) / 2.0;
        let met = self.meet(k, from, BOUND.min[2], Until::Opaque);
        Some(met.val().flatten()?.max[2])
    }

    /// The height a body at `z` rises to out of what fills there in any
    /// column. It climbs out of each column's fill in turn, until no column
    /// fills where the body stands. A column's fill ends where its first
    /// open cell from `z` up begins, at the drawn top of the cell under that
    /// one (`top_under`); a column whose cell at `z` does not fill leaves `z`
    /// as it is. A column whose ray went its reach and met no open cell is
    /// filled to the top of the world's bound, over ground a coarser tile
    /// stands in for as over ground held; a ray that ended at tiles not held
    /// fills nothing, since what a zoom does not hold is absent there.
    ///
    /// With it, where the body did not rise: the highest drawn top at or
    /// under `z` of a cell the body stands within, a cell filled short of
    /// its cube. A ray down from `z` to the first solid never stops in the
    /// cell it starts in (pxl-engine `cast::walk`), so `below` takes that
    /// top from here.
    fn depenetrate(&mut self, mut z: f64) -> (f64, Option<f64>) {
        loop {
            let (mut top, mut cell_top) = (z, None::<f64>);
            for k in 0..self.across * self.across {
                match self.meet(k, z, BOUND.max[2], Until::Transparent) {
                    Answer::Exact(None) | Answer::Coarser(None, _) => top = BOUND.max[2],
                    Answer::Exact(Some(b)) | Answer::Coarser(Some(b), _) if b.min[2] > z => {
                        match self.top_under(k, &b) {
                            Some(t) if t > z => top = top.max(t),
                            Some(t) => cell_top = Some(cell_top.map_or(t, |o| o.max(t))),
                            None => {}
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
    /// bottom of the world's bound: `cell_top`, the top `depenetrate` found within
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
        let cell_zoom = GROUND.cell_zoom(self.zoom) + self.across.ilog2();
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

    /// `v` where the world held all that was asked of it, whatever cells the
    /// eye draws the ground with there: a walk goes over what is drawn.
    fn without_lack<T>(&self, v: T) -> Option<T> {
        self.lack.is_none().then_some(v)
    }

    /// `v`, with the first tile its answers lacked: one the world did not
    /// hold, or the leaf over the cell where the eye draws the ground with
    /// coarser cells than the leaves', which is a coarser answer too.
    fn answer<T>(&self, v: T) -> Answer<T> {
        let [x, y] = self.corner;
        let lack = self
            .lack
            .or_else(|| (self.zoom < LEAF_ZOOM).then(|| leaf_at(&world::point(x, y, 0.0))));
        match lack {
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

    /// Where a body stands when it is put on the drawn surface at `at`
    /// whatever its height, as a fix or a search result is. It climbs out of
    /// what fills the surface, or else drops onto the first solid under it.
    /// None where nothing stands: a cell with no surface and nothing placed.
    pub(crate) fn surface_support(&self, at: &Q3) -> Answer<Option<f64>> {
        self.surface_support_by(observer::eye(), at)
    }

    /// `surface_support` as `eye` draws the world there, or, with none, as an
    /// eye standing in the column does, each ray read from where it starts
    /// (`SplitCell::over`).
    pub(crate) fn surface_support_by(&self, eye: Option<V3>, at: &Q3) -> Answer<Option<f64>> {
        let (mut c, surface) = SplitCell::over(self, eye, xy(at), Path::default());
        let [lo, hi] = [BOUND.min[2], BOUND.max[2]];
        // With no surface, the search starts at the highest solid, or at the
        // bottom of the world's bound.
        let start = surface.or_else(|| c.highest()).unwrap_or(lo).max(lo);
        let stands = match c.depenetrate(start) {
            (z, _) if z > start => Some(z.min(hi)),
            (_, cell_top) => c.below(start, cell_top).or(surface.map(|_| start)),
        };
        c.answer(stands)
    }

    /// The floor a center at `at` stands on, as the eye draws the world
    /// there: up out of what fills `at`, or else down to the first solid's
    /// top. None where nothing stands under `at`.
    pub(crate) fn floor_under(&self, at: &Q3) -> Answer<Option<f64>> {
        let start = q3::to_real(i64::from(at[2]));
        let [x, y] = xy(at);
        let mut c = SplitCell::new(self, [x, y, start], Path::default());
        let stands = match c.depenetrate(start) {
            (z, _) if z > start => Some(z.min(BOUND.max[2])),
            (z, cell_top) => c.below(z, cell_top),
        };
        c.answer(stands)
    }

    /// The floor nearest `at`'s height: the top of a solid with room over
    /// it, under `at` or over it, or the cell's surface. None where nothing
    /// stands. It is asked of a place the camera is sent to with a
    /// height, and read as an eye standing in its column reads, each ray from
    /// where it starts (`SplitCell::over`).
    pub(crate) fn support_nearest(&self, at: &Q3) -> Answer<Option<f64>> {
        let z = q3::to_real(i64::from(at[2]));
        let mut c = SplitCell::over(self, None, xy(at), Path::default()).0;
        let down = match c.depenetrate(z) {
            (t, _) if t > z => Some(t.min(BOUND.max[2])),
            (t, cell_top) => c.below(t, cell_top),
        };
        let up = c.above(z, BOUND.max[2]).map(|a| c.depenetrate(a).0);
        let nearest = [down, up, c.surface()]
            .into_iter()
            .flatten()
            .min_by(|a, b| (a - z).abs().total_cmp(&(b - z).abs()));
        c.answer(nearest)
    }

    /// Where a body standing at the center `from` ends when it goes toward
    /// `to`, one leaf cell at a time. A `from` whose height is not known
    /// stands on the surface.
    ///
    /// The rule follows Quake III's PM_StepSlideMove (id Software,
    /// code/game/bg_slidemove.c), which steps up and slides a blocked axis
    /// along the wall as PM_ClipVelocity clips the velocity. Here the step up
    /// has no height limit but the ceiling's: a step goes only where the
    /// body's box stands clear (`step`), and the body settles on the
    /// floor.
    ///
    /// With `walled`, as for what walks in the world, the body never leaves
    /// the service area (`within`) and slides along its edge; one already
    /// past the area is not walled until it is back in. The camera is not
    /// walled and goes anywhere a position can be, past the area over the
    /// Earth, which holds nothing above sea level.
    ///
    /// The walk is walked only where a body shows at both its ends
    /// (`body_shown`) for `eye`, the eye that sees the body at `from`: the
    /// observer's for a body in the world, and for the camera's own pivot the
    /// eye the camera keeps over it. It decides whether there is a body to
    /// walk; what the walk reads it reads by the observer's eye, as every
    /// reader does. So the walk is no longer than the eye draws a body
    /// across; a `from` of unknown height is stood first, the one read such
    /// a move makes. Otherwise, and where `from` cannot be stood or what the
    /// eye asks on the way is not held, there is no body for the eye and no wall its
    /// size: it ends at `to` with its height not known, and one `walled`
    /// stays where it is rather than leave the area.
    pub(crate) fn step_slide(&self, from: &Q3, to: &Q3, eye: &Q3, walled: bool) -> Q3 {
        self.stands(from)
            .exact()
            .flatten()
            .filter(|from| body_shown(farther(eye, from, to)))
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
        // The body first climbs out of what fills its own cell, so only
        // what is over that counts as a ceiling.
        let mut path = Path::default();
        floor = {
            let [x, y] = center(at);
            let mut c = SplitCell::new(self, [x, y, floor], path);
            let (f, _) = c.depenetrate(floor);
            path = c.path;
            c.without_lack(f)?
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
                // Blocked: the body slides, this axis staying in its cell
                // while the other goes on.
                None => (end[k], blocked[k]) = (short, true),
            }
        }
        Some(gather(end[0], end[1], floor))
    }

    /// One step from the leaf cell at `from` into the one at `to`, with the
    /// feet at `floor`. The body climbs over what fills the cell it enters
    /// and drops onto the floor there, as long as its box stands clear: in the
    /// cell it enters, from the height it enters at (`body_height_u` up), and,
    /// where it climbs, in the cell it leaves from `floor` to the climb's top.
    ///
    /// Returns the floor it ends on. The inner None means a ceiling stopped
    /// it; the outer None means what the eye asks of `to` or of the cell it
    /// leaves is not held. A ceiling is asked only as high as the box, never past it. The columns' rays step their tile lines on
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
        let height = body_height_u();
        let mut c = SplitCell::new(self, [to[0], to[1], floor], *path);
        let (z, cell_top) = c.depenetrate(floor);
        let moved = if c.above(z, z + height).is_some() {
            None
        } else if z > floor {
            let mut from_cell = SplitCell::new(self, [from[0], from[1], floor], c.path);
            let stopped = from_cell.above(floor, z + height).is_some();
            (*path, c.path) = (from_cell.path, from_cell.path);
            from_cell.without_lack(())?;
            (!stopped).then_some(z)
        } else {
            Some(c.below(z, cell_top).unwrap_or(z))
        };
        *path = c.path;
        c.without_lack(moved)
    }
}
