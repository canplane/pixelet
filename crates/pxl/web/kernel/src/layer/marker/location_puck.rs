// pxl-kernel/layer/marker/location_puck.rs

//! The device's location puck: a zone of cells about the fix, each on the
//! ground under it, and a beacon above that eases, spins and bobs.

use pxl_render::frame::draw::{DrawList, PIPELINE_PUCK_BEACON, PIPELINE_PUCK_ZONE};
use pxl_render::gpu::api::word_bytes;
use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::pvf3::projection;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::{add3, center3n, floor3n, V3};
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::{alloc, free};

use crate::kernel::{kernel, with_world};
use crate::layer::environment::fog::view_reach;
use crate::layer::environment::theme::Theme;
use crate::layer::environment::theme_clock::theme_key;
use crate::world::access::Access;
use crate::world::change::request_world_tick;
use crate::world::chunk::table::Frame;
use crate::world::observation::heights_revision;
use xpute_runtime::global::Global;

// A world-space footprint (4u), independent of U_PER_CELL; the .707 is a
// one-cell diagonal correction.
const ZONE_RADIUS_CELL: f64 = 4.0 / U_PER_CELL - 0.707;
const ZONE_RADIUS2_CELL: f64 = ZONE_RADIUS_CELL * ZONE_RADIUS_CELL;

/// Which of its sides (top, bottom, right, left) lie on the zone's border.
struct ZoneSample {
    off: V3,
    borders: [f64; 4],
}

fn zone_samples() -> Vec<ZoneSample> {
    let range = ZONE_RADIUS_CELL.ceil() as i32;
    let flag = |b: bool| if b { 1.0 } else { 0.0 };
    let mut out = Vec::new();
    for x in -range..=range {
        for y in -range..=range {
            let (dx, dy) = (x as f64, y as f64);
            let (dx2, dy2) = (dx * dx, dy * dy);
            if dx2 + dy2 > ZONE_RADIUS2_CELL {
                continue;
            }
            let d_top = dx2 + (dy + 1.0) * (dy + 1.0);
            let d_bottom = dx2 + (dy - 1.0) * (dy - 1.0);
            let d_right = (dx + 1.0) * (dx + 1.0) + dy2;
            let d_left = (dx - 1.0) * (dx - 1.0) + dy2;
            out.push(ZoneSample {
                off: [dx * U_PER_CELL, dy * U_PER_CELL, -U_PER_CELL],
                borders: [
                    flag(d_top > ZONE_RADIUS2_CELL),
                    flag(d_bottom > ZONE_RADIUS2_CELL),
                    flag(d_right > ZONE_RADIUS2_CELL),
                    flag(d_left > ZONE_RADIUS2_CELL),
                ],
            });
        }
    }
    out
}

struct Puck {
    samples: Vec<ZoneSample>,
    /// Holds across a GPS dropout: rebuilding the zone on a blip caused a
    /// stacked-cube glitch after tab-away-and-back.
    last_known_device_coord: Option<Pvf3>,
    /// None until grounded, rather than drawing at a guessed height.
    last_known_center_z: Option<f64>,
    /// The zone's center and the `heights_revision` it was grounded against: a
    /// page arriving or an edit grounds it again.
    zone_for: Option<(V3, (u32, u32))>,
    beacon_for: Option<([f64; 2], bool)>,
    shown: bool,
}

static PUCK: Global<Puck> = Global::new();

pub fn location_puck_step(device_coord: Option<Pvf3>) {
    let p = PUCK.get(|| Puck {
        samples: zone_samples(),
        last_known_device_coord: None,
        last_known_center_z: None,
        zone_for: None,
        beacon_for: None,
        shown: false,
    });
    if device_coord.is_some() {
        p.last_known_device_coord = device_coord;
    }
    let Some(device) = p.last_known_device_coord else {
        return clear(p);
    };

    let pos = projection::unproject(device.i, device.f);
    // The fix names a place and not how high.
    if let Some(z) = Access::now().surface_support(&device).value() {
        p.last_known_center_z = Some(z);
    }
    let Some(center_z) = p.last_known_center_z else {
        return clear(p);
    };
    let raw = [pos[0], pos[1], center_z];
    let center_pos = floor3n(&raw, U_PER_CELL);

    let revision = heights_revision();
    if p.zone_for != Some((center_pos, revision)) {
        p.zone_for = Some((center_pos, revision));
        let mut lane: Vec<f32> = Vec::with_capacity(p.samples.len() * ZONE_INSTANCE_FLOATS);
        for s in &p.samples {
            let v = add3(&center_pos, &s.off);
            let coord = projection::project(v[0], v[1], v[2]);
            // An unloaded neighbor page falls back to the center's height; 0 would sink
            // or launch that instance.
            let delta_ele = Access::now().surface_support(&coord).value().unwrap_or(center_pos[2]) - center_pos[2];
            let lifted = [s.off[0], s.off[1], s.off[2] + delta_ele];
            let v = center3n(&lifted, U_PER_CELL);
            lane.extend([
                v[0] as f32,
                v[1] as f32,
                v[2] as f32,
                s.borders[0] as f32,
                s.borders[1] as f32,
                s.borders[2] as f32,
                s.borders[3] as f32,
            ]);
        }
        with_world(|_, sched| sched.markers.set_zone(&mut sched.draw, center_pos, &lane, ZONE_RADIUS_CELL * U_PER_CELL));
        p.shown = true;
        request_world_tick();
    }

    // The beacon adds back what the zone's cell flooring dropped, so it moves
    // smoothly rather than in whole-cell jumps.
    let beacon = ([pos[0] - center_pos[0], pos[1] - center_pos[1]], theme_key() == Theme::Night);
    if p.beacon_for != Some(beacon) {
        p.beacon_for = Some(beacon);
        kernel().sched.markers.set_beacon(beacon.0, beacon.1);
    }
}

fn clear(p: &mut Puck) {
    if !p.shown {
        return;
    }
    p.shown = false;
    p.zone_for = None;
    with_world(|_world, sched| sched.markers.clear(&mut sched.draw));
    request_world_tick();
}

/// The box's center relative to the zone's (3), and its border flags (4).
pub const ZONE_INSTANCE_FLOATS: usize = 7;
const BEACON_Z_BIAS: f64 = 0.5;
const BEACON_SMOOTH_RATE: f64 = 6.0;

pub struct PuckDraw {
    at: usize,
    bytes: u32,
    handle: u32,
    count: u32,
    center: V3,
    /// Relative to the center.
    target: [f64; 2],
    shown: Option<[f64; 2]>,
    snap_dist2: f64,
    night: bool,
    time: f64,
    visible: bool,
}

#[derive(Default)]
pub struct Markers {
    puck: Option<PuckDraw>,
}

impl Markers {
    /// `lane` holds ZONE_INSTANCE_FLOATS floats an instance. Replaces what was there.
    pub fn set_zone(&mut self, draw: &mut DrawList, center: V3, lane: &[f32], snap_dist: f64) {
        let count = (lane.len() / ZONE_INSTANCE_FLOATS) as u32;
        let bytes = word_bytes(lane.len()).max(4);
        let keep = self.puck.as_ref().is_some_and(|p| p.bytes == bytes);
        if !keep {
            self.clear(draw);
            let Some((at, bytes)) = alloc(bytes) else { return };
            self.puck = Some(PuckDraw {
                at,
                bytes,
                handle: 0,
                count,
                center,
                target: [0.0; 2],
                shown: None,
                snap_dist2: snap_dist * snap_dist,
                night: false,
                time: 0.0,
                visible: false,
            });
        }
        let p = self.puck.as_mut().unwrap();
        // Follows the center, so a whole-cell move does not play as a slide back.
        if let Some(s) = p.shown.as_mut() {
            s[0] -= center[0] - p.center[0];
            s[1] -= center[1] - p.center[1];
        }
        p.center = center;
        p.count = count;
        // SAFETY: the block is the heap's, `p.bytes` long, which is the lane's.
        unsafe { core::ptr::copy_nonoverlapping(lane.as_ptr().cast::<u8>(), ptr_at::<u8>(p.at), word_bytes(lane.len()) as usize) };
        if p.handle == 0 {
            p.handle = draw.upload(p.at as u32, p.bytes, 0, 0);
        } else {
            draw.rewrite(p.handle, p.at as u32, p.bytes);
        }
    }

    pub fn set_beacon(&mut self, target: [f64; 2], night: bool) {
        if let Some(p) = self.puck.as_mut() {
            p.target = target;
            p.night = night;
        }
    }

    pub fn clear(&mut self, draw: &mut DrawList) {
        if let Some(p) = self.puck.take() {
            draw.free(p.handle);
            // SAFETY: the block this marker took and has not given back.
            unsafe { free((p.at, p.bytes)) };
        }
    }

    /// Whether anything is animating and wants frames.
    pub fn tick(&mut self, frame: &Frame) -> bool {
        let Some(p) = self.puck.as_mut() else { return false };
        let dt = frame.delta_s;
        p.time += dt;
        let shown = p.shown.get_or_insert(p.target);
        let gap = [p.target[0] - shown[0], p.target[1] - shown[1]];
        if gap[0] * gap[0] + gap[1] * gap[1] > p.snap_dist2 {
            *shown = p.target;
        } else {
            let a = 1.0 - (-BEACON_SMOOTH_RATE * dt).exp();
            shown[0] += gap[0] * a;
            shown[1] += gap[1] * a;
        }
        // Hidden past the fog's far edge, as three's puck was.
        let [x, y, z] = frame.world_of([p.center[0] + shown[0], p.center[1] + shown[1], p.center[2] + BEACON_Z_BIAS]);
        let c = frame.camera;
        p.visible = (x - c[0]).powi(2) + (y - c[1]).powi(2) + (z - c[2]).powi(2) < view_reach().powi(2);
        p.visible
    }

    pub fn draw(&self, draw: &mut DrawList, eye: V3) {
        let Some(p) = &self.puck else { return };
        if !p.visible || p.handle == 0 {
            return;
        }
        let center = DrawList::translation(p.center, eye);
        draw.draw_as(PIPELINE_PUCK_ZONE, p.handle, p.count, 0, center, 0.0, 1.0);
        let shown = p.shown.unwrap_or(p.target);
        let beacon = [
            center[0] + shown[0] as f32,
            center[1] + shown[1] as f32,
            center[2] + (BEACON_Z_BIAS + (p.time * 2.0).sin() * 0.1) as f32,
        ];
        draw.draw_as(PIPELINE_PUCK_BEACON, 0, p.night as u32, 0, beacon, (p.time * 1.2) as f32, 1.0);
    }
}
