// pxl-kernel/world/observer.rs

//! What the eye observes, for the domain: the observation's cell and what
//! the screen's tiles touch.
//!
//! The cell is an angle, `τ / SPLIT` radians a side, in the eye's own frame;
//! inside it nothing is promised. Refinement only stops an error, projected,
//! from crossing a cell (`Lens::error`). A piece contributes where a tile's
//! footprint touches it. Tiles are the estimator's resolution, not cells, and
//! do not depend on quality, so one cast serves every step. Footprints are
//! reused across small moves (`Sight`) as an acceleration only.

use pxl_spec::space::lod::{Tau, SPLIT};
use pxl_spec::space::observation::{Observation, Requirement};
use pxl_spec::space::v3::{Aabb, V3};

use crate::param;
use crate::world::access::{Access, Detail, Ray};

/// Distance past the met face so a seen point lies in the chunk that holds it.
const INSIDE: f64 = 1e-6;

pub fn cell_angle(tau: Tau) -> f64 {
    tau.scale() / SPLIT
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lens {
    pub eye: V3,
    pub forward: V3,
    pub right: V3,
    pub up: V3,
    pub tan: [f64; 2],
}

impl Lens {
    /// From the frustum's planes (left, right, bottom, top, near, far; normals
    /// in): the axes are differences of opposite normals and the tangents their
    /// ratios. None without a screen.
    pub fn of(o: &Observation) -> Option<Lens> {
        let planes = o.frustum?.planes;
        let n = |k: usize| -> V3 { [planes[k * 4], planes[k * 4 + 1], planes[k * 4 + 2]] };
        let forward = unit(n(4))?;
        let right = unit(sub(n(0), n(1)))?;
        let up = unit(sub(n(2), n(3)))?;
        let tan = [dot(n(0), forward) / dot(n(0), right), dot(n(2), forward) / dot(n(2), up)];
        tan.iter().all(|t| t.is_finite() && *t > 0.0).then_some(Lens { eye: o.eye, forward, right, up, tan })
    }

    /// The angle the error box at `at` spans across and up. Each screen axis is
    /// taken square to the direction to `at` (clamped to the screen's edge), so
    /// a flat piece at a grazing angle projects its depth small.
    ///
    /// An overestimate: the extent is divided by the nearest depth the box
    /// allows, `r` less its diagonal. Unbounded where the box reaches the eye.
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

/// The screen in tiles no wider than `OBSERVED_TILE`, at equal steps of
/// angle about the optical axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    lens: Lens,
    across: [u32; 2],
}

impl Grid {
    pub fn of(lens: Lens) -> Grid {
        Grid {
            lens,
            across: lens.tan.map(|t| ((2.0 * libm::atan(t) / param::OBSERVED_TILE).ceil() as u32).max(1)),
        }
    }

    pub fn count(&self) -> u32 {
        self.across[0] * self.across[1]
    }

    pub fn lens(&self) -> &Lens {
        &self.lens
    }

    pub fn across(&self) -> [u32; 2] {
        self.across
    }

    pub fn corner(&self, [i, j]: [u32; 2]) -> V3 {
        let l = self.lens;
        let at = |k: usize, n: u32| {
            let half = libm::atan(l.tan[k]);
            libm::tan(-half + 2.0 * half * n as f64 / self.across[k] as f64)
        };
        let (tx, ty) = (at(0, i), at(1, j));
        unit(std::array::from_fn(|k| l.forward[k] + tx * l.right[k] + ty * l.up[k])).unwrap_or(l.forward)
    }

    /// Footprints of the tiles `recast` names; each shared corner is cast once.
    /// `met` is reused across casts so casting stops allocating once warm.
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
                    let (hit, lack) = *met[(c[1] * (nx + 1) + c[0]) as usize].get_or_insert_with(|| {
                        rays += 1;
                        meet(self.corner(c))
                    });
                    seen |= hit.is_some();
                    if let Some(l) = lack.filter(|l| !lacks.contains(&Some(*l))) {
                        lacks[lacks.iter().position(Option::is_none).unwrap_or(3)] = Some(l);
                    }
                    hit.unwrap_or_else(|| {
                        let direction = self.corner(c);
                        (std::array::from_fn(|k| eye[k] + direction[k] * far), far)
                    })
                });
                each(tile, seen.then(|| Footprint::of(eye, points)), lacks);
            }
        }
        rays
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Footprint {
    /// Convex hull of the corners along the ground, counter-clockwise.
    hull: [[f64; 2]; 4],
    sides: usize,
    bounds: (f64, f64, f64, f64),
    z: (f64, f64),
    pub near: f64,
    pub reach: (f64, f64, f64, f64),
}

impl Footprint {
    pub fn of(eye: V3, points: [(V3, f64); 4]) -> Footprint {
        let xy: [[f64; 2]; 4] = points.map(|(p, _)| [p[0], p[1]]);
        let z = points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (p, _)| (lo.min(p[2]), hi.max(p[2])));
        let near = points.iter().map(|&(_, t)| t).fold(f64::INFINITY, f64::min);
        let bounds = xy.iter().fold((f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY), |b, p| {
            (b.0.min(p[0]), b.1.min(p[1]), b.2.max(p[0]), b.3.max(p[1]))
        });
        let reach = (bounds.0.min(eye[0]), bounds.1.min(eye[1]), bounds.2.max(eye[0]), bounds.3.max(eye[1]));
        let (hull, sides) = hull(xy);
        Footprint { hull, sides, bounds, z, near, reach }
    }

    pub fn aabb(&self) -> Aabb {
        let (x0, y0, x1, y1) = self.bounds;
        Aabb {
            min: [x0, y0, self.z.0],
            max: [x1, y1, self.z.1],
        }
    }

    /// Separating-axis test: the box's axes, then each hull edge's normal.
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

/// Andrew's monotone chain, on the stack: four points give a chain of at most eight.
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

pub trait Observer {
    /// Returns how many rays were cast.
    fn observe(&self, grid: &Grid, far: f64, recast: &dyn Fn(usize) -> bool, met: &mut Vec<Option<Met>>, each: &mut OnTile) -> u32;
}

pub type OnTile<'a> = dyn FnMut(usize, Option<Footprint>, Lacks) + 'a;

pub type Lacks = [Option<Requirement>; 4];

/// A lack is given as a requirement (level and ground), never as a page,
/// which is the layer's to name.
pub type Met = (Option<(V3, f64)>, Option<Requirement>);

/// Corner rays against what is resident, at the target quality's levels, so
/// the answer is the same whichever step is in force.
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

#[derive(Default)]
pub struct Sight {
    cast: Option<(Lens, [u32; 2], f64)>,
    tiles: Vec<Option<Footprint>>,
    lacks: Vec<Lacks>,
    eyes: Vec<V3>,
    recast: Vec<bool>,
    met: Vec<Option<Met>>,
}

impl Sight {
    /// Recasts every tile where the screen changed shape, the eye turned or the
    /// far edge moved past `REUSE_SHIFT` of a tile, or `everything` is set; and
    /// any tile the eye moved that much relative to, or whose rays cross `moved`
    /// ground. Returns rays cast.
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
                    .map(|&(a, b)| libm::acos(dot(a, b).clamp(-1.0, 1.0)))
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
        // A tile that met nothing is measured against the far edge.
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
        // Kept from the last full cast, so a slow turn is measured whole rather
        // than a frame at a time.
        if whole {
            self.cast = Some((lens, grid.across(), o.far));
        }
        rays
    }

    pub fn footprints(&self) -> impl Iterator<Item = &Footprint> {
        self.tiles.iter().flatten()
    }

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
