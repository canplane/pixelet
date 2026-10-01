// pxl-kernel/world/observer.rs

//! What the eye observes, as the frontier's goal reads it
//! (world/pyramid/domain.rs): the observation's cell, and what of the world
//! the screen's footprint touches.
//!
//! **The cell is the observation's quantum**: the least the observer
//! promises to tell apart, an angle — `τ / SPLIT` radians a side
//! (`cell_angle`), the angle a cell of a level covers at that level's split
//! distance (pxl-spec space/lod.rs) — the same step of direction about the
//! eye whichever way it lies, on the axes of the eye's own frame and not the
//! world's. How many of a screen's pixels it spans is the screen's: the
//! target τ is set so it spans a number of logical pixels at the optical
//! axis (world/quality.rs `target`), and a perspective screen draws it wider
//! toward its edges. There is no other cell. Inside one is
//! don't-care: drawn finer, averaged or some other shape, the observation
//! is the same. What refinement keeps from happening is an approximation
//! leaking — its error, projected, crossing a cell's edge and changing what
//! a neighbouring cell observes — which `Lens::error` measures on each axis
//! of the screen and the goal holds against a cell.
//!
//! **What contributes is found by a footprint.** A piece contributes to a
//! cell where some of it is the first thing a direction in the cell meets.
//! An `Observer` estimates that for a tile of the screen at once — the
//! directions between four corner rays — as the footprint the tile's rays
//! span on what they meet (`Footprint`): every piece it touches contributes,
//! and is then held to the leak rule. The first estimator (`EdgeRays`) casts
//! the corner rays against what is resident (world/access.rs `first_hit`)
//! and takes the stretch between the hits of a tile's bottom and top edge;
//! a cone, more rays, or the GPU's report of what each tile drew are better
//! ones behind the same trait.
//!
//! **A tile is the estimator's resolution, not a cell.** Its footprint spans
//! all of its ground, whatever its size, so no piece seen is missed between
//! two samples; what the tile's size decides is only how much ground hidden
//! inside one tile — behind a crest its top rays pass over — is taken to
//! contribute, which refines what need not be and never leaves a piece
//! coarser than the rule. The tiles are independent of the quality, so one
//! cast answers every step the quality policy weighs (world/quality.rs).
//!
//! An observer reads what is resident and nothing else: it never fetches,
//! asks, prefetches or holds. What it sees decides how deep what is
//! drawn is refined, never what is fetched — that is the demand's
//! (world/pyramid.rs), which no observation of this module's enters.
//!
//! **What was seen is reused across a small move** (`Sight`), as an
//! acceleration and nothing any answer rests on: a tile's footprint is cast
//! again once the eye has turned, or moved relative to what the tile met, by
//! more than `param::REUSE_SHIFT` of a tile, or once residency under its
//! rays has changed. A stale footprint only moves which pieces refine: the
//! cover stands whatever is seen, and the quality policy counts the goal the
//! footprints actually make.

use pxl_spec::space::lod::{Tau, SPLIT};
use pxl_spec::space::observation::{Observation, Requirement};
use pxl_spec::space::v3::{Aabb, V3};

use crate::param;
use crate::world::access::{Access, Detail, Ray};

/// How far past the face a ray met a point it saw is taken, in units: into
/// what was met, so the point lies in the chunk that holds it and not in the
/// air before it. Far under the finest object's side and far over a
/// double's error at the coordinates the world spans.
const INSIDE: f64 = 1e-6;

/// The angle a cell of the observation spans a side at `tau`, in radians:
/// the quantum an error is held against.
pub fn cell_angle(tau: Tau) -> f64 {
    tau.scale() / SPLIT
}

/// The screen as directions: where the eye is, which ways it faces, and how
/// far the screen reaches either way as the tangent of its half-angle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lens {
    pub eye: V3,
    pub forward: V3,
    pub right: V3,
    pub up: V3,
    /// The tangents of the half-angles across and up.
    pub tan: [f64; 2],
}

impl Lens {
    /// The lens an observation's frustum is the planes of — left, right,
    /// bottom, top, near, far, normals in (pxl-spec space/mat4.rs
    /// `mat4_frustum_planes`). A side plane's normal is `cos a` along the
    /// axis it faces and `sin a` along the view, `a` the half-angle, so the
    /// axes are the differences of opposite normals and the tangents their
    /// ratios. None for an observer that samples every direction, which has
    /// no screen, or a frustum no screen makes.
    pub fn of(o: &Observation) -> Option<Lens> {
        let planes = o.frustum?.planes;
        let n = |k: usize| -> V3 { [planes[k * 4], planes[k * 4 + 1], planes[k * 4 + 2]] };
        let forward = unit(n(4))?;
        let right = unit(sub(n(0), n(1)))?;
        let up = unit(sub(n(2), n(3)))?;
        let tan = [dot(n(0), forward) / dot(n(0), right), dot(n(2), forward) / dot(n(2), up)];
        tan.iter().all(|t| t.is_finite() && *t > 0.0).then_some(Lens { eye: o.eye, forward, right, up, tan })
    }

    /// The angle an error at `at` spans on each axis of the screen, across
    /// and up, in radians: `error` along each axis of the world, a box
    /// anything of what is drawn may be anywhere in, projected as a box is
    /// — its extent along each world axis by how much of it the screen's
    /// axis takes. Each axis is taken where it passes the direction to `at`,
    /// square to it, so a flat piece seen at a grazing angle projects its
    /// depth small and what rises from it whole; seen from over it, the
    /// reverse. A point off the screen is taken where the screen's edge
    /// nearest it is, since what of its error shows is what crosses in
    /// there.
    ///
    /// Held over, never under: a point `δ` off `at` is seen `δ · a` across
    /// over `r + δ · d` deep, and what moves it across can bring it nearer
    /// too, so the extent across is taken over the least depth the box
    /// leaves, `r` less its diagonal, and the angle's tangent over the angle.
    /// Unbounded where the box reaches the eye.
    pub fn error(&self, at: V3, error: V3) -> [f64; 2] {
        let d = sub(at, self.eye);
        let r = dot(d, d).sqrt();
        let depth = r - dot(error, error).sqrt();
        if depth < INSIDE {
            return [f64::INFINITY; 2];
        }
        let f = dot(d, self.forward);
        let on = |axis: V3, k: usize| {
            let t = dot(d, axis);
            let t = if f > INSIDE { t / f } else { t.signum() * f64::INFINITY };
            if t.is_nan() {
                0.0
            } else {
                t.clamp(-self.tan[k], self.tan[k])
            }
        };
        let (tx, ty) = (on(self.right, 0), on(self.up, 1));
        let d = unit(std::array::from_fn(|k| self.forward[k] + tx * self.right[k] + ty * self.up[k])).unwrap_or(self.forward);
        [self.right, self.up].map(|a| {
            let square = sub(a, d.map(|c| c * dot(a, d)));
            let Some(a) = unit(square) else { return f64::INFINITY };
            (0..3).map(|k| error[k] * a[k].abs()).sum::<f64>() / depth
        })
    }
}

/// The screen cut into tiles, the estimator's samples: as many across and up
/// as tiles no wider than `param::OBSERVED_TILE` take to cover it, spread
/// evenly in angle about the optical axis on each of the screen's axes — the
/// observation's own chart, the same step of direction everywhere, which a
/// perspective screen draws wider toward its edges. Equal steps of angle, not
/// of solid angle: over a field of view the two part by little, and a tile
/// is the estimator's resolution, not anything held to a bound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    lens: Lens,
    across: [u32; 2],
}

impl Grid {
    pub fn of(lens: Lens) -> Grid {
        Grid {
            lens,
            across: lens.tan.map(|t| ((2.0 * t.atan() / param::OBSERVED_TILE).ceil() as u32).max(1)),
        }
    }

    /// How many tiles it is.
    pub fn count(&self) -> u32 {
        self.across[0] * self.across[1]
    }

    /// The lens it cuts.
    pub fn lens(&self) -> &Lens {
        &self.lens
    }

    /// Tiles across and up.
    pub fn across(&self) -> [u32; 2] {
        self.across
    }

    /// The direction through corner `[i, j]`, `i` from the left edge in
    /// `0..=across`, `j` from the bottom in `0..=up`.
    pub fn corner(&self, [i, j]: [u32; 2]) -> V3 {
        let l = self.lens;
        let at = |k: usize, n: u32| {
            let half = l.tan[k].atan();
            (-half + 2.0 * half * n as f64 / self.across[k] as f64).tan()
        };
        let (tx, ty) = (at(0, i), at(1, j));
        unit(std::array::from_fn(|k| l.forward[k] + tx * l.right[k] + ty * l.up[k])).unwrap_or(l.forward)
    }

    /// The footprint of each tile `recast` names, row by row, from `meet`: the
    /// first point a direction from the eye meets within `far` and how far
    /// along it lies, and what the answer lacked, if anything. Each corner
    /// shared by the tiles about it is met once. A tile none of whose corners
    /// meets anything has no footprint; what its corners lacked is handed
    /// with it either way.
    /// `met` is the caller's to hold across casts, so casting allocates
    /// nothing once it has held a screen's corners.
    pub fn footprints(&self, far: f64, recast: &dyn Fn(usize) -> bool, met: &mut Vec<Option<Met>>, meet: &mut dyn FnMut(V3) -> Met, each: &mut OnTile) -> u32 {
        let [nx, ny] = self.across;
        let eye = self.lens.eye;
        met.clear();
        met.resize(((nx + 1) * (ny + 1)) as usize, None);
        let mut rays = 0;
        for j in 0..ny {
            for i in 0..nx {
                let tile = (j * nx + i) as usize;
                if !recast(tile) {
                    continue;
                }
                let mut seen = false;
                let mut lacks: Lacks = [None; 4];
                let points = [[i, j], [i + 1, j], [i, j + 1], [i + 1, j + 1]].map(|c| {
                    let direction = self.corner(c);
                    let (hit, lack) = *met[(c[1] * (nx + 1) + c[0]) as usize].get_or_insert_with(|| {
                        rays += 1;
                        meet(direction)
                    });
                    seen |= hit.is_some();
                    if let Some(l) = lack.filter(|l| !lacks.contains(&Some(*l))) {
                        lacks[lacks.iter().position(Option::is_none).unwrap_or(3)] = Some(l);
                    }
                    hit.unwrap_or_else(|| (std::array::from_fn(|k| eye[k] + direction[k] * far), far))
                });
                each(tile, seen.then(|| Footprint::of(eye, points)), lacks);
            }
        }
        rays
    }
}

/// What a tile's directions meet, as the stretch between its corners' first
/// hits — or the far edge along a corner that meets nothing — and the
/// heights they span: the ground and whatever stands on it between the
/// tile's bottom and top edges.
#[derive(Clone, Debug, PartialEq)]
pub struct Footprint {
    /// Its outline along the ground: the corners' convex hull, counter-
    /// clockwise, and the box about it.
    hull: [[f64; 2]; 4],
    /// How many of `hull`'s corners it has: a tile's four at most.
    sides: usize,
    bounds: (f64, f64, f64, f64),
    /// The lowest and highest the corners stand.
    z: (f64, f64),
    /// How far the nearest of its corners is from the eye.
    pub near: f64,
    /// Its reach along the ground from the eye, as a box: what its rays
    /// crossed.
    pub reach: (f64, f64, f64, f64),
}

impl Footprint {
    /// The footprint the corner points `points` make, each with how far from
    /// `eye` it lies.
    pub fn of(eye: V3, points: [(V3, f64); 4]) -> Footprint {
        let xy: [[f64; 2]; 4] = points.map(|(p, _)| [p[0], p[1]]);
        let z = points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (p, _)| (lo.min(p[2]), hi.max(p[2])));
        let near = points.iter().map(|&(_, t)| t).fold(f64::INFINITY, f64::min);
        let reach = xy.iter().fold((eye[0], eye[1], eye[0], eye[1]), |b, p| (b.0.min(p[0]), b.1.min(p[1]), b.2.max(p[0]), b.3.max(p[1])));
        let bounds = xy.iter().fold((f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY), |b, p| {
            (b.0.min(p[0]), b.1.min(p[1]), b.2.max(p[0]), b.3.max(p[1]))
        });
        let (hull, sides) = hull(xy);
        Footprint { hull, sides, bounds, z, near, reach }
    }

    /// The box it spans: its outline's bounds along the ground, and the
    /// heights its corners stand at.
    pub fn aabb(&self) -> Aabb {
        let (x0, y0, x1, y1) = self.bounds;
        Aabb {
            min: [x0, y0, self.z.0],
            max: [x1, y1, self.z.1],
        }
    }

    /// Whether it touches box `b`: by the separating axes of the two — the
    /// box's own, which the footprint's bounds answer, and each edge of its
    /// outline's, square to up, since a footprint is kept as an outline and
    /// the heights over it.
    pub fn touches(&self, b: &Aabb) -> bool {
        let (bx0, by0, bx1, by1) = self.bounds;
        let (x0, y0, w, h) = (b.min[0], b.min[1], b.max[0] - b.min[0], b.max[1] - b.min[1]);
        if b.min[2] > self.z.1 || b.max[2] < self.z.0 || x0 > bx1 || x0 + w < bx0 || y0 > by1 || y0 + h < by0 {
            return false;
        }
        let (hull, n) = (&self.hull[..self.sides], self.sides);
        !(0..n).filter(|_| n > 1).any(|k| {
            let (p, q) = (hull[k], hull[(k + 1) % n]);
            let axis = [q[1] - p[1], p[0] - q[0]];
            let at = |v: [f64; 2]| v[0] * axis[0] + v[1] * axis[1];
            let hull = hull.iter().map(|&v| at(v)).fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(v), hi.max(v)));
            let (ax, ay) = ((axis[0] * w).abs(), (axis[1] * h).abs());
            let low = at([x0, y0]) + (axis[0] * w).min(0.0) + (axis[1] * h).min(0.0);
            let square = (low, low + ax + ay);
            hull.1 < square.0 || square.1 < hull.0
        })
    }
}

/// The convex hull of a tile's four corners, counter-clockwise (Andrew's
/// monotone chain), and how many corners it has: on the stack, since a hull
/// of four points has four at most and its chain eight.
fn hull(mut points: [[f64; 2]; 4]) -> ([[f64; 2]; 4], usize) {
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let mut chain = [[0.0; 2]; 8];
    let mut len = 0;
    for pass in [false, true] {
        let start = len;
        for k in 0..4 {
            let p = points[if pass { 3 - k } else { k }];
            while len >= start + 2 && cross(chain[len - 2], chain[len - 1], p) <= 0.0 {
                len -= 1;
            }
            chain[len] = p;
            len += 1;
        }
        len -= 1;
    }
    if len == 0 {
        (chain[0], len) = (points[0], 1);
    }
    let mut out = [[0.0; 2]; 4];
    out[..len.min(4)].copy_from_slice(&chain[..len.min(4)]);
    (out, len.min(4))
}

/// An estimator of what each tile of the screen observes.
pub trait Observer {
    /// The footprint of each tile of `grid` that `recast` names, out to
    /// `far` from the eye, handed to `each` with the tile's index and what
    /// its rays lacked; no footprint for a tile that meets nothing. How many
    /// rays it cast.
    fn observe(&self, grid: &Grid, far: f64, recast: &dyn Fn(usize) -> bool, met: &mut Vec<Option<Met>>, each: &mut OnTile) -> u32;
}

/// Handed each tile cast: its index, its footprint where it met anything,
/// and what its rays lacked.
pub type OnTile<'a> = dyn FnMut(usize, Option<Footprint>, Lacks) + 'a;

/// What a tile's rays lacked: one a corner at most, no two alike.
pub type Lacks = [Option<Requirement>; 4];

/// What a direction met: the point, just inside what it met, and how far
/// along; and what the answer lacked, as what an observation requires —
/// the level and the ground — and never as a page, which is the layer's to
/// name.
pub type Met = (Option<(V3, f64)>, Option<Requirement>);

/// The first estimator: a tile's corner rays, against what is resident, the
/// ground read at the level the target quality — the second field — draws
/// it at each distance: the finest any step does, and so the same whichever
/// step is in force.
pub struct EdgeRays<'a>(pub Access<'a>, pub Tau);

impl Observer for EdgeRays<'_> {
    fn observe(&self, grid: &Grid, far: f64, recast: &dyn Fn(usize) -> bool, met: &mut Vec<Option<Met>>, each: &mut OnTile) -> u32 {
        let eye = grid.lens().eye;
        grid.footprints(
            far,
            recast,
            met,
            &mut |direction| {
                let ray = Ray { origin: eye, direction, reach: far };
                let answer = self.0.first_hit(&ray, Detail::Distance(self.1));
                let lack = answer.lack().map(|l| Requirement {
                    level: l.page.level,
                    region: l.page.region(),
                });
                let hit = answer.value().flatten().map(|hit| (std::array::from_fn(|k| hit.point[k] - hit.normal[k] * INSIDE), hit.t));
                (hit, lack)
            },
            each,
        )
    }
}

/// What the screen's tiles last saw, kept for the next observation: each
/// tile's footprint and what its rays lacked, and the lens and far edge
/// every tile was last cast at.
#[derive(Default)]
pub struct Sight {
    cast: Option<(Lens, [u32; 2], f64)>,
    /// Each tile's footprint, what its rays lacked, and where the eye was
    /// when it was cast.
    tiles: Vec<Option<Footprint>>,
    lacks: Vec<Lacks>,
    eyes: Vec<V3>,
    /// Which tiles are cast again, and what each corner met: held across
    /// observations so observing allocates nothing once it has held a
    /// screen's tiles.
    recast: Vec<bool>,
    met: Vec<Option<Met>>,
}

impl Sight {
    /// The footprints of `o`'s screen, each cast again where it is stale
    /// and reused where not: all of them where the screen is another shape,
    /// the eye has turned past `param::REUSE_SHIFT` of a tile, the far edge
    /// has moved as much, or `everything` says residency moved where no
    /// footprint can tell; a tile where the eye has moved that much relative
    /// to what it met, or where `moved` — ground whose residency changed, as
    /// squares along the ground — falls under its rays. How many rays it
    /// cast; none of it is anything a footprint's use rests on.
    pub fn observe(&mut self, o: &Observation, observer: &impl Observer, everything: bool, moved: &[(f64, f64, f64)]) -> u32 {
        let Some(lens) = Lens::of(o) else {
            self.cast = None;
            self.tiles.clear();
            self.lacks.clear();
            self.eyes.clear();
            return 0;
        };
        let grid = Grid::of(lens);
        let shift = param::REUSE_SHIFT * param::OBSERVED_TILE;
        let whole = everything
            || self.cast.is_none_or(|(was, across, far)| {
                let turned = [(was.forward, lens.forward), (was.right, lens.right), (was.up, lens.up)]
                    .iter()
                    .map(|&(a, b)| dot(a, b).clamp(-1.0, 1.0).acos())
                    .fold(0.0, f64::max);
                let reshaped = (0..2).any(|k| (was.tan[k] - lens.tan[k]).abs() > 1e-9 * lens.tan[k]);
                across != grid.across() || reshaped || turned > shift || (o.far - far).abs() > shift * far
            });
        if whole {
            let n = grid.count() as usize;
            self.tiles.clear();
            self.tiles.resize(n, None);
            self.lacks.clear();
            self.lacks.resize(n, [None; 4]);
            self.eyes.clear();
            self.eyes.resize(n, lens.eye);
        }
        // A tile that met nothing is measured against the far edge: what it
        // would meet first is past everything nearer.
        let stale = |k: usize| {
            let moved_eye = dot(sub(lens.eye, self.eyes[k]), sub(lens.eye, self.eyes[k])).sqrt();
            whole
                || match &self.tiles[k] {
                    Some(f) => {
                        let (x0, y0, x1, y1) = f.reach;
                        moved_eye > shift * f.near || moved.iter().any(|&(x, y, side)| x < x1 && x + side > x0 && y < y1 && y + side > y0)
                    }
                    None => moved_eye > shift * o.far || !moved.is_empty(),
                }
        };
        let mut recast = core::mem::take(&mut self.recast);
        recast.clear();
        recast.extend((0..grid.count() as usize).map(stale));
        let (tiles, lacks, eyes) = (&mut self.tiles, &mut self.lacks, &mut self.eyes);
        let rays = observer.observe(&grid, o.far, &|k| recast[k], &mut self.met, &mut |k, f, lacked| {
            tiles[k] = f;
            lacks[k] = lacked;
            eyes[k] = lens.eye;
        });
        self.recast = recast;
        // Held from the last time every tile was cast, so a turn a little at
        // a time is measured whole and not a frame's worth at a time.
        if whole {
            self.cast = Some((lens, grid.across(), o.far));
        }
        rays
    }

    /// The footprints last observed, of the tiles that met anything.
    pub fn footprints(&self) -> impl Iterator<Item = &Footprint> {
        self.tiles.iter().flatten()
    }

    /// What the rays last cast lacked: the ground they read coarser than
    /// asked, or not at all, at the level they asked it at.
    pub fn lacks(&self) -> impl Iterator<Item = &Requirement> {
        self.lacks.iter().flatten().flatten()
    }
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: V3, b: V3) -> V3 {
    std::array::from_fn(|k| a[k] - b[k])
}

fn unit(v: V3) -> Option<V3> {
    let l = dot(v, v).sqrt();
    (l > 1e-12 && l.is_finite()).then(|| v.map(|c| c / l))
}

#[cfg(test)]
#[path = "observer.test.rs"]
mod test;
