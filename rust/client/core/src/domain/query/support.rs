// pxl-core/domain/query/support.rs

//! Where something stands: the support, the floor and the ceiling over a
//! point, found by climbing as a body would. Each is asked of the point's
//! cell, split into parts, with a ray straight up or down through each
//! part (`SplitCell`). That ray is the one every reader casts
//! (`World::along`). In any part it finds the first cell that fills or,
//! starting inside what fills, the first cell that does not.
//!
//! The cell is the one the eye draws the ground with where the point is,
//! split as finely as the eye reads there and anything is placed in it, so a
//! body near the eye stands on what is placed whatever its size, and far
//! from it on what is folded into the cell. A cell coarser than the leaves'
//! gives a coarser answer. The eye sets the cell's size, by the point's
//! distance from it. Where the camera is sent, and where its pivot stands,
//! is asked at the leaves instead (`SizeBy`). What the cell's rays ask of
//! the world they ask at that one size, wherever they go, so the answer is
//! the world's at that size. A
//! cell filled short of its cube stands to the top of the box it is drawn as
//! (pxl-engine `cast::top`).

use pxl_spec::space::coord::q3::{self, FRAC_W, Q3};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::world::{BOUND, ROUND};
use pxl_spec::space::coord::zoom::{LEAF_ZOOM, Zoom, units};
use pxl_spec::space::layer::lattice::GROUND;
use xpute_kit::math::scalar::wrap;

use pxl_spec::space::coord::aabb::Aabb;

use super::change;
use super::detail::{drawn, leaf_at, one_zoom};
use super::ray::{Descent, Ray, Until};
use super::world::World;
use crate::domain::layer::tile_cache::Path;
use crate::domain::layer::*;
use pxl_content::scatter::box_shows_at;
use pxl_spec::asset::character::BODY_BOX;
use pxl_spec::space::layer::quadrant::Quadrant;

/// The cell zoom a walk asks at `at`: the size the eye sets there
/// (`change::size`). None with no eye.
pub(crate) fn cell_zoom_read(at: V3) -> Option<Zoom> {
    let (xy, z) = ([at[0], at[1]], Some(at[2]));
    let eye_zoom = change::eye_zoom(xy, z)?;
    Some(change::size(xy, z, eye_zoom, eye_zoom))
}

/// The cell zoom the eye's rays draw the ground with at `at`: the zoom a walk
/// reads there (`cell_zoom_read`), as the ground is drawn (`drawn`). None
/// with no eye.
pub(crate) fn cell_zoom_drawn(at: V3) -> Option<Zoom> {
    cell_zoom_read(at).map(drawn)
}

/// Whether the eye draws the ground at `at` with the leaves' cells: the size
/// it sets there (`cell_zoom_read`) is the leaves' or finer, so what is seen
/// there is the ground itself, with no coarser cell standing for it. It is
/// of the eye and the point alone, whatever zooms the source holds. With no
/// eye nothing is. What is near is this: first person is eased to such a
/// place and warps to one that is not, and the pointer picks and the effects
/// reach as far as it holds (pxl-spec lattice.rs `LEAF_REACH`).
pub(crate) fn leaf_drawn(at: V3) -> bool {
    cell_zoom_read(at).is_some_and(|read| read >= GROUND.leaf_cell_zoom())
}

/// Whether a body shows at `at`. A body claims its box (pxl-spec `BODY_BOX`)
/// and shows as a decor of that box placed there is drawn (pxl-content
/// `generation::drawn_at`, `scatter::box_shows_at`): where the size the eye
/// sets there (`change::size`) is the leaves' or a coarser one whose cell a
/// cube of its box keeps an eighth of, whatever zooms the source holds. A
/// character is drawn, and a walk is walked, only where this holds; with no
/// eye nothing shows.
pub(crate) fn body_shown(at: V3) -> bool {
    cell_zoom_read(at).is_some_and(body_shown_at)
}

/// `body_shown` where the eye sets the size `cell_zoom`.
fn body_shown_at(cell_zoom: Zoom) -> bool {
    box_shows_at(BODY_BOX, GROUND.leaf_cell_zoom(), cell_zoom)
}

/// How high a body's box stands over its floor, in units.
pub(crate) fn body_height_u() -> f64 {
    f64::from(BODY_BOX[2]) * units(GROUND.leaf_cell_zoom())
}

/// Where the eye stands in first person over a floor whose top is `top`:
/// half a cell under the top of the body's box, as the body stands half a
/// cell over its bottom (`over`).
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

/// Whether a body shows at both ends of a walk from `from` to over `to`, at
/// `from`'s height (`body_shown`).
fn walk_shown(from: &Q3, to: &Q3) -> bool {
    let z = q3::to_real(i64::from(from[2]));
    [from, to].into_iter().all(|p| {
        let [x, y] = xy(p);
        body_shown([x, y, z])
    })
}

/// A point's place on the ground, in units.
fn xy(at: &Q3) -> [f64; 2] {
    [q3::to_real(i64::from(at[0])), q3::to_real(i64::from(at[1]))]
}

/// What sets the size a place is asked at. The eye sets it by the place's
/// distance from where it stands. Where the camera is sent, and where its
/// pivot stands, is asked at the leaves instead, the ground's own cells: a
/// place is made a point before the camera goes there, by what the ground is
/// and not by how any eye would draw it, so the point is the same whoever
/// looks, from wherever. That read takes nothing of the eye.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum SizeBy {
    Eye,
    Leaf,
}

/// The cell over `at` a read is of, the one the eye draws the ground with
/// there or the leaves' (`SizeBy`), split into parts as the module doc says: at the leaf, `across` parts a side at
/// the finest zoom anything is placed at in it; at a coarser zoom, one
/// part. A body meets what a ray straight up or down through a part's
/// middle meets. The parts keep the first tile an answer lacked. Each
/// ray's tile lines step on from the last ray's (`path`), which were handed
/// on from the parts these were made beside, since the rays climb the same
/// tiles over the same tile cache.
struct SplitCell<'w> {
    world: &'w World,
    corner: [f64; 2],
    side: f64,
    across: u32,
    lack: Option<Quadrant>,
    /// The zoom of the tiles the cell is of.
    zoom: Zoom,
    /// The zoom the cell is read at, the eye's or the leaves', the one size
    /// its rays ask wherever they go.
    asks: Zoom,
    path: Path,
    /// Where the last part's ray ended. A ray from the same point goes on
    /// from there, and `lack` holds what the descent to there lacked.
    descent: Option<Descent>,
}

impl<'w> SplitCell<'w> {
    /// The leaves' cell over `xy`, read at the leaves.
    fn leaf(world: &'w World, xy: [f64; 2]) -> SplitCell<'w> {
        let leaf = GROUND.leaf_cell_zoom();
        SplitCell::sized(world, xy, leaf, Path::default(), |finest| finest)
    }

    /// The cell `find` is answered by over `xy`, with what it found there.
    /// By the eye, it starts from the cell drawn at the height `z`, or with
    /// none at the point over `xy` nearest the eye, the finest the cells over
    /// `xy` can be drawn with, and settles on the cell the eye draws where
    /// what was found lies (`settled`); None with no eye, where nothing is
    /// observed. At the leaves it is the leaves' cell there.
    fn read(
        world: &'w World,
        xy: [f64; 2],
        z: Option<f64>,
        by: SizeBy,
        find: impl Fn(&mut SplitCell<'w>) -> Option<f64>,
    ) -> Option<(SplitCell<'w>, Option<f64>)> {
        match by {
            SizeBy::Eye => Some(SplitCell::of(world, xy, z, Path::default())?.settled(xy, find)),
            SizeBy::Leaf => {
                let mut c = SplitCell::leaf(world, xy);
                let found = find(&mut c);
                Some((c, found))
            }
        }
    }

    /// The cell the eye draws where what is asked for over `xy` is found, and
    /// what was found: `find` asks a cell for a height over it, and while the
    /// cell the eye draws at the height found is coarser than the cell
    /// asked, that one is asked in turn. What is found far from where the
    /// asking started is a place of its own, at the size the eye sets there.
    fn settled(
        mut self,
        xy: [f64; 2],
        find: impl Fn(&mut SplitCell<'w>) -> Option<f64>,
    ) -> (SplitCell<'w>, Option<f64>) {
        loop {
            let Some(found) = find(&mut self) else {
                return (self, None);
            };
            match SplitCell::at(self.world, [xy[0], xy[1], found], self.path) {
                Some(there) if there.asks < self.asks => {
                    self = SplitCell {
                        lack: self.lack,
                        ..there
                    }
                }
                _ => return (self, Some(found)),
            }
        }
    }

    /// The cell the eye draws the ground with at `at`, a point whose height
    /// is known (`of`): a body takes its size from its own distance from the
    /// eye, on a deck as on the ground under it. None with no eye.
    fn at(world: &'w World, at: V3, path: Path) -> Option<SplitCell<'w>> {
        SplitCell::of(world, [at[0], at[1]], Some(at[2]), path)
    }

    /// The cell over `xy` at the height `z`, or with none at the eye's own
    /// height, the point over `xy` nearest the eye: the cell the ground is
    /// drawn with at the size the eye sets there (`drawn`), split as finely
    /// as that size reads and anything is placed in it. What it asks of the
    /// world it asks at that size, no finer than the world holds over the
    /// cell: the leaves' cells, or the finest placed there (`asks`). A size
    /// finer than that is the same cells asked again, so it is no other
    /// size, and what is placed there changing is the world's change. None
    /// with no eye.
    fn of(world: &'w World, xy: [f64; 2], z: Option<f64>, path: Path) -> Option<SplitCell<'w>> {
        let eye_zoom = change::eye_zoom(xy, z)?;
        Some(SplitCell::sized(world, xy, eye_zoom, path, |finest| {
            change::size(xy, z, eye_zoom, finest)
        }))
    }

    /// The cell over `xy` where `zoom` is set, the eye's or the leaves'. `size`
    /// gives the size the cell asks where the world holds nothing finer than
    /// its argument there: the eye's, no finer than is held, or what is held.
    fn sized(
        world: &'w World,
        xy: [f64; 2],
        zoom: Zoom,
        path: Path,
        size: impl FnOnce(Zoom) -> Zoom,
    ) -> SplitCell<'w> {
        let zoom = GROUND.read_zoom(drawn(zoom));
        let cell_zoom = GROUND.cell_zoom(zoom);
        let side = units(cell_zoom);
        let corner = xy.map(|v| libm::floor(v / side) * side);
        let finest_placed = world.finest_in(&Aabb {
            min: [corner[0], corner[1], BOUND.min[2]],
            max: [corner[0] + side, corner[1] + side, BOUND.max[2]],
        });
        let finest = finest_placed.unwrap_or(GROUND.leaf_cell_zoom());
        let asks = size(finest).max(cell_zoom);
        let finer = finest_placed.map_or(0, |z| z.min(asks).saturating_sub(cell_zoom));
        SplitCell {
            world,
            corner,
            side: side / f64::from(1u32 << finer),
            across: 1 << finer,
            lack: None,
            zoom,
            asks,
            path,
            descent: None,
        }
    }

    /// The floor under the height `start`: up out of what fills there, or
    /// else down to the first solid's top. None where nothing stands.
    fn floor_under(&mut self, start: f64) -> Option<f64> {
        match self.depenetrate(start) {
            (z, _) if z > start => Some(z.min(BOUND.max[2])),
            (z, cell_top) => self.below(z, cell_top),
        }
    }

    /// Where something stands on the cell's surface, whose ground is at
    /// `surface`: the floor under that height (`floor_under`), and the
    /// ground's own height where nothing solid is met at or under it, since
    /// the ground is itself a floor. With no surface, the search starts at
    /// the highest solid, or at the bottom of the world's bound. None where
    /// nothing stands: a cell with no surface and nothing placed.
    fn surface_support(&mut self, surface: Option<f64>) -> Option<f64> {
        let lo = BOUND.min[2];
        let start = surface.or_else(|| self.highest()).unwrap_or(lo).max(lo);
        self.floor_under(start).or(surface.map(|_| start))
    }

    /// For each part, the box of the first cell that its ray from the
    /// height `z` to the height `to`, up or down, stops at (`until`).
    fn rays(&mut self, z: f64, to: f64, until: Until) -> impl Iterator<Item = Aabb> + use<'_, 'w> {
        (0..self.across * self.across)
            .filter_map(move |k| self.meet(k, z, to, until).val().flatten())
    }

    /// What cell `k`'s ray meets, with what the world held along the way,
    /// read at the cell's size (`asks`). A ray that meets nothing before a
    /// lack tells nothing past it.
    fn meet(&mut self, k: u32, z: f64, to: f64, until: Until) -> Answer<Option<Aabb>> {
        let [x, y] = self.middle(k);
        let ray = Ray {
            origin: [x, y, z],
            dir: [0.0, 0.0, if to > z { 1.0 } else { -1.0 }],
            t_max: (to - z).abs(),
        };
        let answer = self.world.along(
            &ray,
            one_zoom(self.asks),
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

    /// The drawn top of what cell `k` holds under its open cell `open`
    /// (pxl-engine `cast::top`): what the part's ray down from the middle
    /// of `open` meets, the cube's top where that cell fills its cube whole,
    /// else the top of its fill box. None where it meets nothing: a cell
    /// filled short of its cube is drawn as a box inset on every side
    /// (pxl-engine `cast::fill_bound`), and a part beside that box has
    /// nothing of the cell under it.
    fn top_under(&mut self, k: u32, open: &Aabb) -> Option<f64> {
        let from = (open.min[2] + open.max[2]) / 2.0;
        let met = self.meet(k, from, BOUND.min[2], Until::Opaque);
        Some(met.val().flatten()?.max[2])
    }

    /// The height a body at `z` rises to out of what fills there in any
    /// part. It climbs out of each part's fill in turn, until no part
    /// fills where the body stands. A part's fill ends where its first
    /// open cell from `z` up begins, at the drawn top of the cell under that
    /// one (`top_under`); a part whose cell at `z` does not fill leaves `z`
    /// as it is. A part whose ray went its reach and met no open cell is
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

    /// The top of the highest solid at or under `z` in any part, above the
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

    /// The bottom of the lowest solid over `z` and under `to` in any part.
    /// The rays go up from `z` and start in the cell at `z`, so they never
    /// meet a solid at `z` itself.
    fn above(&mut self, z: f64, to: f64) -> Option<f64> {
        self.rays(z, to, Until::Opaque)
            .map(|b| b.min[2])
            .filter(|&b| b < to)
            .reduce(f64::min)
    }

    /// The top of the highest solid in any part, within the world's
    /// bound. It is
    /// each cell's surface at its zoom, the one support every reader asks
    /// (`World::surface`, pxl-engine `support::surface`).
    fn highest(&mut self) -> Option<f64> {
        let cell_zoom = GROUND.cell_zoom(self.zoom) + self.across.ilog2();
        let unit = units(cell_zoom);
        let mut most: Option<f64> = None;
        for k in 0..self.across * self.across {
            let middle = |i: u32, c: f64| c + (f64::from(i) + 0.5) * self.side;
            let c = [
                middle(k % self.across, self.corner[0]),
                middle(k / self.across, self.corner[1]),
            ]
            .map(|v| libm::floor(v / unit) as i32);
            let (answer, _) = self.world.surface(cell_zoom, c);
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

    /// The elevation of the cell's ground, in units.
    fn ele_u(&mut self) -> Option<f64> {
        let cell_zoom = GROUND.cell_zoom(self.zoom);
        let side = units(cell_zoom);
        let answer = self.world.ground_of(
            cell_zoom,
            self.corner
                .map(|v| libm::floor((v + side / 2.0) / side) as i32),
        );
        if let Some(l) = answer.lack() {
            self.lack.get_or_insert(l);
        }
        answer.val().flatten().map(|g| g.ele_u)
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
        let finer = (self.zoom < LEAF_ZOOM).then(|| leaf_at(&world::point(x, y, 0.0)));
        match self.lack.or(finer) {
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

    /// Where the point `at` stands, as the eye draws the world there: on the
    /// surface where its height is not known (`surface_support`), and
    /// otherwise where it is.
    pub(crate) fn stands(&self, at: &Q3) -> Answer<Option<Q3>> {
        match is_unknown(at) {
            true => self
                .surface_support(at, SizeBy::Eye)
                .map(|top| top.map(|top| on_floor(at, top))),
            false => Answer::Exact(Some(*at)),
        }
    }

    /// Where something stands when it is put on the surface at `at`
    /// whatever its height, as a fix or a search result is
    /// (`surface_support_zoom`).
    pub(crate) fn surface_support(&self, at: &Q3, by: SizeBy) -> Answer<Option<f64>> {
        self.surface_support_zoom(at, by)
            .map(|s| s.map(|(top, _)| top))
    }

    /// Where something stands on the surface at `at`, whatever its height,
    /// with the cell zoom the ground is read with there. It climbs out of
    /// what fills the surface, or else drops onto the first solid under it.
    /// None where nothing stands: a cell with no surface and nothing placed.
    pub(crate) fn surface_support_zoom(&self, at: &Q3, by: SizeBy) -> Answer<Option<(f64, Zoom)>> {
        let Some((mut c, surface)) = SplitCell::read(self, xy(at), None, by, |c| c.ele_u()) else {
            return Answer::Lack(leaf_at(at));
        };
        let stands = c.surface_support(surface);
        let cell_zoom = GROUND.cell_zoom(c.zoom);
        c.answer(stands.map(|top| (top, cell_zoom)))
    }

    /// The floor a center at `at` stands on: up out of what fills `at`, or
    /// else down to the first solid's top. None where nothing stands under
    /// `at`. By the eye, a floor far under a point high in the air is asked
    /// of the cell the eye draws where it lies (`SplitCell::settled`).
    pub(crate) fn floor_under(&self, at: &Q3, by: SizeBy) -> Answer<Option<f64>> {
        let start = q3::to_real(i64::from(at[2]));
        match SplitCell::read(self, xy(at), Some(start), by, |c| c.floor_under(start)) {
            Some((c, stands)) => c.answer(stands),
            None => Answer::Lack(leaf_at(at)),
        }
    }

    /// The floor nearest `at`'s height, at the leaves: the top of a solid
    /// with room over it, under `at` or over it, or the cell's surface. None
    /// where nothing stands.
    pub(crate) fn floor_nearest(&self, at: &Q3) -> Answer<Option<f64>> {
        let z = q3::to_real(i64::from(at[2]));
        let nearest = |c: &mut SplitCell| {
            let down = c.floor_under(z);
            let up = c.above(z, BOUND.max[2]).map(|a| c.depenetrate(a).0);
            [down, up, c.ele_u()]
                .into_iter()
                .flatten()
                .min_by(|a, b| (a - z).abs().total_cmp(&(b - z).abs()))
        };
        let mut c = SplitCell::leaf(self, xy(at));
        let stands = nearest(&mut c);
        c.answer(stands)
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
    /// The walk is walked only where the eye draws a body at both its ends
    /// (`body_shown`), so it is no longer than the eye draws a body across. A
    /// `from` of unknown height is stood first: a body shows at a point, and
    /// that read is what tells where the body is, so it comes before the
    /// test and is the one read a move makes of a body not drawn. Otherwise, and where `from` cannot be stood or what the eye
    /// asks on the way is not held, there is no body for the eye and no wall
    /// its size: it ends at `to` with its height not known, and one `walled`
    /// stays where it is rather than leave the area.
    pub(crate) fn step_slide(&self, from: &Q3, to: &Q3, walled: bool) -> Q3 {
        self.stands(from)
            .exact()
            .flatten()
            .filter(|from| walk_shown(from, to))
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
            let mut c = SplitCell::at(self, [x, y, floor], path)?;
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
    /// leaves is not held. A ceiling is asked only as high as the box, never past it. The parts' rays step their tile lines on
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
        let mut c = SplitCell::at(self, [to[0], to[1], floor], *path)?;
        let (z, cell_top) = c.depenetrate(floor);
        let moved = if c.above(z, z + height).is_some() {
            None
        } else if z > floor {
            let mut from_cell = SplitCell::at(self, [from[0], from[1], floor], c.path)?;
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
