// pxl-kernel/layer/marker/frame_gizmo.rs

//! The debug gizmos (the `gizmos` setting): the map center and its cell, the
//! page it stands in, the floating origin and the square it holds over, and
//! the view's and the world's frames, as debug lines. The view's forward, its up and the world's north are told
//! apart by color; nothing is labeled.

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::{floor3n, len3, V3};
use pxl_spec::space::pyramid::{anchor_span_of, LevelPage};

use crate::global::Global;
use crate::kernel::kernel;
use crate::world::access::Access;
use crate::world::change::request_world_tick;
use crate::world::frame::coord::canonical_to_local;
use crate::world::frame::view::{origin_level, view};
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;

const COLOR: V3 = [0.0, 0.0, 0.0];
const FORWARD: V3 = [1.0, 0.0, 0.0];
const NORTH: V3 = [0.0, 1.0, 0.0];
const UP: V3 = [0.0, 0.0, 1.0];
const DASH: f64 = 0.3;
const GAP: f64 = 0.2;
/// Past this a line's dashes lengthen rather than multiply: a line to the
/// floating origin can span hundreds of thousands of units, and a fixed dash
/// made millions a frame, a block the heap refused.
const DASHES: f64 = 256.0;
/// Half the cross a point is drawn as.
const POINT_HALF: f64 = 0.15;
const RING_RADIUS: f64 = 4.0;
const RING_SEGMENTS: u32 = 64;

/// Segments as pairs of vertices: position, then color.
#[derive(Default)]
struct Sketch(Vec<f32>);

impl Sketch {
    fn line(&mut self, from: V3, to: V3, color: V3) {
        for p in [from, to] {
            self.0.extend([p[0], p[1], p[2], color[0], color[1], color[2]].map(|v| v as f32));
        }
    }

    fn dashed(&mut self, from: V3, to: V3) {
        let len = len3(&[to[0] - from[0], to[1] - from[1], to[2] - from[2]]);
        if len == 0.0 {
            return;
        }
        let d = [(to[0] - from[0]) / len, (to[1] - from[1]) / len, (to[2] - from[2]) / len];
        let period = (DASH + GAP).max(len / DASHES);
        let dash = period * DASH / (DASH + GAP);
        let mut s = 0.0;
        while s < len {
            let e = len.min(s + dash);
            self.line(
                [from[0] + d[0] * s, from[1] + d[1] * s, from[2] + d[2] * s],
                [from[0] + d[0] * e, from[1] + d[1] * e, from[2] + d[2] * e],
                COLOR,
            );
            s += period;
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

    fn r#box(&mut self, base: V3, span: f64) {
        let c = |x: f64, y: f64, z: f64| -> V3 { [base[0] + x * span, base[1] + y * span, base[2] + z * span] };
        let corners = [
            c(0.0, 0.0, 0.0),
            c(1.0, 0.0, 0.0),
            c(1.0, 1.0, 0.0),
            c(0.0, 1.0, 0.0),
            c(0.0, 0.0, 1.0),
            c(1.0, 0.0, 1.0),
            c(1.0, 1.0, 1.0),
            c(0.0, 1.0, 1.0),
        ];
        for (a, b) in [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)] {
            self.line(corners[a], corners[b], COLOR);
        }
    }

    /// The square from `base` `side` across, level.
    fn square(&mut self, base: V3, side: f64, dashed: bool) {
        let c = [[0.0, 0.0], [side, 0.0], [side, side], [0.0, side]].map(|[x, y]| [base[0] + x, base[1] + y, base[2]]);
        for k in 0..4 {
            let (a, b) = (c[k], c[(k + 1) % 4]);
            if dashed {
                self.dashed(a, b);
            } else {
                self.line(a, b, COLOR);
            }
        }
    }

    /// A dashed ring of `radius` about `center` in the plane of `a` and `b`.
    fn ring(&mut self, center: V3, radius: f64, a: V3, b: V3, color: V3) {
        let mut prev: Option<V3> = None;
        for i in 0..=RING_SEGMENTS {
            let t = (i as f64 / RING_SEGMENTS as f64) * core::f64::consts::PI * 2.0;
            let (c, sn) = (libm::cos(t) * radius, libm::sin(t) * radius);
            let at = [center[0] + a[0] * c + b[0] * sn, center[1] + a[1] * c + b[1] * sn, center[2] + a[2] * c + b[2] * sn];
            if let Some(p) = prev.filter(|_| i % 2 == 1) {
                self.line(p, at, color);
            }
            prev = Some(at);
        }
    }

    /// The upright ring through `vertex`, in the frame `to_local` maps into the root frame.
    fn upright(&mut self, to_local: &dyn Fn(V3) -> V3, vertex: V3, color: V3) {
        let center = to_local([0.0, 0.0, 0.0]);
        let axis = |v: V3| {
            let p = to_local(v);
            [p[0] - center[0], p[1] - center[1], p[2] - center[2]]
        };
        let radius = len3(&vertex);
        self.ring(center, radius, axis(vertex.map(|v| v / radius)), axis([0.0, 0.0, 1.0]), color);
    }
}

/// About the map center: one level ring, which both frames share; the view's
/// frame turns with the azimuth, its upright ring through the right so it
/// faces the screen; the world's never turns, its upright ring through north.
fn sketch_frames(s: &mut Sketch) {
    if crate::control::camera::controller::dist() == 0.0 {
        return;
    }
    let Some(center) = crate::control::camera::controller::center_local() else { return };
    let (sin, cos) = libm::sincos(-crate::control::camera::controller::azimuth());
    let view_frame = move |p: V3| [center[0] + p[0] * cos - p[1] * sin, center[1] + p[0] * sin + p[1] * cos, center[2] + p[2]];
    s.ring(center, RING_RADIUS, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], COLOR);
    s.upright(&view_frame, [RING_RADIUS, 0.0, 0.0], FORWARD);
    s.point(view_frame([0.0, RING_RADIUS, 0.0]), FORWARD);
    s.point(view_frame([0.0, 0.0, RING_RADIUS]), UP);
    let world_frame = move |p: V3| [center[0] + p[0], center[1] + p[1], center[2] + p[2]];
    s.upright(&world_frame, [0.0, RING_RADIUS, 0.0], NORTH);
    s.point(world_frame([0.0, RING_RADIUS, 0.0]), NORTH);
}

/// The map center and its cell; the page it stands in, outlined; the origin,
/// and the square the eye stays in before the origin moves, dashed. Both
/// squares are bounds of coordinates, drawn level at the map center.
fn sketch_map(map_center: &Pvf3, map_origin: IVec) -> Sketch {
    let mut s = Sketch::default();
    let origin = ivec::to_v3(map_origin);
    let center_v = canonical_to_local(map_center, map_origin);
    let ground = Access::now().floor_under(map_center).value().map(|top| crate::world::frame::coord::local_height(top, map_origin));
    let mut cell_v = [0.0; 3];
    floor3n(&[center_v[0], center_v[1], ground.unwrap_or(0.0)], U_PER_CELL, &mut cell_v);
    s.r#box(cell_v, U_PER_CELL);
    s.point(center_v, COLOR);

    let z = center_v[2];
    let (x0, y0, side) = LevelPage::over(0, map_center.i).rect();
    s.square([x0 - origin[0], y0 - origin[1], z], side, false);

    let span = anchor_span_of(origin_level(view().eye_at())) as f64;
    s.square([0.0, 0.0, z], span, true);
    s.point([0.0, 0.0, z], COLOR);
    s
}

/// Map center, origin, its level, tiles' revision, and the azimuth the lines were drawn for.
type Drawn = (Pvf3, IVec, u32, u32, f64);

static GIZMOS: Global<Option<Drawn>> = Global::new();

fn set_lines(at: V3, vertices: &[f32]) {
    let Some((_, sched)) = kernel().world() else { return };
    sched.debug.set(&mut sched.draw, at, vertices);
}

pub fn debug_gizmos_step(show: bool, map_center: Option<Pvf3>, map_origin: IVec) {
    let built_for = GIZMOS.get(|| None);
    let Some(center) = map_center.filter(|_| show) else {
        if built_for.take().is_some() {
            set_lines([0.0; 3], &[]);
        }
        return;
    };
    let key = (center, map_origin, origin_level(view().eye_at()), Terrain::pyramid().paging().revision(), crate::control::camera::controller::azimuth());
    if *built_for == Some(key) {
        return;
    }
    *built_for = Some(key);
    let mut s = sketch_map(&center, map_origin);
    sketch_frames(&mut s);
    set_lines(ivec::to_v3(map_origin), &s.0);
    request_world_tick();
}
