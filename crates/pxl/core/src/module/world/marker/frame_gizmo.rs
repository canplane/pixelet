// pxl_core/module/world/marker/frame_gizmo.rs

//! The dev gizmos (the `show_debug_objects` setting): the cell the map centre
//! floors to, the map centre itself, the floating origin, the page's base and
//! centre corners, and the two frames' gyro rings — one fixed to the screen
//! whatever the azimuth, one turning with the map so its vertex points north —
//! as line segments the kernel holds and the renderer draws over everything
//! (the second half), with labels at points the shell projects and shows as
//! DOM over the canvas. The map's part is rebuilt when
//! the map moves, the frames' part when the view does; both go to the kernel
//! as one set.

use pxl_shared::bake::constant::U_PER_CELL;
use pxl_shared::spec::space::indexer::{L1_IDX_IN_L4, L3_BASE, L3_CENTER};
use pxl_shared::spec::space::pvf3::ivec::{self, IVec};
use pxl_shared::spec::space::pvf3::projection;
use pxl_shared::spec::space::pvf3::pvf3::PVF3;
use pxl_shared::spec::space::v3::{floor3n, V3};
use pxl_shared::spec::space::xy::TO_XY;
use xpute_core::kit::mat4::{mat4_transform_point, Mat4, Vec3};

use crate::module::world::change::request_world_tick;
use crate::module::world::chunk_node::World;
use crate::module::world::frame::view::view;
use crate::module::world::terrain::elevation::{get_ele, with_ele};
use crate::module::world::terrain::store::tile_pages;
use crate::render::draw::{DrawList, Held, PIPELINE_DEBUG};
use crate::sys::global::Global;
use crate::sys::kernel::kernel;

const COLOR: V3 = [0.0, 0.0, 0.0];
const DASH: f64 = 0.3;
const GAP: f64 = 0.2;
/// Half the cross a point is drawn as, in units.
const POINT_HALF: f64 = 0.15;
const RING_RADIUS: f64 = 4.0;
const RING_SEGMENTS: u32 = 64;

/// A label, at a point of the root frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub at: V3,
    pub text: String,
}

/// A sketch: segments as pairs of vertices (position and colour), and labels at
/// points.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sketch {
    pub vertices: Vec<f32>,
    pub labels: Vec<Label>,
}

fn hypot3(v: V3) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

impl Sketch {
    pub fn line(&mut self, from: V3, to: V3) {
        for p in [from, to] {
            self.vertices.extend([p[0] as f32, p[1] as f32, p[2] as f32, COLOR[0] as f32, COLOR[1] as f32, COLOR[2] as f32]);
        }
    }

    pub fn dashed(&mut self, from: V3, to: V3) {
        let len = hypot3([to[0] - from[0], to[1] - from[1], to[2] - from[2]]);
        if len == 0.0 {
            return;
        }
        let d = [(to[0] - from[0]) / len, (to[1] - from[1]) / len, (to[2] - from[2]) / len];
        let mut s = 0.0;
        while s < len {
            let e = len.min(s + DASH);
            self.line([from[0] + d[0] * s, from[1] + d[1] * s, from[2] + d[2] * s], [from[0] + d[0] * e, from[1] + d[1] * e, from[2] + d[2] * e]);
            s += DASH + GAP;
        }
    }

    pub fn point(&mut self, at: V3, label: Option<&str>) {
        for axis in 0..3 {
            let (mut a, mut b) = (at, at);
            a[axis] -= POINT_HALF;
            b[axis] += POINT_HALF;
            self.line(a, b);
        }
        if let Some(label) = label {
            self.labels.push(Label { at, text: label.to_string() });
        }
    }

    pub fn r#box(&mut self, base: V3, span: f64, label: Option<&str>) {
        let c = |x: f64, y: f64, z: f64| -> V3 { [base[0] + x * span, base[1] + y * span, base[2] + z * span] };
        let corners = [c(0.0, 0.0, 0.0), c(1.0, 0.0, 0.0), c(1.0, 1.0, 0.0), c(0.0, 1.0, 0.0), c(0.0, 0.0, 1.0), c(1.0, 0.0, 1.0), c(1.0, 1.0, 1.0), c(0.0, 1.0, 1.0)];
        for (a, b) in [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)] {
            self.line(corners[a], corners[b]);
        }
        self.labels.push(Label {
            at: c(0.5, 0.0, 0.0),
            text: format!("{span}"),
        });
        if let Some(label) = label {
            self.labels.push(Label {
                at: c(0.5, 0.5, 0.5),
                text: label.to_string(),
            });
        }
    }

    pub fn labelled_line(&mut self, from: V3, to: V3, label: Option<&str>, dashed: bool) {
        if dashed {
            self.dashed(from, to);
        } else {
            self.line(from, to);
        }
        if let Some(label) = label {
            self.labels.push(Label {
                at: [(from[0] + to[0]) / 2.0, (from[1] + to[1]) / 2.0, (from[2] + to[2]) / 2.0],
                text: label.to_string(),
            });
        }
    }

    /// A dashed ring of `radius` about `centre` in the plane of `a` and `b`.
    pub fn ring(&mut self, centre: V3, radius: f64, a: V3, b: V3) {
        let mut prev: Option<V3> = None;
        for i in 0..=RING_SEGMENTS {
            let t = (i as f64 / RING_SEGMENTS as f64) * core::f64::consts::PI * 2.0;
            let (c, sn) = (t.cos() * radius, t.sin() * radius);
            let at = [centre[0] + a[0] * c + b[0] * sn, centre[1] + a[1] * c + b[1] * sn, centre[2] + a[2] * c + b[2] * sn];
            if let Some(p) = prev.filter(|_| i % 2 == 1) {
                self.line(p, at);
            }
            prev = Some(at);
        }
    }

    /// A gyro ring: a horizontal ring and a vertical one through `vertex`, the
    /// vertex marked, and the radius drawn when asked, all in the frame
    /// `to_local` maps into the root frame.
    pub fn gyro(&mut self, to_local: &dyn Fn(V3) -> V3, vertex: V3, label: &str, show_radius: bool) {
        let horiz = (vertex[0] * vertex[0] + vertex[1] * vertex[1]).sqrt();
        let dir = if horiz > 1e-9 { [vertex[0] / horiz, vertex[1] / horiz, 0.0] } else { [1.0, 0.0, 0.0] };
        let radius = hypot3(vertex);
        let centre = to_local([0.0, 0.0, 0.0]);
        let axis = |v: V3| {
            let p = to_local(v);
            [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]]
        };
        self.ring(centre, radius, axis([1.0, 0.0, 0.0]), axis([0.0, 1.0, 0.0]));
        self.ring(centre, radius, axis(dir), axis([0.0, 0.0, 1.0]));
        if show_radius {
            self.labelled_line(centre, to_local(vertex), Some(&format!("{radius}")), true);
        }
        self.point(to_local(vertex), Some(label));
    }

    pub fn append(&mut self, other: &Sketch) {
        self.vertices.extend_from_slice(&other.vertices);
        self.labels.extend_from_slice(&other.labels);
    }
}

/// A point of a matrix's frame, in the root frame.
fn through(m: Mat4) -> impl Fn(V3) -> V3 {
    move |v: V3| {
        let mut out = Vec3::default();
        mat4_transform_point(&mut out, &m, v[0], v[1], v[2]);
        [out.x, out.y, out.z]
    }
}

/// The frames' rings: the view frame, which never turns, and the world frame,
/// which turns with the map, both about the view's origin.
fn sketch_frames(s: &mut Sketch) {
    let v = view();
    let view_frame = through(v.inv_root);
    s.gyro(&view_frame, [4.0, 0.0, 0.0], "view-frame (4,0,0)", false);
    s.point(view_frame([0.0, 0.0, 4.0]), Some("view-frame (0,0,4)"));
    let (tx, ty, tz) = (v.tx, v.ty, v.tz);
    let world_frame = move |p: V3| [p[0] - tx, p[1] - ty, p[2] - tz];
    s.gyro(&world_frame, [0.0, RING_RADIUS, 0.0], "north (world-frame, azimuth-locked)", true);
}

/// A point in the root frame: its address's offset from the map origin.
fn local_of(p: &PVF3, map_origin: IVec) -> V3 {
    let local = ivec::sub(p.i, map_origin);
    let mut v = [0.0; 3];
    projection::unproject(&mut v, 0, p.f);
    [ivec::to_real(ivec::X(local)) + v[0], ivec::to_real(ivec::Y(local)) + v[1], ivec::to_real(ivec::Z(local)) + v[2]]
}

fn sketch_map(map_center: &PVF3, map_origin: IVec) -> Sketch {
    let mut s = Sketch::default();

    // raw_v: the map centre as placed, no elevation override. quant_v: the cell
    // it floors to, ground-snapped — a cell, so a box.
    let mut raw_v = [0.0; 3];
    projection::unproject(&mut raw_v, L1_IDX_IN_L4(map_center.i), map_center.f);
    let gnd_v = [raw_v[0], raw_v[1], get_ele(map_center).unwrap_or(0.0)];
    let mut quant_v = [0.0; 3];
    floor3n(&gnd_v, U_PER_CELL, &mut quant_v);

    // The floating origin, grounded and floored: local (0, 0, ground).
    let mut anchor_base_gnd_v = [0.0; 3];
    projection::unproject(&mut anchor_base_gnd_v, L1_IDX_IN_L4(map_origin), 0);
    anchor_base_gnd_v[2] = get_ele(&PVF3 { i: map_origin, f: 0 }).unwrap_or(0.0);
    let mut anchor_base_quant_v = [0.0; 3];
    floor3n(&anchor_base_gnd_v, U_PER_CELL, &mut anchor_base_quant_v);

    s.r#box(quant_v, U_PER_CELL, Some("floor(map_center)"));
    s.point(anchor_base_quant_v, Some("anchor_base (map_origin)"));
    s.labelled_line(quant_v, raw_v, None, false);
    s.labelled_line(quant_v, anchor_base_quant_v, Some("floor(map_center) <-> anchor_base (map_origin)"), true);

    for (corner, label) in [(TO_XY(L3_BASE(map_center.i)), "page_base"), (TO_XY(L3_CENTER(map_center.i)), "page_center")] {
        let mut out = PVF3::default();
        if let Some(grounded) = with_ele(&mut out, &PVF3 { i: corner, f: 0 }) {
            s.r#box(local_of(&grounded, map_origin), 1.0, Some(label));
        }
    }
    s.point(raw_v, Some("map_center"));
    s
}

struct Gizmos {
    /// What the drawn sketch was built from: the map centre, the origin, the
    /// tiles' revision, and the view's frame.
    built_for: Option<(PVF3, IVec, u32, [f64; 4])>,
    drawn: Sketch,
    shown: bool,
}

static GIZMOS: Global<Gizmos> = Global::new();

fn set_lines(at: V3, vertices: &[f32]) {
    let k = kernel();
    let Some(volumes) = k.volumes.as_mut() else { return };
    let mut world = World {
        volumes,
        materials: &k.terrain,
        store: &k.objects,
        painter: &mut k.painter,
    };
    let sched = &mut k.sched;
    // SAFETY: the vertices are f32s of the kernel's memory, read before this returns.
    unsafe { sched.debug.set(&mut world, &mut sched.draw, at, (vertices.len() / 6) as u32, vertices.as_ptr() as usize) };
}

/// The gizmos' turn, while `show` is on: the set rebuilt when the map or the
/// view moved, and gone when `show` turns off.
pub fn debug_gizmos_step(show: bool, map_center: Option<PVF3>, map_origin: IVec) {
    let g = GIZMOS.get(|| Gizmos {
        built_for: None,
        drawn: Sketch::default(),
        shown: false,
    });
    let Some(center) = map_center.filter(|_| show) else {
        if g.shown {
            set_lines([0.0; 3], &[]);
            g.shown = false;
            g.built_for = None;
            g.drawn = Sketch::default();
        }
        return;
    };
    let v = view();
    let key = (center, map_origin, tile_pages().revision(), [v.azimuth, v.tx, v.ty, v.tz]);
    if g.built_for == Some(key) {
        return;
    }
    g.built_for = Some(key);
    let mut s = sketch_map(&center, map_origin);
    sketch_frames(&mut s);
    set_lines([ivec::to_real(ivec::X(map_origin)), ivec::to_real(ivec::Y(map_origin)), ivec::to_real(ivec::Z(map_origin))], &s.vertices);
    g.drawn = s;
    g.shown = true;
    request_world_tick();
}

/// The labels of the drawn sketch, for the shell to project each frame.
pub fn debug_labels() -> &'static [Label] {
    GIZMOS
        .get(|| Gizmos {
            built_for: None,
            drawn: Sketch::default(),
            shown: false,
        })
        .drawn
        .labels
        .as_slice()
}

// ---- the gizmos as the kernel holds them ----
//
// Line segments over the world, drawn on top of everything — the frame's
// origins, page corners and the map centre. The segments are
// written whenever what they mark moves; this holds them as a block and
// lists their draw each frame, placed against the frame's origin.

/// Floats a vertex takes: its position relative to the set's origin (3)
/// and its colour, linear (3). Two vertices a segment.
pub const DEBUG_VERTEX_FLOATS: usize = 6;

#[derive(Default)]
pub struct DebugLines {
    held: Held,
    vertex_cnt: u32,
    origin: V3,
}

impl DebugLines {
    /// Replaces the segments with `vertex_cnt` vertices from `lane`,
    /// relative to `origin` in units.
    ///
    /// # Safety
    /// `lane` addresses vertex_cnt * DEBUG_VERTEX_FLOATS f32s in the memory.
    pub unsafe fn set(&mut self, world: &mut World, draw: &mut DrawList, origin: V3, vertex_cnt: u32, lane: usize) {
        let len = (vertex_cnt as usize * DEBUG_VERTEX_FLOATS * 4) as u32;
        let written = self.held.write(draw, world.volumes.mem(), lane, len);
        self.vertex_cnt = if written { vertex_cnt } else { 0 };
        self.origin = origin;
    }

    pub fn draw(&self, draw: &mut DrawList, origin: V3) {
        if self.vertex_cnt == 0 || self.held.handle == 0 {
            return;
        }
        let t = DrawList::translation(self.origin, origin);
        draw.draw_as(PIPELINE_DEBUG, self.held.handle, self.vertex_cnt, 0, t, 0.0, 1.0);
    }
}
