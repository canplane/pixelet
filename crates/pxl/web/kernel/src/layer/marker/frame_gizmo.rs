// pxl-kernel/layer/marker/frame_gizmo.rs

//! The debug gizmos (the `gizmos` setting): the cell the map center
//! floors to, the map center itself, the floating origin, the page's base and
//! center corners, and the two frames' gyro rings — one fixed to the screen
//! whatever the azimuth, one turning with the map so its vertex points north —
//! as line segments the kernel holds and the renderer draws over everything
//! (debug_lines.rs), with labels at points drawn as billboards: a line of
//! text a label in the one sheet (layer/billboard/sheet.rs), one set standing
//! where the lines do. The map's part is rebuilt when the map moves, the
//! frames' part when the view does; both go to the kernel as one set.

use pxl_render::asset::atlas::{DrawSpec, NO_SLOT};
use pxl_render::gpu::pass::badges::LABEL_FONT;
use pxl_render::gpu::surface::DPR_MAX;
use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::indexer::{L3_BASE, L3_CENTER};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::projection;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::v3::{floor3n, V3};
use pxl_spec::space::xy::TO_XY;

use crate::control::camera::orbit::orbit_view;
use crate::global::Global;
use crate::kernel::kernel;
use crate::layer::billboard::field::BILLBOARD_FLOATS;
use crate::layer::billboard::sheet::{billboard_sheet, new_billboard_set, place_billboard_set, remove_billboard_set, write_billboard_set, BillboardOrder};
use crate::text::text_width;
use crate::world::access::{Access, FINEST};
use crate::world::change::request_world_tick;
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

const COLOR: V3 = [0.0, 0.0, 0.0];
const DASH: f64 = 0.3;
const GAP: f64 = 0.2;
/// The most dashes a line is drawn in, said aloud: past this its dashes
/// lengthen rather than multiply. A line between the map center and the
/// floating origin reaches as far as the coarsest level the origin is floored
/// to, hundreds of thousands of units at a zoom out, and at a fixed dash that
/// was millions of them a frame — a block the heap refused, and a trap.
const DASHES: f64 = 256.0;
/// Half the cross a point is drawn as, in units.
const POINT_HALF: f64 = 0.15;
const RING_RADIUS: f64 = 4.0;
const RING_SEGMENTS: u32 = 64;
/// A label's height, as the view paints it (gpu/pass/badges.rs's LABEL_H); its
/// width is measured here in the font the view paints it in.
const LABEL_H: f64 = 12.0;
const LABEL_PAD_X: f64 = 2.0;

/// A label, at a point of the root frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub at: V3,
    pub text: String,
}

/// A sketch: segments as pairs of vertices (position and color), and labels at
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
        let period = (DASH + GAP).max(len / DASHES);
        let dash = period * DASH / (DASH + GAP);
        let mut s = 0.0;
        while s < len {
            let e = len.min(s + dash);
            self.line(
                [from[0] + d[0] * s, from[1] + d[1] * s, from[2] + d[2] * s],
                [from[0] + d[0] * e, from[1] + d[1] * e, from[2] + d[2] * e],
            );
            s += period;
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

    pub fn labeled_line(&mut self, from: V3, to: V3, label: Option<&str>, dashed: bool) {
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

    /// A dashed ring of `radius` about `center` in the plane of `a` and `b`.
    pub fn ring(&mut self, center: V3, radius: f64, a: V3, b: V3) {
        let mut prev: Option<V3> = None;
        for i in 0..=RING_SEGMENTS {
            let t = (i as f64 / RING_SEGMENTS as f64) * core::f64::consts::PI * 2.0;
            let (c, sn) = (t.cos() * radius, t.sin() * radius);
            let at = [center[0] + a[0] * c + b[0] * sn, center[1] + a[1] * c + b[1] * sn, center[2] + a[2] * c + b[2] * sn];
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
        let center = to_local([0.0, 0.0, 0.0]);
        let axis = |v: V3| {
            let p = to_local(v);
            [p[0] - center[0], p[1] - center[1], p[2] - center[2]]
        };
        self.ring(center, radius, axis([1.0, 0.0, 0.0]), axis([0.0, 1.0, 0.0]));
        self.ring(center, radius, axis(dir), axis([0.0, 0.0, 1.0]));
        if show_radius {
            self.labeled_line(center, to_local(vertex), Some(&format!("{radius}")), true);
        }
        self.point(to_local(vertex), Some(label));
    }

    pub fn append(&mut self, other: &Sketch) {
        self.vertices.extend_from_slice(&other.vertices);
        self.labels.extend_from_slice(&other.labels);
    }
}

/// The frames' rings about the orbit's pivot: the observer's heading, which
/// turns with the orbit, and the world's axes, which never turn.
fn sketch_frames(s: &mut Sketch) {
    let o = orbit_view();
    let (pivot, (sin, cos)) = (o.pivot, (-o.azimuth).sin_cos());
    let heading_frame = move |p: V3| [pivot[0] + p[0] * cos - p[1] * sin, pivot[1] + p[0] * sin + p[1] * cos, pivot[2] + p[2]];
    s.gyro(&heading_frame, [4.0, 0.0, 0.0], "heading (4,0,0)", false);
    s.point(heading_frame([0.0, 0.0, 4.0]), Some("heading (0,0,4)"));
    let world_frame = move |p: V3| [pivot[0] + p[0], pivot[1] + p[1], pivot[2] + p[2]];
    s.gyro(&world_frame, [0.0, RING_RADIUS, 0.0], "north (world)", true);
}

/// A point in the root frame: its address's offset from the map origin.
fn local_of(p: &Pvf3, map_origin: IVec) -> V3 {
    let local = ivec::sub(p.i, map_origin);
    let mut v = [0.0; 3];
    projection::unproject(&mut v, 0, p.f);
    [ivec::to_real(ivec::X(local)) + v[0], ivec::to_real(ivec::Y(local)) + v[1], ivec::to_real(ivec::Z(local)) + v[2]]
}

fn sketch_map(map_center: &Pvf3, map_origin: IVec) -> Sketch {
    let mut s = Sketch::default();

    // raw_v: the map center as placed, no elevation override. quant_v: the cell
    // it floors to, ground-snapped — a cell, so a box.
    let mut raw_v = [0.0; 3];
    projection::unproject(&mut raw_v, ivec::sub(map_center.i, map_origin), map_center.f);
    let ground = |at: &Pvf3| Access::now().at(at, FINEST).value().map(|g| g.top);
    let gnd_v = [raw_v[0], raw_v[1], ground(map_center).unwrap_or(0.0)];
    let mut quant_v = [0.0; 3];
    floor3n(&gnd_v, U_PER_CELL, &mut quant_v);

    // The floating origin, grounded and floored: local (0, 0, ground).
    let mut anchor_base_gnd_v = [0.0; 3];
    anchor_base_gnd_v[2] = ground(&Pvf3 { i: map_origin, f: 0 }).unwrap_or(0.0);
    let mut anchor_base_quant_v = [0.0; 3];
    floor3n(&anchor_base_gnd_v, U_PER_CELL, &mut anchor_base_quant_v);

    s.r#box(quant_v, U_PER_CELL, Some("floor(map_center)"));
    s.point(anchor_base_quant_v, Some("anchor_base (map_origin)"));
    s.labeled_line(quant_v, raw_v, None, false);
    s.labeled_line(quant_v, anchor_base_quant_v, Some("floor(map_center) <-> anchor_base (map_origin)"), true);

    for (corner, label) in [(TO_XY(L3_BASE(map_center.i)), "page_base"), (TO_XY(L3_CENTER(map_center.i)), "page_center")] {
        let at = Pvf3 { i: corner, f: 0 };
        if let Some(top) = ground(&at) {
            let mut grounded = at;
            pvf3::gather(&mut grounded, pvf3::X(&at), pvf3::Y(&at), pvf3::Z(&at) + pvf3::from_real(top));
            s.r#box(local_of(&grounded, map_origin), 1.0, Some(label));
        }
    }
    s.point(raw_v, Some("map_center"));
    s
}

struct Gizmos {
    /// What the drawn sketch was built from: the map center, the origin, the
    /// tiles' revision, and the view's frame.
    built_for: Option<(Pvf3, IVec, u32, [f64; 4])>,
    drawn: Sketch,
    shown: bool,
    /// The labels' billboard set, and each drawn label's slot of the sheet
    /// with its logical width, in the sketch's order; a label the sheet had
    /// no room for has NO_SLOT.
    set: Option<u32>,
    slots: Vec<(i32, f64)>,
    /// The sheet's generation the instances were written against: a repack
    /// moves every rectangle, and the instances are written again.
    written: Option<u32>,
}

static GIZMOS: Global<Gizmos> = Global::new();

fn gizmos() -> &'static mut Gizmos {
    GIZMOS.get(|| Gizmos {
        built_for: None,
        drawn: Sketch::default(),
        shown: false,
        set: None,
        slots: Vec::new(),
        written: None,
    })
}

/// Lets the labels' set and slots go.
fn drop_labels(g: &mut Gizmos) {
    if let Some(set) = g.set.take() {
        remove_billboard_set(set);
    }
    for (slot, _) in g.slots.drain(..) {
        if slot != NO_SLOT {
            billboard_sheet().release(slot as u32);
        }
    }
    g.written = None;
}

/// Paints the drawn sketch's labels into the sheet and stands their set at
/// `origin`; the instances follow on the next `write_labels`. Where a label
/// is not laid out yet, nothing is painted and the sketch is built again on a
/// later frame, when it is.
fn make_labels(g: &mut Gizmos, origin: V3) {
    drop_labels(g);
    if g.drawn.labels.is_empty() {
        return;
    }
    let widths: Vec<Option<f64>> = g.drawn.labels.iter().map(|l| text_width(LABEL_FONT, &l.text)).collect();
    if widths.contains(&None) {
        g.built_for = None;
        return;
    }
    let sheet = billboard_sheet();
    for (label, width) in g.drawn.labels.iter().zip(widths) {
        let logical_w = width.or_bug(Errno::ENOTRECOVERABLE).ceil() + LABEL_PAD_X * 2.0;
        let slot = sheet.add(logical_w * DPR_MAX, LABEL_H * DPR_MAX, DrawSpec::Text { text: label.text.clone(), logical_w });
        g.slots.push((slot, logical_w));
    }
    let set = new_billboard_set(BillboardOrder::Debug);
    place_billboard_set(set, origin);
    g.set = Some(set);
}

/// Writes the labels' instances, once per sheet generation: each label's
/// point, its logical size, its rectangle of the sheet, centered on the point.
fn write_labels(g: &mut Gizmos) {
    let Some(set) = g.set else { return };
    let sheet = billboard_sheet();
    let generation = sheet.sheet_generation();
    if g.written == Some(generation) {
        return;
    }
    g.written = Some(generation);
    let mut lane: Vec<f32> = Vec::with_capacity(g.slots.len() * BILLBOARD_FLOATS);
    for (label, &(slot, logical_w)) in g.drawn.labels.iter().zip(&g.slots) {
        if slot == NO_SLOT {
            continue;
        }
        let s = slot as u32;
        lane.extend_from_slice(&[
            label.at[0] as f32,
            label.at[1] as f32,
            label.at[2] as f32,
            logical_w as f32,
            LABEL_H as f32,
            sheet.u0(s) as f32,
            sheet.v0(s) as f32,
            sheet.u1(s) as f32,
            sheet.v1(s) as f32,
            0.5,
        ]);
    }
    write_billboard_set(set, &lane);
}

fn set_lines(at: V3, vertices: &[f32]) {
    let Some((_, sched)) = kernel().world() else { return };
    // SAFETY: the vertices are f32s of the kernel's memory, read before this returns.
    sched.debug.set(&mut sched.draw, at, vertices);
}

/// The gizmos' turn, while `show` is on: the set rebuilt when the map or the
/// view moved, and gone when `show` turns off.
pub fn debug_gizmos_step(show: bool, map_center: Option<Pvf3>, map_origin: IVec) {
    let g = gizmos();
    let Some(center) = map_center.filter(|_| show) else {
        if g.shown {
            set_lines([0.0; 3], &[]);
            drop_labels(g);
            g.shown = false;
            g.built_for = None;
            g.drawn = Sketch::default();
        }
        return;
    };
    let o = orbit_view();
    let key = (center, map_origin, Terrain::pyramid().revision(), [o.azimuth, o.pivot[0], o.pivot[1], o.pivot[2]]);
    if g.built_for == Some(key) {
        write_labels(g);
        return;
    }
    g.built_for = Some(key);
    let mut s = sketch_map(&center, map_origin);
    sketch_frames(&mut s);
    let origin = [ivec::to_real(ivec::X(map_origin)), ivec::to_real(ivec::Y(map_origin)), ivec::to_real(ivec::Z(map_origin))];
    set_lines(origin, &s.vertices);
    g.drawn = s;
    g.shown = true;
    make_labels(g, origin);
    write_labels(g);
    request_world_tick();
}
