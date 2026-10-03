// pxl-kernel/layer/marker/frame_gizmo.rs

//! The debug gizmos (the `gizmos` setting): the map center's cell, the
//! floating origin, the page's corners and the frames' gyro rings, as debug
//! lines with billboard labels.

use pxl_render::asset::atlas::{DrawSpec, NO_SLOT};
use pxl_render::gpu::pass::badges::{LABEL_FONT, LABEL_H};
use pxl_render::gpu::surface::DPR_MAX;
use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::indexer::{L3_BASE, L3_CENTER};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::v3::{floor3n, len3, V3};
use pxl_spec::space::xy::TO_XY;

use crate::global::Global;
use crate::kernel::kernel;
use crate::layer::billboard::field::billboard_instance;
use crate::layer::billboard::field::BILLBOARD_FLOATS;
use crate::layer::billboard::sheet::{billboard_sheet, new_billboard_set, place_billboard_set, remove_billboard_set, write_billboard_set, BillboardOrder};
use crate::text::text_width;
use crate::world::access::Access;
use crate::world::frame::coord::canonical_to_local;
use crate::world::change::request_world_tick;
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;

const COLOR: V3 = [0.0, 0.0, 0.0];
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
const LABEL_PAD_X: f64 = 2.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub at: V3,
    pub text: String,
}

/// Segments as pairs of vertices (position, color), and labels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sketch {
    pub vertices: Vec<f32>,
    pub labels: Vec<Label>,
}

impl Sketch {
    pub fn line(&mut self, from: V3, to: V3) {
        for p in [from, to] {
            self.vertices.extend([p[0] as f32, p[1] as f32, p[2] as f32, COLOR[0] as f32, COLOR[1] as f32, COLOR[2] as f32]);
        }
    }

    pub fn dashed(&mut self, from: V3, to: V3) {
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
        if let Some(label) = label {
            self.labels.push(Label {
                at: c(0.5, 0.5, 0.5),
                text: label.to_string(),
            });
        }
    }

    /// A dashed ring of `radius` about `center` in the plane of `a` and `b`.
    pub fn ring(&mut self, center: V3, radius: f64, a: V3, b: V3) {
        let mut prev: Option<V3> = None;
        for i in 0..=RING_SEGMENTS {
            let t = (i as f64 / RING_SEGMENTS as f64) * core::f64::consts::PI * 2.0;
            let (c, sn) = (libm::cos(t) * radius, libm::sin(t) * radius);
            let at = [center[0] + a[0] * c + b[0] * sn, center[1] + a[1] * c + b[1] * sn, center[2] + a[2] * c + b[2] * sn];
            if let Some(p) = prev.filter(|_| i % 2 == 1) {
                self.line(p, at);
            }
            prev = Some(at);
        }
    }

    /// In the frame `to_local` maps into the root frame.
    pub fn gyro(&mut self, to_local: &dyn Fn(V3) -> V3, vertex: V3, label: &str) {
        let horiz = (vertex[0] * vertex[0] + vertex[1] * vertex[1]).sqrt();
        let dir = if horiz > 1e-9 { [vertex[0] / horiz, vertex[1] / horiz, 0.0] } else { [1.0, 0.0, 0.0] };
        let radius = len3(&vertex);
        let center = to_local([0.0, 0.0, 0.0]);
        let axis = |v: V3| {
            let p = to_local(v);
            [p[0] - center[0], p[1] - center[1], p[2] - center[2]]
        };
        self.ring(center, radius, axis([1.0, 0.0, 0.0]), axis([0.0, 1.0, 0.0]));
        self.ring(center, radius, axis(dir), axis([0.0, 0.0, 1.0]));
        self.point(to_local(vertex), Some(label));
    }
}

/// About the map center: the heading frame turns with the orbit, the world's never.
fn sketch_frames(s: &mut Sketch) {
    if crate::control::camera::controller::radius() == 0.0 {
        return;
    }
    let Some(center) = crate::control::camera::controller::center_local() else { return };
    let (sin, cos) = libm::sincos(-crate::control::camera::controller::heading());
    let heading_frame = move |p: V3| [center[0] + p[0] * cos - p[1] * sin, center[1] + p[0] * sin + p[1] * cos, center[2] + p[2]];
    s.gyro(&heading_frame, [RING_RADIUS, 0.0, 0.0], "heading");
    s.point(heading_frame([0.0, 0.0, RING_RADIUS]), Some("up"));
    let world_frame = move |p: V3| [center[0] + p[0], center[1] + p[1], center[2] + p[2]];
    s.gyro(&world_frame, [0.0, RING_RADIUS, 0.0], "north");
}

fn sketch_map(map_center: &Pvf3, map_origin: IVec) -> Sketch {
    let mut s = Sketch::default();

    let raw_v = canonical_to_local(map_center, map_origin);
    let ground = |at: &Pvf3| Access::now().floor_under(at).value().map(|top| crate::world::frame::coord::local_height(top, map_origin));
    let gnd_v = [raw_v[0], raw_v[1], ground(map_center).unwrap_or(0.0)];
    let mut quant_v = [0.0; 3];
    floor3n(&gnd_v, U_PER_CELL, &mut quant_v);

    // The origin, on the ground under it.
    let mut origin_v = [0.0; 3];
    floor3n(&[0.0, 0.0, ground(&Pvf3 { i: map_origin, f: 0 }).unwrap_or(0.0)], U_PER_CELL, &mut origin_v);

    s.r#box(quant_v, U_PER_CELL, Some("center cell"));
    s.point(origin_v, Some("origin"));
    s.line(quant_v, raw_v);
    s.dashed(quant_v, origin_v);

    for (corner, label) in [(TO_XY(L3_BASE(map_center.i)), "page base"), (TO_XY(L3_CENTER(map_center.i)), "page center")] {
        let at = Pvf3 { i: corner, f: 0 };
        if let Some(top) = ground(&at) {
            let mut grounded = at;
            pvf3::gather(&mut grounded, pvf3::X(&at), pvf3::Y(&at), pvf3::Z(&at) + pvf3::from_real(top));
            s.r#box(canonical_to_local(&grounded, map_origin), 1.0, Some(label));
        }
    }
    s.point(raw_v, Some("map center"));
    s
}

struct Gizmos {
    /// Map center, origin, tiles' revision, and the view's yaw.
    built_for: Option<(Pvf3, IVec, u32, f64)>,
    drawn: Sketch,
    shown: bool,
    set: Option<u32>,
    /// Each label's text, its sheet slot and width, in the labels' order. A
    /// slot is kept while its text is, since a new one shows nothing until its
    /// text is drawn, and a label the sheet had no room for has NO_SLOT.
    slots: Vec<(String, i32, f64)>,
    /// A repack moves every rectangle, so instances are rewritten per generation.
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

fn drop_labels(g: &mut Gizmos) {
    if let Some(set) = g.set.take() {
        remove_billboard_set(set);
    }
    for (_, slot, _) in g.slots.drain(..) {
        if slot != NO_SLOT {
            billboard_sheet().release(slot as u32);
        }
    }
    g.written = None;
}

/// Where a label is not laid out yet, the sketch is rebuilt on a later frame.
fn make_labels(g: &mut Gizmos, origin: V3) {
    let sheet = billboard_sheet();
    let mut held = core::mem::take(&mut g.slots);
    for label in &g.drawn.labels {
        if let Some(k) = held.iter().position(|h| h.0 == label.text && h.1 != NO_SLOT) {
            g.slots.push(held.swap_remove(k));
            continue;
        }
        let Some(width) = text_width(LABEL_FONT, &label.text) else {
            g.built_for = None;
            g.slots.push((label.text.clone(), NO_SLOT, 0.0));
            continue;
        };
        let logical_w = width.ceil() + LABEL_PAD_X * 2.0;
        let slot = sheet.add(logical_w * DPR_MAX, f64::from(LABEL_H) * DPR_MAX, DrawSpec::Text { text: label.text.clone(), logical_w });
        g.slots.push((label.text.clone(), slot, logical_w));
    }
    for (_, slot, _) in held {
        if slot != NO_SLOT {
            sheet.release(slot as u32);
        }
    }
    let set = *g.set.get_or_insert_with(|| new_billboard_set(BillboardOrder::Debug));
    place_billboard_set(set, origin);
    g.written = None;
}

fn write_labels(g: &mut Gizmos) {
    let Some(set) = g.set else { return };
    let sheet = billboard_sheet();
    let generation = sheet.sheet_generation();
    if g.written == Some(generation) {
        return;
    }
    g.written = Some(generation);
    let mut lane: Vec<f32> = Vec::with_capacity(g.slots.len() * BILLBOARD_FLOATS);
    for (label, (_, slot, logical_w)) in g.drawn.labels.iter().zip(&g.slots) {
        if *slot == NO_SLOT {
            continue;
        }
        lane.extend_from_slice(&billboard_instance(label.at, [*logical_w, f64::from(LABEL_H)], *slot as u32, 0.5));
    }
    write_billboard_set(set, &lane);
}

fn set_lines(at: V3, vertices: &[f32]) {
    let Some((_, sched)) = kernel().world() else { return };
    sched.debug.set(&mut sched.draw, at, vertices);
}

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
    let key = (center, map_origin, Terrain::pyramid().paging().revision(), crate::control::camera::controller::heading());
    if g.built_for == Some(key) {
        write_labels(g);
        return;
    }
    g.built_for = Some(key);
    let mut s = sketch_map(&center, map_origin);
    sketch_frames(&mut s);
    let origin = ivec::to_v3(map_origin);
    set_lines(origin, &s.vertices);
    g.drawn = s;
    g.shown = true;
    make_labels(g, origin);
    write_labels(g);
    request_world_tick();
}
