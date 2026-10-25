// pxl-kernel/domain/query/support.rs

//! Where something stands: the support, floor and ceiling over a point,
//! climbing as a walker would. A walker is a cell wide, so each is asked of
//! the point's cell as its columns at the finest zoom anything is placed at
//! in it, a ray straight up or down through each (`Upright`), the ray every
//! reader casts (`World::along`): the first block that fills, or out of what
//! fills the first that does not, in any of them. So a walker stands on
//! what is placed as it is held, whatever its size; a cell with nothing
//! placed finer than it is one column, one ray.

use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::world::{REACH, ROUND};
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::layer::lattice::Cells;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use xpute_kit::math::scalar::wrap;

use pxl_spec::space::coord::aabb::Aabb;

use super::ray::{at_zoom, Ray, Until};
use super::surface::Surface;
use super::world::World;
use crate::domain::layer::*;
use pxl_spec::space::layer::tile::Tile;
use pxl_spec_core::cast::Detail;

/// The center's height over a floor whose top is `top`: half a cell up, as a
/// cell's center stands over its ground.
pub fn over(top: f64) -> f64 {
    top + units(GROUND.cell()) / 2.0
}

/// The floor under a center at `z`: what `over` lifts it from.
pub fn under(z: f64) -> f64 {
    z - units(GROUND.cell()) / 2.0
}

/// The cell zoom the eye's rays draw at `at`: the camera grid's rule at its
/// distance (`Tau::cell_at`), no finer than the ground's cells and, once the
/// source declares its top, no coarser than a tile draws.
pub fn cell_at(eye: V3, tau: Tau, at: V3) -> Zoom {
    let v = v3::sub(&at, &eye);
    let cell = tau.cell_at(v3::dot(&v, &v)).min(GROUND.cell());
    TerrainTile::top().map_or(cell, |top| cell.max(GROUND.grid_zoom(top)))
}

/// Whether a walker at `at`, seen from `eye`, walks: the camera grid at the
/// criterion's own τ draws the ground there at its own cells (`cell_at`),
/// X the short way round. Else the world is a map to it (`step_slide`).
pub fn walks(at: &Q3, eye: &Q3) -> bool {
    let (a, e) = (q3::unpack(at), q3::unpack(eye));
    cell_at(e, Tau::FINEST, [world::nearest(a[0], e[0]), a[1], a[2]]) == GROUND.cell()
}

/// A point's place on the ground, in units.
fn xy(at: &Q3) -> [f64; 2] {
    [q3::to_real(i64::from(at[0])), q3::to_real(i64::from(at[1]))]
}

/// What a walker meets straight up and down the cell over `xy` at the tile
/// zoom `zoom`'s cells, asked of its columns at the finest zoom anything is
/// placed at in it, `across` a side, a ray through each one's middle at that
/// zoom; and the first tile an answer lacked.
struct Upright<'w, 'a> {
    world: &'w World<'a>,
    corner: [f64; 2],
    side: f64,
    across: u32,
    detail: Detail,
    lack: Option<Tile>,
    zoom: Zoom,
}

impl<'w, 'a> Upright<'w, 'a> {
    fn new(world: &'w World<'a>, xy: [f64; 2], zoom: Zoom) -> Upright<'w, 'a> {
        let cell = GROUND.cell_zoom(zoom);
        let side = units(cell);
        let corner = xy.map(|v| libm::floor(v / side) * side);
        let column = Aabb {
            min: [corner[0], corner[1], REACH.min[2]],
            max: [corner[0] + side, corner[1] + side, REACH.max[2]],
        };
        let finer = world
            .finest_in(&column)
            .map_or(0, |z| z.saturating_sub(cell));
        Upright {
            world,
            corner,
            side: side / f64::from(1u32 << finer),
            across: 1 << finer,
            detail: at_zoom(zoom + finer),
            lack: None,
            zoom,
        }
    }

    /// The box of the first block each column's ray from the height `z` to
    /// the height `to`, up or down, stops at (`until`).
    fn rays(
        &mut self,
        z: f64,
        to: f64,
        until: Until,
    ) -> impl Iterator<Item = Aabb> + use<'_, 'w, 'a> {
        (0..self.across * self.across)
            .filter_map(move |k| self.met(k, z, to, until).value().flatten())
    }

    /// Column `k`'s ray, with what the world held of the way: a ray that
    /// meets nothing before a lack tells nothing past it.
    fn met(&mut self, k: u32, z: f64, to: f64, until: Until) -> Answer<Option<Aabb>> {
        let middle = |i: u32, c: f64| c + (f64::from(i) + 0.5) * self.side;
        let ray = Ray {
            origin: [
                middle(k % self.across, self.corner[0]),
                middle(k / self.across, self.corner[1]),
                z,
            ],
            direction: [0.0, 0.0, if to > z { 1.0 } else { -1.0 }],
            reach: (to - z).abs(),
        };
        let (answer, _) = self.world.along(&ray, self.detail, until, None);
        if let Some(l) = answer.lack() {
            self.lack.get_or_insert(l);
        }
        answer.map(|h| h.map(|h| h.cube))
    }

    /// The top of what fills at the height `z` in any column: out of each
    /// one's fill in turn, until none fills where the walker stands. A
    /// column's fill tops at the bottom of its first block that does not
    /// fill from `z` up, `z` where its own does not; filled to the reach's
    /// top only where the world holds it so: a way into tiles not held fills
    /// nothing, as what a zoom does not hold is absent there.
    fn lift(&mut self, mut z: f64) -> f64 {
        loop {
            let top = (0..self.across * self.across)
                .map(|k| match self.met(k, z, REACH.max[2], Until::Air) {
                    Answer::Exact(None) => REACH.max[2],
                    Answer::Exact(Some(b)) | Answer::Coarser(Some(b), _) => b.min[2].max(z),
                    Answer::Coarser(None, _) | Answer::Nothing(_) => z,
                })
                .fold(z, f64::max);
            if top == z {
                return z;
            }
            z = top;
        }
    }

    /// The top of the highest solid under `z` in any column, over the
    /// reach's floor: the rays down from `z`, which start in the block over
    /// it.
    fn below(&mut self, z: f64) -> Option<f64> {
        self.rays(z, REACH.min[2], Until::Fills)
            .map(|b| b.max[2])
            .filter(|&t| t > REACH.min[2])
            .reduce(f64::max)
    }

    /// The bottom of the lowest solid over `z` and under `to` in any column:
    /// the rays up from `z`, which start in the block at `z` and so meet
    /// none at it.
    fn above(&mut self, z: f64, to: f64) -> Option<f64> {
        self.rays(z, to, Until::Fills)
            .map(|b| b.min[2])
            .filter(|&b| b < to)
            .reduce(f64::min)
    }

    /// The top of the highest solid in any column, within the reach.
    fn highest(&mut self) -> Option<f64> {
        self.rays(REACH.max[2], REACH.min[2], Until::Fills)
            .map(|b| b.max[2].min(REACH.max[2]))
            .reduce(f64::max)
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
        answer.value().flatten().map(|s| s.top)
    }

    /// `v`, with the first tile its answers lacked.
    fn done<T>(&self, v: T) -> Answer<T> {
        match self.lack {
            None => Answer::Exact(v),
            Some(l) => Answer::Coarser(v, l),
        }
    }
}

impl<'a> World<'a> {
    /// The ground's surface under `at`, as the frame draws it at the tile
    /// zoom `zoom`; none where the cell has no ground.
    pub fn surface(&self, at: &Q3, zoom: Zoom) -> Answer<Option<Surface>> {
        let (cell, c) = GROUND.cell_of(zoom, at).node();
        self.surface_of(cell, c)
    }

    /// What is in the leaf block at `at`, as the cast draws it: its value
    /// (pxl-spec-core `world::node`'s), void where none.
    pub fn block_of(&self, at: &Q3) -> Answer<u32> {
        let side = units(GROUND.cell());
        let p = [i64::from(at[0]), i64::from(at[1]), i64::from(at[2])].map(q3::to_real);
        self.block((GROUND.cell(), p.map(|v| (v / side).floor() as i32)))
    }

    /// Where a walker stands from the drawn surface, ignoring `at`'s height
    /// (a fix, a search result): up out of what fills it, else down onto
    /// the first solid under it; none where nothing stands, a cell with no
    /// surface and nothing placed.
    pub fn surface_support(&self, at: &Q3) -> Answer<Option<f64>> {
        let mut c = Upright::new(self, xy(at), LEAF_ZOOM);
        let [lo, hi] = [REACH.min[2], REACH.max[2]];
        let surface = c.surface();
        // With no surface, the search starts at the highest solid, or at the reach's floor.
        let start = surface.or_else(|| c.highest()).unwrap_or(lo).max(lo);
        let stands = match c.lift(start) {
            z if z > start => Some(z.min(hi)),
            _ => c.below(start).or(surface.map(|_| start)),
        };
        c.done(stands)
    }

    /// From the center `at` down to the first solid's top, or up out of what
    /// fills it, as the frame draws it at the tile zoom `zoom`; none where
    /// nothing stands under `at`.
    pub fn floor_under(&self, at: &Q3, zoom: Zoom) -> Answer<Option<f64>> {
        let start = q3::to_real(i64::from(at[2]));
        let mut c = Upright::new(self, xy(at), zoom);
        let stands = match c.lift(start) {
            z if z > start => Some(z.min(REACH.max[2])),
            z => c.below(z),
        };
        c.done(stands)
    }

    /// The floor under `at` nearest its height: the top of a solid with
    /// room over it, under `at` or over it, or the cell's surface; none
    /// where nothing stands.
    pub fn support_nearest(&self, at: &Q3) -> Answer<Option<f64>> {
        let z = q3::to_real(i64::from(at[2]));
        let mut c = Upright::new(self, xy(at), LEAF_ZOOM);
        let down = match c.lift(z) {
            t if t > z => Some(t.min(REACH.max[2])),
            t => c.below(t),
        };
        let up = c.above(z, REACH.max[2]).map(|a| c.lift(a));
        let nearest = [down, up, c.surface()]
            .into_iter()
            .flatten()
            .min_by(|a, b| (a - z).abs().total_cmp(&(b - z).abs()));
        c.done(nearest)
    }

    /// Where a walker standing at the center `from` ends going toward `to`,
    /// a leaf cell at a time. Our rule after Quake III's PM_StepSlideMove
    /// (id Software, code/game/bg_slidemove.c), which steps up and slides a
    /// blocked axis along the wall as PM_ClipVelocity clips the velocity:
    /// here the step up has no height but the ceiling's, since nothing jumps,
    /// it gives up where its own cell's ceiling stops the climb, and it
    /// settles on the floor where it `falls`; with `falls` off it keeps its
    /// height wherever nothing lifts it, as the orbit's eye does. Where the
    /// camera grid at the criterion's own τ draws `from` coarser than the
    /// ground's own cells, seen from `eye`, this is a map, not a walk: it goes
    /// to `to` and stands there. That is decided where it starts, and by the
    /// criterion's τ so that the quality a device draws at moves no wall.
    /// It never leaves the service area (`within`), sliding along its edge.
    /// It stops at the edge of a leaf on the way not held, keeping its walk up
    /// to there; none where the leaf under `from`, or a map's `to`, is not
    /// held, when its caller goes on with the height unknown. `from` is a
    /// center whose height is known.
    pub fn step_slide(&self, from: &Q3, to: &Q3, eye: &Q3, falls: bool) -> Option<Q3> {
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
            if !self.within(&at) {
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
        floor = {
            let mut c = Upright::new(self, center(at), LEAF_ZOOM);
            let f = c.lift(floor);
            c.done(f).exact()?
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
            match self.step(center(at), floor, center(into), falls) {
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
    /// climb, never past it.
    fn step(&self, from: [f64; 2], floor: f64, to: [f64; 2], falls: bool) -> Option<Option<f64>> {
        // Past the service area is a wall, slid along as any.
        if !self.within(&world::at(q3::from_real(to[0]), q3::from_real(to[1]), 0)) {
            return Some(None);
        }
        let cell = units(GROUND.cell());
        let mut c = Upright::new(self, to, LEAF_ZOOM);
        let mut z = c.lift(floor);
        let moved = if z > floor {
            let mut left = Upright::new(self, from, LEAF_ZOOM);
            let stopped = left.above(floor, z + cell).is_some();
            left.done(()).exact()?;
            (!stopped).then_some(z)
        } else {
            if falls {
                z = c.below(z).unwrap_or(z);
            }
            Some(z)
        };
        c.done(moved).exact()
    }
}
