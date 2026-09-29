// pxl-kernel/world/fixture.test.rs
//
// A world the goal's tests observe, known exactly: flat ground at a height
// and solid boxes on it, as the bounds a domain reads (`Bounds`) and
// as what an eye's tiles touch of it (`Observer`); and an eye looking one
// way, as the observation whose frustum is its screen.

use pxl_spec::space::lod::Tau;
use pxl_spec::space::observation::{Bounds, Contents, Frustum, Observation};
use pxl_spec::space::pyramid::{GROUND, TOP_LEVEL};
use pxl_spec::space::v3::{Aabb, V3};

use crate::world::observer::{Footprint, Grid, Observer, OnTile, Sight};
use crate::world::pyramid::domain::Domain;

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

impl Bounds for World {
    /// The ground and the boxes in `region`, each by the heights it stands
    /// at over the region's square. The ground's lowest is its top cell's
    /// floor, as the index lane bounds it: what stands is the cells under
    /// the top. Flat, it rises a cell's step over an index cell of z16's
    /// sixteen, and steeper by the slack; a box is off by all of itself.
    fn contents(&self, level: u32, region: Aabb) -> Contents {
        if !self.answered {
            return Contents::Unknown;
        }
        let (lo, hi) = (region.min, region.max);
        let column = |z0: f64, z1: f64| {
            Aabb {
                min: [lo[0], lo[1], z0],
                max: [hi[0], hi[1], z1],
            }
            .meet(region)
        };
        let cell = GROUND.scale(level).side();
        let (mut bound, mut rise): (Option<Aabb>, f64) = (None, 0.0);
        if let Some(g) = column(self.ground - 1.0 - self.slack, self.ground + self.slack) {
            bound = Some(g);
            rise = (1.0 + 2.0 * self.slack) / 16.0 * cell;
        }
        for (b0, b1) in self.boxes.iter().filter(|(b0, b1)| b0[0] < hi[0] && b1[0] > lo[0] && b0[1] < hi[1] && b1[1] > lo[1]) {
            if let Some(b) = column(b0[2] - self.slack, b1[2] + self.slack) {
                bound = Some(bound.map_or(b, |a| a.join(b)));
                rise = rise.max(b.max[2] - b.min[2]);
            }
        }
        let extent = region.extent();
        let error = [cell, cell, rise];
        bound.map_or(Contents::Empty, |bound| Contents::Holds {
            bound,
            error: std::array::from_fn(|k| error[k].clamp(0.0, extent[k])),
        })
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
    pub fn first(&self, o: V3, d: V3, far: f64) -> Option<(V3, f64)> {
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
    fn observe(&self, grid: &Grid, far: f64, recast: &dyn Fn(usize) -> bool, each: &mut OnTile) -> u32 {
        if !self.answered {
            return 0;
        }
        let eye = grid.lens().eye;
        grid.footprints(far, recast, &mut |d| (self.first(eye, d, far), None), each)
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
