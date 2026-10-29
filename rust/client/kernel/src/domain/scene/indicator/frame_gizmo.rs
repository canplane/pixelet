// pxl-kernel/domain/scene/indicator/frame_gizmo.rs

//! The frame gizmo (the `world_gizmo` setting), as debug lines: the pivot and
//! its cell, and about it a level ring, an upright ring facing the screen and
//! one through north. The view's forward, its up and north are told apart by
//! the spec's colors (`gizmo_spec::frame`), which the shell's legend reads.

use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;

use crate::domain::query::model::draw_list;
use crate::domain::query::World;
use crate::domain::scene::indicator::gizmo_spec::frame;
use crate::domain::scene::scene;
use crate::domain::view::frame::coord::to_world;
use crate::sys::mem::section;
use crate::sys::sched::edge::request_world_tick;
use pxl_render::look::color::linear_rgb;
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use xpute_guest::global::Global;

/// Half the cross a point is drawn as.
const POINT_HALF: f64 = 0.15;
const RING_RADIUS: f64 = 4.0;
const RING_SEGMENTS: u32 = 64;

/// Segments as pairs of vertices: position about `about`, then color. Outlines
/// only, with no faces: a ring is an outline, and a cube's faces would hide the
/// pivot inside it.
#[derive(PartialEq)]
struct Sketch {
    about: V3,
    lines: Vec<f32>,
}

impl Sketch {
    fn line(&mut self, from: V3, to: V3, color: [f64; 3]) {
        for p in [from, to] {
            let [x, y, z] = core::array::from_fn(|k| p[k] - self.about[k]);
            self.lines
                .extend([x, y, z, color[0], color[1], color[2], 1.0].map(|v| v as f32));
        }
    }

    fn point(&mut self, at: V3, color: [f64; 3]) {
        for axis in 0..3 {
            let (mut a, mut b) = (at, at);
            a[axis] -= POINT_HALF;
            b[axis] += POINT_HALF;
            self.line(a, b, color);
        }
    }

    /// Its twelve edges: those between corners one bit of the index apart.
    fn cube(&mut self, base: V3, side: f64) {
        let corner =
            |i: usize| -> V3 { core::array::from_fn(|k| base[k] + ((i >> k) & 1) as f64 * side) };
        for i in 0..8 {
            for k in 0..3 {
                if i & (1 << k) == 0 {
                    self.line(corner(i), corner(i | 1 << k), linear_rgb(frame::pivot::rgb));
                }
            }
        }
    }

    /// Of `RING_RADIUS` about `center` in the plane of the unit axes `a` and `b`.
    fn ring(&mut self, center: V3, a: V3, b: V3, (rgb, dashed): (u32, bool)) {
        let color = linear_rgb(rgb);
        let at = |i: u32| -> V3 {
            let (s, c) = libm::sincos(i as f64 / RING_SEGMENTS as f64 * core::f64::consts::TAU);
            core::array::from_fn(|k| center[k] + (a[k] * c + b[k] * s) * RING_RADIUS)
        };
        for i in (0..RING_SEGMENTS).step_by(if dashed { 2 } else { 1 }) {
            self.line(at(i), at(i + 1), color);
        }
    }
}

/// Into `s`, its lines cleared first.
fn sketch(s: &mut Sketch, pivot: &Q3, heading: Option<f64>) {
    let center = to_world(pivot);
    s.about = center;
    s.lines.clear();
    let ground = World::now().floor_under(pivot, LEAF_ZOOM).value().flatten();
    let cell = v3::floor(
        &[center[0], center[1], ground.unwrap_or(0.0)],
        units(GROUND.cell()),
    );
    s.cube(cell, units(GROUND.cell()));
    s.point(center, linear_rgb(frame::pivot::rgb));
    // First person the eye is the pivot, and nothing about it is seen.
    let Some(azimuth) = heading else { return };
    let (sin, cos) = libm::sincos(azimuth);
    let (east, north, up) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let (right, forward) = ([cos, -sin, 0.0], [sin, cos, 0.0]);
    let tip = |d: V3| -> V3 { core::array::from_fn(|k| center[k] + d[k] * RING_RADIUS) };
    s.ring(
        center,
        east,
        north,
        (frame::level::rgb, frame::level::dashed),
    );
    s.ring(
        center,
        right,
        up,
        (frame::forward::rgb, frame::forward::dashed),
    );
    s.ring(center, north, up, (frame::north::rgb, frame::north::dashed));
    s.point(tip(forward), linear_rgb(frame::forward::rgb));
    s.point(tip(north), linear_rgb(frame::north::rgb));
    s.point(tip(up), linear_rgb(frame::up::rgb));
}

/// What is drawn, whether it is, and the next sketch, kept so a step allocates nothing.
static GIZMOS: Global<(Sketch, bool, Sketch)> = Global::new();

fn set_lines(at: V3, lines: &[f32]) {
    let Some((scene, draw)) = section::map().map(|_| (scene(), draw_list())) else {
        return;
    };
    scene.debug.set(draw, at, lines, lines.len());
}

/// `heading` is the camera's azimuth third person, none first person.
/// Sketched each step and handed on only where the sketch moved, so the
/// floor under the pivot is drawn where it is as soon as it is known.
/// Before the frame's list begins (domain/scene/stage.rs), so its lines and the
/// origin they are drawn about are the same turn's.
pub fn frame_gizmo_step(show: bool, pivot: Option<Q3>, heading: Option<f64>) {
    let empty = || Sketch {
        about: [0.0; 3],
        lines: Vec::new(),
    };
    let (drawn, shown, next) = GIZMOS.get(|| (empty(), false, empty()));
    let Some(center) = pivot.filter(|_| show) else {
        if core::mem::take(shown) {
            set_lines([0.0; 3], &[]);
        }
        return;
    };
    sketch(next, &center, heading);
    if *shown && drawn == next {
        return;
    }
    set_lines(next.about, &next.lines);
    core::mem::swap(drawn, next);
    *shown = true;
    request_world_tick();
}
