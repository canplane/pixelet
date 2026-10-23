// pxl-kernel/scene/indicator/frame_gizmo.rs

//! The frame gizmo (the `gizmo` setting), as debug lines: the focus and
//! its cell, and about it a level ring, an upright ring facing the screen and
//! one through north. The view's forward, its up and north are told apart by
//! the spec's colors (`gizmo_spec::frame`), which the shell's legend reads.

use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;

use crate::kernel;
use crate::query::World;
use crate::scene::indicator::gizmo_spec::frame;
use crate::sched::edge::request_world_tick;
use crate::view::frame::coord::to_world;
use pxl_render::look::color::linear_rgb;
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use xpute_guest::global::Global;

/// Half the cross a point is drawn as.
const POINT_HALF: f64 = 0.15;
const RING_RADIUS: f64 = 4.0;
const RING_SEGMENTS: u32 = 64;

/// Segments as pairs of vertices: position about `about`, then color. Outlines
/// only, with no faces: a ring is an outline, and a cube's faces would hide the
/// focus inside it.
struct Sketch {
    about: V3,
    lines: Vec<f32>,
}

impl Sketch {
    fn line(&mut self, from: V3, to: V3, color: [f64; 3]) {
        for p in [from, to] {
            let [x, y, z] = core::array::from_fn(|k| p[k] - self.about[k]);
            self.lines.extend([x, y, z, color[0], color[1], color[2], 1.0].map(|v| v as f32));
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
        let corner = |i: usize| -> V3 { core::array::from_fn(|k| base[k] + ((i >> k) & 1) as f64 * side) };
        for i in 0..8 {
            for k in 0..3 {
                if i & (1 << k) == 0 {
                    self.line(corner(i), corner(i | 1 << k), linear_rgb(frame::focus::rgb));
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
fn sketch(s: &mut Sketch, focus: &Q3, heading: Option<f64>) {
    let center = to_world(focus);
    s.about = center;
    s.lines.clear();
    let ground = World::now().floor_under(focus, LEAF_ZOOM).value().flatten();
    let cell = v3::floor(&[center[0], center[1], ground.unwrap_or(0.0)], units(GROUND.cell()));
    s.cube(cell, units(GROUND.cell()));
    s.point(center, linear_rgb(frame::focus::rgb));
    // First person the eye is the focus, and nothing about it is seen.
    let Some(azimuth) = heading else { return };
    let (sin, cos) = libm::sincos(azimuth);
    let (east, north, up) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let (right, forward) = ([cos, -sin, 0.0], [sin, cos, 0.0]);
    let tip = |d: V3| -> V3 { core::array::from_fn(|k| center[k] + d[k] * RING_RADIUS) };
    s.ring(center, east, north, (frame::level::rgb, frame::level::dashed));
    s.ring(center, right, up, (frame::forward::rgb, frame::forward::dashed));
    s.ring(center, north, up, (frame::north::rgb, frame::north::dashed));
    s.point(tip(forward), linear_rgb(frame::forward::rgb));
    s.point(tip(north), linear_rgb(frame::north::rgb));
    s.point(tip(up), linear_rgb(frame::up::rgb));
}

/// What is drawn, kept so a step allocates nothing, and what it was drawn
/// of, the focus and the heading; none while nothing is.
struct Drawn {
    sketch: Sketch,
    of: Option<(Q3, Option<f64>)>,
}

static GIZMO: Global<Drawn> = Global::new();

fn set_lines(at: V3, lines: &[f32]) {
    let Some((scene, draw)) = kernel().parts() else { return };
    scene.debug.set(draw, at, lines, lines.len());
}

/// `heading` is the camera's azimuth third person, none first person.
/// Sketched again only where the focus or the heading moved, or a change to
/// the ground touched the focus's column, whose floor it reads
/// (`changed::within`): a step each frame turn sees each change once, and
/// one hidden sketches afresh when shown.
pub fn frame_gizmo_step(show: bool, focus: Option<Q3>, heading: Option<f64>) {
    let Drawn { sketch: drawn, of } = GIZMO.get(|| Drawn {
        sketch: Sketch { about: [0.0; 3], lines: Vec::new() },
        of: None,
    });
    let Some(center) = focus.filter(|_| show) else {
        if of.take().is_some() {
            set_lines([0.0; 3], &[]);
        }
        return;
    };
    let at = to_world(&center);
    let column = [[at[0], at[1]]; 2];
    if *of == Some((center, heading)) && !crate::query::changed::within(column, &[]) {
        return;
    }
    sketch(drawn, &center, heading);
    set_lines(drawn.about, &drawn.lines);
    *of = Some((center, heading));
    request_world_tick();
}
