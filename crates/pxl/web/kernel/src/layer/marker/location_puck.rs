// pxl-kernel/layer/marker/location_puck.rs

//! The device's location puck: a zone of cells about where the device is, each
//! at the ground under it, and a beacon above. The first half decides where
//! the device is, which cells the zone covers at what height and where the
//! beacon should stand; the second holds the zone, eases, spins and bobs the
//! beacon, and draws both.

use pxl_render::frame::draw::{DrawList, PIPELINE_PUCK_BEACON, PIPELINE_PUCK_ZONE};
use pxl_render::gpu::api::word_bytes;
use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::pvf3::projection;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::{add3, center3n, floor3n, V3};
use xpute_runtime::mem::heap::{alloc, free};

use crate::global::Global;
use crate::kernel::{kernel, with_world};
use crate::layer::environment::fog::view_reach;
use crate::layer::environment::theme::Theme;
use crate::layer::environment::theme_clock::theme_key;
use crate::layer::terrain::elevation::get_ele;
use crate::world::change::request_world_tick;
use crate::world::chunk::node::World;
use crate::world::chunk::table::Frame;
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;

// The numerator (4) is a world-space distance, not a cell count — it fixes the
// zone's footprint size independent of U_PER_CELL. The .707 shave is a fixed
// one-cell diagonal correction in cell units, so it does not scale either.
const ZONE_RADIUS_CELL: f64 = 4.0 / U_PER_CELL - 0.707;
const ZONE_RADIUS2_CELL: f64 = ZONE_RADIUS_CELL * ZONE_RADIUS_CELL;

/// A zone cell: its offset from the centre, and which of its sides (top,
/// bottom, right, left) lie on the zone's border.
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
    /// Holds the last known device position across a transient GPS dropout.
    /// Without this, a blip tore down and rebuilt the zone — that, not the
    /// position math, was the source of a "stacked/cube" rendering glitch after
    /// tab-away-and-back.
    last_known_device_coord: Option<Pvf3>,
    /// Not yet grounded gates drawing, instead of drawing the marker at a
    /// guessed height while the device's own page isn't loaded yet.
    last_known_center_z: Option<f64>,
    /// The zone as last written: its centre, and the tiles it was grounded
    /// against. A zone sample can reach a page not yet loaded; a page arriving
    /// grounds it again.
    zone_for: Option<(V3, u32)>,
    beacon_for: Option<([f64; 2], bool)>,
    shown: bool,
}

static PUCK: Global<Puck> = Global::new();

/// The puck's turn, for the device's coordinate as it stands.
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

    let mut pos = [0.0; 3];
    projection::unproject(&mut pos, device.i, device.f);
    if let Some(ele) = get_ele(&device) {
        p.last_known_center_z = Some(ele);
    }
    let Some(center_z) = p.last_known_center_z else {
        return clear(p);
    };
    let raw = [pos[0], pos[1], center_z];
    let mut center_pos = [0.0; 3];
    floor3n(&raw, U_PER_CELL, &mut center_pos);

    let tiles = Terrain::pyramid().level(0).revision();
    if p.zone_for != Some((center_pos, tiles)) {
        p.zone_for = Some((center_pos, tiles));
        let mut lane: Vec<f32> = Vec::with_capacity(p.samples.len() * 7);
        let mut v = [0.0; 3];
        for s in &p.samples {
            add3(&center_pos, &s.off, &mut v);
            let mut coord = Pvf3::default();
            projection::project(&mut coord, v[0], v[1], v[2]);
            // Falls back to flat (center height) rather than 0 when a sample
            // lands on an unloaded neighboring page — absolute 0 would sink or
            // launch that instance away from the rest of the zone.
            let delta_ele = get_ele(&coord).unwrap_or(center_pos[2]) - center_pos[2];
            let lifted = [s.off[0], s.off[1], s.off[2] + delta_ele];
            center3n(&lifted, U_PER_CELL, &mut v);
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
        let count = p.samples.len() as u32;
        with_world(|world, sched| {
            // SAFETY: the zone's floats are in the kernel's memory, read before this returns.
            unsafe { sched.markers.set_zone(world, &mut sched.draw, center_pos, count, lane.as_ptr() as usize, ZONE_RADIUS_CELL * U_PER_CELL) };
        });
        p.shown = true;
        request_world_tick();
    }

    // center_pos floors to the cell grid to line up with the zone's cell-sized
    // boxes; the beacon has no such constraint, so this adds the remainder back
    // — otherwise it would quantize to whole-cell jumps instead of moving
    // smoothly.
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

// ---- the puck as the kernel holds it ----
//
// The zone as a ring of cell boxes laid over the ground, and a beacon above
// it that eases toward the fix, spins and bobs. Where the device is and which
// cells the zone covers at what height are decided above; this holds them,
// animates the beacon each tick, and lists their draws.

/// Floats a zone instance takes: the box's centre relative to the zone's
/// (3), and which of its top, bottom, right and left edges border the ring
/// (4).
pub const ZONE_INSTANCE_FLOATS: usize = 7;
/// The beacon's height above the ground, how fast it eases toward the fix,
/// past what distance it jumps instead, and how far away it stops being
/// drawn.
const BEACON_Z_BIAS: f64 = 0.5;
const BEACON_SMOOTH_RATE: f64 = 6.0;

pub struct PuckDraw {
    at: usize,
    bytes: u32,
    handle: u32,
    count: u32,
    /// The zone's centre, in units.
    centre: V3,
    /// Where the beacon should stand and where it stands, relative to the centre.
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
    /// The zone: `count` instances of ZONE_INSTANCE_FLOATS floats from `lane`,
    /// about a centre at `centre` in units. Replaces what was there.
    ///
    /// # Safety
    /// `lane` addresses count * ZONE_INSTANCE_FLOATS f32s in the memory.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn set_zone(&mut self, world: &mut World, draw: &mut DrawList, centre: V3, count: u32, lane: usize, snap_dist: f64) {
        let bytes = word_bytes(count as usize * ZONE_INSTANCE_FLOATS).max(4);
        let keep = self.puck.as_ref().is_some_and(|p| p.bytes == bytes);
        if !keep {
            self.clear(draw);
            let Some((at, bytes)) = alloc(bytes) else { return };
            self.puck = Some(PuckDraw {
                at,
                bytes,
                handle: 0,
                count,
                centre,
                target: [0.0; 2],
                shown: None,
                snap_dist2: snap_dist * snap_dist,
                night: false,
                time: 0.0,
                visible: false,
            });
        }
        let p = self.puck.as_mut().unwrap();
        // The beacon's shown offset follows the centre, so a centre that moves
        // a whole cell does not play as a slide back.
        if let Some(s) = p.shown.as_mut() {
            s[0] -= centre[0] - p.centre[0];
            s[1] -= centre[1] - p.centre[1];
        }
        p.centre = centre;
        p.count = count;
        let mem = world.volumes.mem();
        core::ptr::copy_nonoverlapping(mem.add(lane), mem.add(p.at), count as usize * ZONE_INSTANCE_FLOATS * 4);
        if p.handle == 0 {
            p.handle = draw.upload(p.at as u32, p.bytes, 0, 0);
        } else {
            draw.rewrite(p.handle, p.at as u32, p.bytes);
        }
    }

    /// Where the beacon should stand relative to the zone's centre, and
    /// whether it wears the night colour.
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

    /// Advances the beacon; whether anything is animating and wants frames.
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
        // Drawn within the fog's far edge of the camera, as the three puck
        // hid itself past it: the beacon's place through the anchor root.
        let [x, y, z] = frame.world_of([p.centre[0] + shown[0], p.centre[1] + shown[1], p.centre[2] + BEACON_Z_BIAS]);
        let c = frame.camera;
        p.visible = (x - c[0]).powi(2) + (y - c[1]).powi(2) + (z - c[2]).powi(2) < view_reach().powi(2);
        p.visible
    }

    pub fn draw(&self, draw: &mut DrawList, origin: V3) {
        let Some(p) = &self.puck else { return };
        if !p.visible || p.handle == 0 {
            return;
        }
        let centre = DrawList::translation(p.centre, origin);
        draw.draw_as(PIPELINE_PUCK_ZONE, p.handle, p.count, 0, centre, 0.0, 1.0);
        let shown = p.shown.unwrap_or(p.target);
        let beacon = [
            centre[0] + shown[0] as f32,
            centre[1] + shown[1] as f32,
            centre[2] + (BEACON_Z_BIAS + (p.time * 2.0).sin() * 0.1) as f32,
        ];
        draw.draw_as(PIPELINE_PUCK_BEACON, 0, p.night as u32, 0, beacon, (p.time * 1.2) as f32, 1.0);
    }
}
