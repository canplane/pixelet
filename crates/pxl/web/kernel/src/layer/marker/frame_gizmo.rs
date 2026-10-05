// pxl-kernel/layer/marker/frame_gizmo.rs

//! The debug gizmos (the `gizmos` setting), as debug lines: the map center and
//! its cell, and about it a level ring, an upright ring facing the screen and
//! one through north. The view's forward, its up and north are told apart by
//! color; nothing is labeled.

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::{floor3n, V3};

use crate::control::camera::controller::{azimuth, dist};
use crate::kernel::kernel;
use crate::world::access::Access;
use crate::world::change::request_world_tick;
use crate::world::frame::coord::to_world;
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;
use xpute_runtime::global::Global;

const BLACK: V3 = [0.0, 0.0, 0.0];
const RED: V3 = [1.0, 0.0, 0.0];
const GREEN: V3 = [0.0, 1.0, 0.0];
const BLUE: V3 = [0.0, 0.0, 1.0];
/// Half the cross a point is drawn as.
const POINT_HALF: f64 = 0.15;
const RING_RADIUS: f64 = 4.0;
const RING_SEGMENTS: u32 = 64;

/// Segments as pairs of vertices: position about `about`, then color.
struct Sketch {
    about: V3,
    lines: Vec<f32>,
}

impl Sketch {
    fn line(&mut self, from: V3, to: V3, color: V3) {
        for p in [from, to] {
            let [x, y, z] = core::array::from_fn(|k| p[k] - self.about[k]);
            self.lines.extend([x, y, z, color[0], color[1], color[2], 1.0].map(|v| v as f32));
        }
    }

    fn point(&mut self, at: V3, color: V3) {
        for axis in 0..3 {
            let (mut a, mut b) = (at, at);
            a[axis] -= POINT_HALF;
            b[axis] += POINT_HALF;
            self.line(a, b, color);
        }
    }

    /// Its twelve edges: those between corners one bit of the index apart.
    fn cube(&mut self, base: V3, side: f64) {
        let corner = |i: usize| -> V3 { core::array::from_fn(|k| base[k] + ((i >> k) & 1) as f64 * side) };
        for i in 0..8 {
            for k in 0..3 {
                if i & (1 << k) == 0 {
                    self.line(corner(i), corner(i | 1 << k), BLACK);
                }
            }
        }
    }

    /// Dashed, of `RING_RADIUS` about `center` in the plane of the unit axes `a` and `b`.
    fn ring(&mut self, center: V3, a: V3, b: V3, color: V3) {
        let at = |i: u32| -> V3 {
            let (s, c) = libm::sincos(i as f64 / RING_SEGMENTS as f64 * core::f64::consts::TAU);
            core::array::from_fn(|k| center[k] + (a[k] * c + b[k] * s) * RING_RADIUS)
        };
        for i in (0..RING_SEGMENTS).step_by(2) {
            self.line(at(i), at(i + 1), color);
        }
    }
}

fn sketch(map_center: &Pvf3) -> Sketch {
    let center = to_world(map_center);
    let mut s = Sketch { about: center, lines: Vec::new() };
    let ground = Access::now().floor_under(map_center).value().flatten();
    let cell = floor3n(&[center[0], center[1], ground.unwrap_or(0.0)], U_PER_CELL);
    s.cube(cell, U_PER_CELL);
    s.point(center, BLACK);
    // First person the eye is the map center, and nothing about it is seen.
    if dist() == 0.0 {
        return s;
    }
    let (sin, cos) = libm::sincos(azimuth());
    let (east, north, up) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let (right, forward) = ([cos, -sin, 0.0], [sin, cos, 0.0]);
    let tip = |d: V3| -> V3 { core::array::from_fn(|k| center[k] + d[k] * RING_RADIUS) };
    s.ring(center, east, north, BLACK);
    s.ring(center, right, up, RED);
    s.ring(center, north, up, GREEN);
    s.point(tip(forward), RED);
    s.point(tip(north), GREEN);
    s.point(tip(up), BLUE);
    s
}

/// Map center, tiles' revision, and the azimuth the lines were drawn for.
type Drawn = (Pvf3, u32, f64);

static GIZMOS: Global<Option<Drawn>> = Global::new();

fn set_lines(at: V3, vertices: &[f32]) {
    let Some((_, sched)) = kernel().world() else { return };
    sched.debug.set(&mut sched.draw, at, vertices);
}

pub fn debug_gizmos_step(show: bool, map_center: Option<Pvf3>) {
    let drawn = GIZMOS.get(|| None);
    let Some(center) = map_center.filter(|_| show) else {
        if drawn.take().is_some() {
            set_lines([0.0; 3], &[]);
        }
        return;
    };
    let key = (center, Terrain::pyramid().paging().revision(), azimuth());
    if *drawn == Some(key) {
        return;
    }
    *drawn = Some(key);
    let s = sketch(&center);
    set_lines(s.about, &s.lines);
    request_world_tick();
}
