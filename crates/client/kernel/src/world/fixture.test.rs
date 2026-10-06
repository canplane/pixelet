// pxl-kernel/world/fixture.test.rs
//
// A world known exactly for the goal's tests: flat ground with solid boxes,
// as the query bounds it and as what tiles touch (`Observer`).

use pxl_spec::space::camera::eye::{Frustum, Observation};
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::v3::{add3, Aabb, V3};
use pxl_spec::space::coord::zoom::cube;
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

use crate::world::access::{Answer, Bound};
use crate::world::observer::{Footprint, Grid, Met, Observer, OnPatch, Sight};
use crate::world::pyramid::domain::{Domain, Price};
use pxl_spec::space::coord::zoom::Zoom;

#[derive(Clone)]
pub struct World {
    pub ground: f64,
    pub boxes: Vec<(V3, V3)>,
    /// Without anything resident, nothing is known and nothing is seen.
    pub answered: bool,
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

impl World {
    /// As the query bounds a node: the ground's lowest is its top cell's
    /// floor, as the index lane bounds it.
    pub fn bounds(&self, (zoom, at): Node<3>, _cell: Zoom) -> Answer<Option<Bound>> {
        if !self.answered {
            return Answer::Nothing(Tile::new(LEAF_ZOOM, 0));
        }
        let region = cube(zoom, at);
        let (lo, hi) = (region.min, region.max);
        let column = |z0: f64, z1: f64| {
            Aabb {
                min: [lo[0], lo[1], z0],
                max: [hi[0], hi[1], z1],
            }
            .meet(region)
        };
        let mut bound: Option<Aabb> = column(self.ground - 1.0 - self.slack, self.ground + self.slack);
        for (b0, b1) in self.boxes.iter().filter(|(b0, b1)| b0[0] < hi[0] && b1[0] > lo[0] && b0[1] < hi[1] && b1[1] > lo[1]) {
            if let Some(b) = column(b0[2] - self.slack, b1[2] + self.slack) {
                bound = Some(bound.map_or(b, |a| a.join(b)));
            }
        }
        Answer::Exact(bound.map(|aabb| Bound { aabb, finest: GROUND.cell() }))
    }
}

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
    fn observe(&self, grid: &Grid, far: f64, recast: &dyn Fn(usize) -> bool, met: &mut Vec<Option<Met>>, each: &mut OnPatch) -> u32 {
        if !self.answered {
            return 0;
        }
        let eye = grid.lens().eye;
        grid.footprints(far, recast, met, &mut |d| (self.first(eye, d, far), None), each)
    }
}

pub fn footprints(o: &Observation, world: &World) -> Vec<Footprint> {
    let mut sight = Sight::default();
    sight.observe(o, world, true, &[]);
    sight.footprints().cloned().collect()
}

pub fn domain(o: Observation, ladder: &[Tau], world: &World) -> Domain {
    domain_priced(o, ladder, world, &Free)
}

pub fn domain_priced(o: Observation, ladder: &[Tau], world: &World, price: &dyn Price) -> Domain {
    Domain::priced(
        o,
        GROUND.top(),
        ladder,
        0,
        {
            let w = world.clone();
            Box::new(move |n, c| w.bounds(n, c))
        },
        &footprints(&o, world),
        price,
    )
}

fn unit(v: V3) -> V3 {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    v.map(|c| c / l)
}

fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// Left, right, bottom, top, near, far; normals in, as the view's are.
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

/// 50° up and down, 16:9.
pub const REFERENCE: [f64; 2] = [0.466_307_658_154_998_6 * 16.0 / 9.0, 0.466_307_658_154_998_6];

/// All around where `forward` is none; flat planet; every zoom supplied.
pub fn eye(eye: V3, forward: Option<V3>, far: f64, ground: f64) -> Observation {
    Observation {
        eye,
        frustum: forward.map(|f| frustum(eye, f, REFERENCE, far)),
        reach: far,
        fog: f64::INFINITY,
        planet: f64::INFINITY,
        ground,
    }
}

pub fn place(center: V3, dist: f64, polar: f64, azimuth: f64) {
    use crate::control::camera::controller::{camera, set_eye_world};
    use crate::control::camera::orbit::{orbit, orbit_eye_offset};
    use crate::control::camera::store::{camera_store, Intent};
    orbit().dist = dist;
    let cam = camera();
    (cam.polar, cam.azimuth) = (polar, azimuth);
    camera_store().intent = Intent { dist, azimuth, polar };
    let e = orbit_eye_offset(dist, polar, azimuth);
    set_eye_world(&add3(&center, &e));
}

/// No cost, no limit.
pub struct Free;

use crate::world::chunk::mesh_slot::POOLS;

impl crate::world::pyramid::domain::Price for Free {
    fn piece(&self, _: crate::world::pyramid::chunk::Chunk, _: f64, _: pxl_spec::space::camera::grid::Tau) -> [u32; POOLS] {
        [0; POOLS]
    }

    fn split(&self, _: crate::world::pyramid::chunk::Chunk) -> [u32; POOLS] {
        [0; POOLS]
    }

    fn over(&self, _: &[u32; POOLS]) -> bool {
        false
    }
}
