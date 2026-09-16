//! The location puck: the device's interaction zone as a ring of cell boxes
//! laid over the ground, and a beacon above it that eases toward the fix,
//! spins and bobs. module/world/marker/location_puck.tsx decides where the
//! device is and which cells the zone covers at what height; the core holds
//! them, animates the beacon each tick, and lists their draws.

use crate::render::draw::{DrawList, PIPELINE_PUCK_BEACON, PIPELINE_PUCK_ZONE};
use crate::world::build::World;
use crate::world::sched::Frame;

/// Floats a zone instance takes: the box's centre relative to the zone's
/// (3), and which of its top, bottom, right and left edges border the ring
/// (4).
pub const ZONE_INSTANCE_FLOATS: usize = 7;
/// location_puck.tsx's constants: the beacon's height above the ground, how
/// fast it eases toward the fix, past what distance it jumps instead, and
/// how far away it stops being drawn.
const BEACON_Z_BIAS: f64 = 0.5;
const BEACON_SMOOTH_RATE: f64 = 6.0;

pub struct Puck {
    at: usize,
    bytes: u32,
    handle: u32,
    count: u32,
    /// The zone's centre, in units.
    centre: [f64; 3],
    /// Where the beacon should stand and where it stands, relative to the centre.
    target: [f64; 2],
    shown: Option<[f64; 2]>,
    snap_dist2: f64,
    fog_far: f64,
    night: bool,
    time: f64,
    visible: bool,
}

#[derive(Default)]
pub struct Markers {
    puck: Option<Puck>,
}

impl Markers {
    /// The zone: `count` instances of ZONE_INSTANCE_FLOATS floats from `lane`,
    /// about a centre at `centre` in units. Replaces what was there.
    ///
    /// # Safety
    /// `lane` addresses count * ZONE_INSTANCE_FLOATS f32s in the memory.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn set_zone(&mut self, world: &mut World, draw: &mut DrawList, centre: [f64; 3], count: u32, lane: usize, snap_dist: f64, fog_far: f64) {
        let bytes = (count as usize * ZONE_INSTANCE_FLOATS * 4).max(4) as u32;
        let keep = self.puck.as_ref().is_some_and(|p| p.bytes == bytes);
        if !keep {
            self.clear(draw);
            let Some(at) = crate::bake::volume::take_bytes(bytes as usize) else { return };
            self.puck = Some(Puck {
                at,
                bytes,
                handle: 0,
                count,
                centre,
                target: [0.0; 2],
                shown: None,
                snap_dist2: snap_dist * snap_dist,
                fog_far,
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
            unsafe { crate::bake::volume::give_bytes(p.at, p.bytes as usize) };
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
        p.visible = (x - c[0]).powi(2) + (y - c[1]).powi(2) + (z - c[2]).powi(2) < p.fog_far * p.fog_far;
        p.visible
    }

    pub fn draw(&self, draw: &mut DrawList, origin: [f64; 3]) {
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
