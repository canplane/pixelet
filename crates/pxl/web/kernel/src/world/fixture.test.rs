// pxl-kernel/world/fixture.test.rs
//
// A world the goal's tests observe, known exactly: flat ground at a height
// and solid boxes on it, as the bounds a domain reads (`ColumnBounds`) and
// as what an eye's tiles touch of it (`Observer`); and an eye looking one
// way, as the observation whose frustum is its screen.

use pxl_spec::space::hierarchy::Scale;
use pxl_spec::space::lod::Tau;
use pxl_spec::space::observation::{Frustum, Observation};
use pxl_spec::space::pyramid::{LevelPage, TOP_LEVEL};
use pxl_spec::space::v3::V3;

use crate::world::observer::{Footprint, Grid, Observer, Sight};
use crate::world::pyramid::domain::{ColumnBounds, Domain, Spans};

/// Flat ground and boxes on it, each as solid as the ground and known as
/// objects are, by the span they reach and the heights they are in.
#[derive(Clone)]
pub struct World {
    pub ground: f64,
    pub boxes: Vec<(V3, V3)>,
    /// Whether anything over the ground is resident: without, nothing is
    /// known and nothing is seen.
    pub answered: bool,
    /// How much looser than the truth the bounds are, each way.
    pub slack: f64,
}

impl World {
    pub fn flat(ground: f64) -> World {
        World {
            ground,
            boxes: Vec::new(),
            answered: true,
            slack: 0.0,
        }
    }

    pub fn with(mut self, lo: V3, hi: V3) -> World {
        self.boxes.push((lo, hi));
        self
    }
}

impl ColumnBounds for World {
    fn spans(&self, _page: LevelPage, column: (Scale, [i64; 2])) -> Spans {
        if !self.answered {
            return (None, None);
        }
        // The ground's lowest is its top cell's floor, as the index lane
        // bounds a column: what stands is the cells under the top.
        let objects = self
            .over(column)
            .fold(None, |span: Option<(f64, f64)>, (lo, hi)| Some(span.map_or((lo[2], hi[2]), |(l, h)| (l.min(lo[2]), h.max(hi[2])))));
        (
            Some((self.ground - 1.0 - self.slack, self.ground + self.slack)),
            objects.map(|(lo, hi)| (lo - self.slack, hi + self.slack)),
        )
    }

    fn holds(&self, column: (Scale, [i64; 2]), z: (f64, f64)) -> bool {
        self.over(column).any(|(lo, hi)| lo[2] - self.slack < z.1 && hi[2] + self.slack >= z.0)
    }

    /// Flat: a cell's step over an index cell of z16's sixteen, and steeper
    /// by the slack.
    fn slope(&self, _page: LevelPage, _column: (Scale, [i64; 2])) -> Option<f64> {
        self.answered.then(|| (1.0 + 2.0 * self.slack) / 16.0)
    }
}

impl World {
    /// The boxes over `column`.
    fn over(&self, (scale, [x, y]): (Scale, [i64; 2])) -> impl Iterator<Item = &(V3, V3)> {
        let side = scale.side();
        let (x0, y0) = (x as f64 * side, y as f64 * side);
        self.boxes.iter().filter(move |(lo, hi)| lo[0] < x0 + side && hi[0] > x0 && lo[1] < y0 + side && hi[1] > y0)
    }
}

/// Where a ray from `o` along `d` enters the box, and through which face.
fn enter(o: V3, d: V3, lo: V3, hi: V3) -> Option<(f64, V3)> {
    let (mut near, mut far, mut normal) = (f64::NEG_INFINITY, f64::INFINITY, [0.0; 3]);
    for k in 0..3 {
        if d[k].abs() < 1e-12 {
            if o[k] < lo[k] || o[k] > hi[k] {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo[k] - o[k]) / d[k], (hi[k] - o[k]) / d[k]);
        let (a, b) = if d[k] > 0.0 { (a, b) } else { (b, a) };
        if a > near {
            near = a;
            normal = [0.0; 3];
            normal[k] = if d[k] > 0.0 { -1.0 } else { 1.0 };
        }
        far = far.min(b);
    }
    (near <= far && near > 0.0).then_some((near, normal))
}

impl World {
    /// The first point a ray from `o` along `d` meets within `far`, just
    /// inside what it met, and how far along.
    fn first(&self, o: V3, d: V3, far: f64) -> Option<(V3, f64)> {
        let ground = (d[2] < 0.0).then(|| ((self.ground - o[2]) / d[2], [0.0, 0.0, 1.0]));
        let (t, n) = self
            .boxes
            .iter()
            .filter_map(|&(lo, hi)| enter(o, d, lo, hi))
            .chain(ground)
            .filter(|&(t, _)| t > 0.0 && t <= far)
            .min_by(|a, b| a.0.total_cmp(&b.0))?;
        Some((std::array::from_fn(|k| o[k] + d[k] * t - n[k] * 1e-6), t))
    }
}

impl Observer for World {
    fn observe(&self, grid: &Grid, far: f64, recast: &dyn Fn(usize) -> bool, each: &mut dyn FnMut(usize, Option<Footprint>)) -> u32 {
        if !self.answered {
            return 0;
        }
        let eye = grid.lens().eye;
        grid.footprints(far, recast, &mut |d| self.first(eye, d, far), each)
    }
}

/// What the tiles of `o`'s screen touch of `world`, all of them cast.
pub fn footprints(o: &Observation, world: &World) -> Vec<Footprint> {
    let mut sight = Sight::default();
    sight.observe(o, world, true, &[]);
    sight.footprints().cloned().collect()
}

/// The domain of `o` over `world` at each quality of `ladder`, the finest in
/// force.
pub fn domain(o: Observation, ladder: &[Tau], world: &World) -> Domain {
    Domain::of(o, ladder, 0, Box::new(world.clone()), footprints(&o, world))
}

fn unit(v: V3) -> V3 {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    v.map(|c| c / l)
}

fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// The planes of an eye at `eye` looking along `forward`, Z up, its screen
/// `tan` of its half-angles across and up, out to `far`: left, right,
/// bottom, top, near, far, normals in, as the view's are.
pub fn frustum(eye: V3, forward: V3, tan: [f64; 2], far: f64) -> Frustum {
    let f = unit(forward);
    let r = unit(cross(f, [0.0, 0.0, 1.0]));
    let u = cross(r, f);
    let side = |axis: V3, sign: f64, t: f64| unit(std::array::from_fn(|k| sign * axis[k] + t * f[k]));
    let normals = [side(r, 1.0, tan[0]), side(r, -1.0, tan[0]), side(u, 1.0, tan[1]), side(u, -1.0, tan[1]), f, f.map(|c| -c)];
    let mut planes = [0.0; 24];
    for (k, n) in normals.iter().enumerate() {
        let d = -(n[0] * eye[0] + n[1] * eye[1] + n[2] * eye[2]);
        let d = match k {
            4 => d - 0.01,
            5 => d + far,
            _ => d,
        };
        planes[k * 4..k * 4 + 4].copy_from_slice(&[n[0], n[1], n[2], d]);
    }
    Frustum { planes }
}

/// The reference screen's tangents: 50° up and down, 16:9.
pub const REFERENCE: [f64; 2] = [0.466_307_658_154_998_6 * 16.0 / 9.0, 0.466_307_658_154_998_6];

/// An eye at `eye` over ground at `ground` looking along `forward` through
/// the reference screen, or all around where `forward` is none, out to
/// `far`, on a flat planet, over a source that supplies every level.
pub fn eye(eye: V3, forward: Option<V3>, far: f64, ground: f64) -> Observation {
    Observation {
        eye,
        frustum: forward.map(|f| frustum(eye, f, REFERENCE, far)),
        far,
        top: TOP_LEVEL,
        planet: f64::INFINITY,
        ground,
    }
}
