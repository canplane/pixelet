// pxl-core/domain/scene/gizmo/frame_gizmo.rs

//! The frame gizmo, shown with the `world_gizmo` setting and drawn as debug
//! lines. It marks the pivot and the ground cell under it. Around the pivot
//! it draws a level ring, an upright ring facing the screen and an upright
//! ring through north. The view's forward, its up and north are told apart by
//! the spec's colors (`gizmo_spec::frame`), which the UI's legend reads too.

use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;

use crate::domain::query::World;
use crate::domain::scene::gizmo::debug_line::DebugLines;
use crate::domain::scene::gizmo::gizmo_spec::frame;
use crate::domain::scene::model::draw_list;
use crate::domain::view::frame::coord::to_world;
use crate::sys::sched::edge::request_world_tick;
use pxl_render::gpu::draw::DrawList;
use pxl_render::look::color::linear_rgb;
use pxl_spec::space::layer::quadrant::LEAF_ZOOM;
use xpute_core::global::Global;

/// Half the length of each arm of the cross a point is drawn as.
const POINT_HALF: f64 = 0.15;
const RING_RADIUS: f64 = 4.0;
const RING_SEGMENTS: u32 = 64;

/// Segments as pairs of vertices, each a position relative to `about` and
/// then a color. A sketch has no faces: a ring is only an outline, and a
/// cube's faces would hide the pivot inside it.
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

    /// Draws the cube's twelve edges. A corner's index holds one bit per
    /// axis, so an edge joins two corners whose indices differ in one bit.
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

    /// Draws a ring of `RING_RADIUS` about `center`, in the plane of the unit
    /// axes `a` and `b`.
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

/// Sketches the gizmo for `pivot` into `s`, replacing what `s` held.
fn sketch(s: &mut Sketch, pivot: &Q3, heading: Option<f64>) {
    let at = to_world(pivot);
    s.about = at;
    s.lines.clear();
    let ground = World::now().floor_under(pivot, LEAF_ZOOM).val().flatten();
    let cell = v3::floor(
        &[at[0], at[1], ground.unwrap_or(0.0)],
        units(GROUND.leaf_cell_zoom()),
    );
    s.cube(cell, units(GROUND.leaf_cell_zoom()));
    s.point(at, linear_rgb(frame::pivot::rgb));
    // In first person the eye is at the pivot, so nothing drawn about it
    // would be seen.
    let Some(azimuth) = heading else { return };
    let (sin, cos) = libm::sincos(azimuth);
    let (east, north, up) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let (right, forward) = ([cos, -sin, 0.0], [sin, cos, 0.0]);
    let tip = |d: V3| -> V3 { core::array::from_fn(|k| at[k] + d[k] * RING_RADIUS) };
    s.ring(at, east, north, (frame::level::rgb, frame::level::dashed));
    s.ring(at, right, up, (frame::forward::rgb, frame::forward::dashed));
    s.ring(at, north, up, (frame::north::rgb, frame::north::dashed));
    s.point(tip(forward), linear_rgb(frame::forward::rgb));
    s.point(tip(north), linear_rgb(frame::north::rgb));
    s.point(tip(up), linear_rgb(frame::up::rgb));
}

struct Gizmo {
    /// The sketch now drawn, and whether the gizmo is shown at all.
    drawn: Sketch,
    shown: bool,
    /// The next sketch. It is kept between steps so a step allocates nothing.
    next: Sketch,
    lines: DebugLines,
}

static GIZMO: Global<Gizmo> = Global::new();

fn gizmo() -> &'static mut Gizmo {
    let empty = || Sketch {
        about: [0.0; 3],
        lines: Vec::new(),
    };
    GIZMO.get(|| Gizmo {
        drawn: empty(),
        shown: false,
        next: empty(),
        lines: DebugLines::default(),
    })
}

pub fn frame_gizmo_draw(draw: &mut DrawList) {
    gizmo().lines.draw(draw);
}

/// `heading` is the camera's azimuth in third person, and None in first
/// person. The gizmo is sketched every step but handed to the renderer only
/// when the sketch changed, so the floor under the pivot is drawn in its
/// place as soon as it is known. It must run before the frame's list begins
/// (domain/scene/stage.rs), so the lines and the origin they are drawn about
/// come from the same turn.
pub fn frame_gizmo_step(show: bool, pivot: Option<Q3>, heading: Option<f64>) {
    let g = gizmo();
    let Some(at) = pivot.filter(|_| show) else {
        if core::mem::take(&mut g.shown) {
            g.lines.set(draw_list(), [0.0; 3], &[], 0);
        }
        return;
    };
    sketch(&mut g.next, &at, heading);
    if g.shown && g.drawn == g.next {
        return;
    }
    g.lines
        .set(draw_list(), g.next.about, &g.next.lines, g.next.lines.len());
    core::mem::swap(&mut g.drawn, &mut g.next);
    g.shown = true;
    request_world_tick();
}
