// pxl-kernel/view/sight.rs

//! What the eye observes: one ray a cell of the camera grid, cast down the
//! pyramid against what is held (`World::first_hit`), the same wherever the
//! eye is (decision/20261012-the-ground-is-a-grid-then-overwrites.md, "What
//! is drawn"). Where each ray stops, how far, and the tiles its descent
//! lacked are the observation; what is made of it is the chunks'.
//!
//! A cast runs a slice a turn and is published whole. A ray whose path
//! crosses nothing that moved since it was cast answers what it answered,
//! so only those that do are cast again: reuse that cannot change the result.

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::camera::grid::{target_cell_px, Tau, SPLIT};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::v3::{dot3, len3, sub3, V3};
use pxl_spec::space::coord::world::nearest_box;
use pxl_spec::space::coord::zoom::{of_side, Zoom};
use pxl_spec::space::layer::tile::Tile;
use xpute_guest::global::Global;

use crate::param;
use crate::query::{Detail, Ray, World};
use crate::view::frame::pose::view;
use crate::view::observation::observation;

/// Distance past the met face, so a stop lies in the block the ray met.
const INSIDE: f64 = 1e-6;

/// The camera grid's cell at `tau`, as an angle.
pub fn cell_angle(tau: Tau) -> f64 {
    tau.scale() / SPLIT
}

/// The quality: a cell spans as many logical pixels at the optical axis as
/// the camera grid's at the reference view (`target_cell_px`), never finer
/// than `SPLIT`, and the grid holds no more than `GRID_CELLS_MAX` cells. The
/// screen plane stands `height / (2 · tan(fov / 2))` pixels from the eye, so
/// `n` of them span `2 · atan(n / 2 / that)`. The grid is the screen's angles
/// across, `a`, and up, `b`, cut into cells `c` a side, ⌈a / c⌉ · ⌈b / c⌉ of
/// them, at most (a / c + 1)(b / c + 1): the least `c` that keeps that within
/// the most is the root of `ab·u² + (a + b)·u + 1 − most` in `u = 1 / c`.
pub fn tau() -> Tau {
    let v = view();
    let tan_up = libm::tan(v.fov.to_radians() / 2.0);
    let half = target_cell_px() * tan_up / v.height;
    let [a, b] = [libm::atan(tan_up * v.width / v.height), libm::atan(tan_up)].map(|t| 2.0 * t);
    let most = param::GRID_CELLS_MAX;
    let u = (-(a + b) + libm::sqrt((a + b) * (a + b) + 4.0 * a * b * (most - 1.0))) / (2.0 * a * b);
    let cell = (2.0 * libm::atan(half)).max(1.0 / u);
    Tau::of((cell * SPLIT).max(1.0))
}

/// The screen as the eye sees it: its axes and the tangents of its half-angles.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Lens {
    forward: V3,
    right: V3,
    up: V3,
    tan: [f64; 2],
}

impl Lens {
    /// From the frustum's planes (left, right, bottom, top, near, far; normals
    /// in): the axes are differences of opposite normals and the tangents their
    /// ratios. None without a screen.
    fn of(o: &Observation) -> Option<Lens> {
        let planes = o.frustum?.planes;
        let n = |k: usize| -> V3 { [planes[k * 4], planes[k * 4 + 1], planes[k * 4 + 2]] };
        let forward = unit(n(4))?;
        let right = unit(sub3(&n(0), &n(1)))?;
        let up = unit(sub3(&n(2), &n(3)))?;
        let tan = [dot3(&n(0), &forward) / dot3(&n(0), &right), dot3(&n(2), &forward) / dot3(&n(2), &up)];
        tan.iter().all(|t| t.is_finite() && *t > 0.0).then_some(Lens { forward, right, up, tan })
    }

    /// The camera grid's cells across and up the screen at `tau`, at equal
    /// steps of angle about the optical axis.
    fn across(&self, tau: Tau) -> [u32; 2] {
        self.tan.map(|t| ((2.0 * libm::atan(t) / cell_angle(tau)).ceil() as u32).max(1))
    }

    /// A cell of `across`, as its angles across and up: the view's angle
    /// on each axis cut into as many.
    fn step(&self, across: [u32; 2]) -> [f64; 2] {
        core::array::from_fn(|k| 2.0 * libm::atan(self.tan[k]) / f64::from(across[k].max(1)))
    }

    /// The direction through the middle of cell `[i, j]` of `across`.
    fn through(&self, across: [u32; 2], [i, j]: [u32; 2]) -> V3 {
        let step = self.step(across);
        let at = |k: usize, n: u32| libm::tan(-libm::atan(self.tan[k]) + step[k] * (f64::from(n) + 0.5));
        let (tx, ty) = (at(0, i), at(1, j));
        unit(core::array::from_fn(|k| self.forward[k] + tx * self.right[k] + ty * self.up[k])).unwrap_or(self.forward)
    }
}

/// Where a cell's ray stopped: in the block it met, `t` from the eye, and
/// the zoom that block is drawn at there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stop {
    pub at: V3,
    pub t: f64,
    pub zoom: Zoom,
}

/// One cell's ray as cast: where it stopped, none where it met nothing
/// within reach, and the tile its descent lacked.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Cell {
    stop: Option<Stop>,
    lack: Option<Tile>,
}

/// An observation, whole: what the eye observed from, at which quality, and
/// what each of the camera grid's cells met.
#[derive(Default)]
pub struct Sight {
    observation: Option<Observation>,
    tau: Option<Tau>,
    lens: Option<Lens>,
    across: [u32; 2],
    cells: Vec<Cell>,
    /// Made whole again at each publish.
    stops: Vec<Stop>,
    lacks: Vec<Tile>,
    /// Which cast this is, so a reader knows when to read it again.
    generation: u32,
}

impl Sight {
    pub fn observation(&self) -> Option<&Observation> {
        self.observation.as_ref()
    }

    pub fn tau(&self) -> Option<Tau> {
        self.tau
    }

    /// The camera grid's cell as cast, its angles across and up the
    /// screen (`Lens::step`); none before a screen.
    pub fn step(&self) -> [f64; 2] {
        self.lens.map_or([0.0; 2], |l| l.step(self.across))
    }

    /// Where the rays stopped.
    pub fn stops(&self) -> &[Stop] {
        &self.stops
    }

    /// The tiles the rays' descents lacked, sorted.
    pub fn lacks(&self) -> &[Tile] {
        &self.lacks
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Whether cell `k`'s ray, as cast, crosses `b`: from the eye to where it
    /// stopped, or to the far edge where it met nothing. On the eye's lap.
    fn crosses(&self, k: usize, b: &Aabb) -> bool {
        let (Some(o), Some(lens)) = (self.observation, self.lens) else { return true };
        let [nx, _] = self.across;
        let d = lens.through(self.across, [k as u32 % nx, k as u32 / nx]);
        let end = self.cells[k].stop.map_or(o.far(), |s| s.t);
        nearest_box(b, o.eye[0]).span_along(o.eye, d).is_some_and(|(enter, leave, _)| enter <= end && leave >= 0.0)
    }
}

/// A cast under way: the cells left to cast, by index.
struct Cast {
    sight: Sight,
    left: Vec<u32>,
}

#[derive(Default)]
struct Casting {
    published: Sight,
    job: Option<Cast>,
    spare: Option<Sight>,
    /// Where the world moved since the last cast began.
    moved: Vec<Aabb>,
    left: Vec<u32>,
    generation: u32,
    /// Rays cast since `take_rays`.
    rays: u32,
}

static CASTING: Global<Casting> = Global::new();

/// Starts the next cast when the observation or the quality moved, or the
/// world did where a ray passed; a cast under way is never restarted.
pub fn start() {
    let c = CASTING.get(Casting::default);
    if c.job.is_some() {
        return;
    }
    let (o, tau) = (observation(), tau());
    let lens = o.as_ref().and_then(Lens::of);
    let p = &c.published;
    let same = p.observation == o && p.tau == Some(tau) && p.lens == lens;
    if same && c.moved.is_empty() {
        return;
    }
    let mut sight = c.spare.take().unwrap_or_default();
    let mut left = core::mem::take(&mut c.left);
    left.clear();
    (sight.observation, sight.tau, sight.lens) = (o, Some(tau), lens);
    sight.across = lens.map_or([0, 0], |l| l.across(tau));
    let n = (sight.across[0] * sight.across[1]) as usize;
    sight.cells.clear();
    if same {
        // The same cells: a ray that crosses nothing moved answers as it did.
        sight.cells.extend_from_slice(&p.cells);
        left.extend((0..n as u32).filter(|&k| c.moved.iter().any(|b| p.crosses(k as usize, b))));
    } else {
        sight.cells.resize(n, Cell::default());
        left.extend(0..n as u32);
    }
    c.moved.clear();
    // Cast from the last, popped from the end, so the first cell goes first.
    left.reverse();
    c.job = Some(Cast { sight, left });
}

/// A slice of the cast; publishes it whole. Returns whether one is still under way.
pub fn advance(stop: &dyn Fn() -> bool) -> bool {
    // At least a slice a turn.
    const SLICE: u32 = 16;
    let c = CASTING.get(Casting::default);
    let Some(job) = c.job.as_mut() else { return false };
    let (Some(o), Some(tau), Some(lens)) = (job.sight.observation, job.sight.tau, job.sight.lens) else {
        job.left.clear();
        return publish(c);
    };
    let world = World::now();
    let mut k = 0;
    while let Some(cell) = job.left.pop() {
        let [nx, _] = job.sight.across;
        let direction = lens.through(job.sight.across, [cell % nx, cell / nx]);
        let ray = Ray {
            origin: o.eye,
            direction,
            reach: o.far(),
        };
        let answer = world.first_hit(&ray, Detail::Distance(tau));
        let lack = answer.lack();
        let met = answer.value().flatten().map(|hit| Stop {
            at: core::array::from_fn(|a| hit.point[a] - hit.normal[a] * INSIDE),
            t: hit.t,
            zoom: of_side(hit.cube.max[0] - hit.cube.min[0]),
        });
        job.sight.cells[cell as usize] = Cell { stop: met, lack };
        c.rays += 1;
        k += 1;
        if k % SLICE == 0 && stop() {
            return true;
        }
    }
    publish(c)
}

fn publish(c: &mut Casting) -> bool {
    let Some(Cast { mut sight, left }) = c.job.take() else { return false };
    sight.stops.clear();
    sight.stops.extend(sight.cells.iter().filter_map(|cell| cell.stop));
    sight.lacks.clear();
    sight.lacks.extend(sight.cells.iter().filter_map(|cell| cell.lack));
    sight.lacks.sort_unstable();
    sight.lacks.dedup();
    c.generation = c.generation.wrapping_add(1);
    sight.generation = c.generation;
    c.spare = Some(core::mem::replace(&mut c.published, sight));
    c.left = left;
    false
}

/// The world moved in `regions`: the rays that crossed them are cast again.
pub fn moved(regions: impl IntoIterator<Item = Aabb>) {
    CASTING.get(Casting::default).moved.extend(regions);
}

/// The quality in force: the published cast's, else the one asked for now.
pub fn effective() -> Tau {
    current().and_then(Sight::tau).unwrap_or_else(tau)
}

/// The last cast published; none before the first.
pub fn current() -> Option<&'static Sight> {
    let c = CASTING.get(Casting::default);
    (c.published.generation != 0).then_some(&c.published)
}

pub fn casting() -> bool {
    CASTING.get(Casting::default).job.is_some()
}

pub fn take_rays() -> u32 {
    core::mem::take(&mut CASTING.get(Casting::default).rays)
}

fn unit(v: V3) -> Option<V3> {
    let l = len3(&v);
    (l > 1e-12 && l.is_finite()).then(|| v.map(|c| c / l))
}
