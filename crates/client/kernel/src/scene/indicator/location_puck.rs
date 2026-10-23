// pxl-kernel/scene/indicator/location_puck.rs

//! The device's location puck: a zone of cells about the fix, each on the
//! ground under it, and a beacon above that eases, spins and bobs.

use pxl_render::frame::draw::{DrawList, Held, PIPELINE_PUCK_BEACON, PIPELINE_PUCK_ZONE};
use pxl_render::gpu::api::bytes;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;

use crate::kernel;
use crate::query::World;
use crate::scene::frame::Frame;
use crate::sched::edge::request_world_tick;

// A world-space footprint (4u), whatever the ground's cell; the .707 is a
// one-cell diagonal correction.
const ZONE_RADIUS_CELL: f64 = 4.0 / units(GROUND.cell()) - 0.707;
const ZONE_RADIUS2_CELL: f64 = ZONE_RADIUS_CELL * ZONE_RADIUS_CELL;
/// The zone's, past which the beacon snaps to its target rather than slide.
const ZONE_RADIUS: f64 = ZONE_RADIUS_CELL * units(GROUND.cell());

/// Which of its sides (top, bottom, right, left) lie on the zone's border.
struct ZoneSample {
    off: V3,
    borders: [f64; 4],
}

fn zone_samples() -> impl Iterator<Item = ZoneSample> {
    let range = ZONE_RADIUS_CELL.ceil() as i32;
    let flag = |b: bool| if b { 1.0 } else { 0.0 };
    (-range..=range).flat_map(move |x| (-range..=range).map(move |y| (x as f64, y as f64))).filter_map(move |(dx, dy)| {
        let (dx2, dy2) = (dx * dx, dy * dy);
        if dx2 + dy2 > ZONE_RADIUS2_CELL {
            return None;
        }
        let d_top = dx2 + (dy + 1.0) * (dy + 1.0);
        let d_bottom = dx2 + (dy - 1.0) * (dy - 1.0);
        let d_right = (dx + 1.0) * (dx + 1.0) + dy2;
        let d_left = (dx - 1.0) * (dx - 1.0) + dy2;
        Some(ZoneSample {
            off: [dx * units(GROUND.cell()), dy * units(GROUND.cell()), -units(GROUND.cell())],
            borders: [
                flag(d_top > ZONE_RADIUS2_CELL),
                flag(d_bottom > ZONE_RADIUS2_CELL),
                flag(d_right > ZONE_RADIUS2_CELL),
                flag(d_left > ZONE_RADIUS2_CELL),
            ],
        })
    })
}

/// What grounding the zone is made of, by value: the fix, and where it is
/// in the eye's frame. With the ground its queries read (`read`), it is all.
#[derive(Clone, Copy, PartialEq)]
struct Seen {
    device: Q3,
    pos: V3,
}

pub fn location_puck_step(device_coord: Option<Q3>) {
    let Some((scene, draw)) = kernel().parts() else { return };
    let night = scene.environment.night();
    scene.puck.step(draw, device_coord, night);
}

/// The box's center relative to the zone's (3), and its border flags (4).
pub const ZONE_INSTANCE_FLOATS: usize = 7;
const BEACON_Z_BIAS: f64 = 0.5;
const BEACON_SMOOTH_RATE: f64 = 6.0;
/// How far the beacon bobs either way, in units.
const BEACON_BOB: f64 = 0.1;

/// The zone shown and its beacon.
struct Zone {
    center: V3,
    /// Relative to the center.
    target: [f64; 2],
    shown: Option<[f64; 2]>,
    night: bool,
    time: f64,
    visible: bool,
}

#[derive(Default)]
pub struct LocationPuck {
    /// Holds across a GPS dropout: rebuilding the zone on a blip caused a
    /// stacked-cube glitch after tab-away-and-back.
    last_known_device_coord: Option<Q3>,
    /// None until grounded, rather than drawing at a guessed height.
    last_known_center_z: Option<f64>,
    /// What the zone was last grounded from, and the center it gave; none
    /// where it found no height.
    grounded: Option<(Seen, Option<V3>)>,
    /// The zone grounded this step, kept so a step allocates nothing.
    lane: Vec<f32>,
    /// The zone's instances drawn, ZONE_INSTANCE_FLOATS floats an instance,
    /// which `held` holds, so grounding it again to the same heights draws
    /// nothing anew.
    drawn: Vec<f32>,
    held: Held,
    /// None where nothing is shown.
    zone: Option<Zone>,
}

impl LocationPuck {
    fn step(&mut self, draw: &mut DrawList, device_coord: Option<Q3>, night: bool) {
        if device_coord.is_some() {
            self.last_known_device_coord = device_coord;
        }
        let Some(device) = self.last_known_device_coord else {
            return self.clear(draw);
        };

        let seen = Seen {
            device,
            pos: crate::view::frame::coord::to_world(&device),
        };
        let center_pos = match self.grounded {
            Some((s, center)) if s == seen && !crate::query::changed::within(read(&device), &[]) => center,
            _ => {
                let center = self.ground(draw, seen);
                self.grounded = Some((seen, center));
                center
            }
        };
        let Some(center_pos) = center_pos else {
            return self.clear(draw);
        };
        let pos = seen.pos;

        // The beacon adds back what the zone's cell flooring dropped, so it moves
        // smoothly rather than in whole-cell jumps.
        if let Some(zone) = self.zone.as_mut() {
            zone.target = [pos[0] - center_pos[0], pos[1] - center_pos[1]];
            zone.night = night;
        }
    }

    /// The zone grounded at the fix and drawn where it changed: its center, none
    /// where the fix has no height yet.
    fn ground(&mut self, draw: &mut DrawList, seen: Seen) -> Option<V3> {
        let (device, pos) = (seen.device, seen.pos);
        // The fix names a place and not how high.
        if let Some(z) = World::now().surface_support(&device).value().flatten() {
            self.last_known_center_z = Some(z);
        }
        let center_z = self.last_known_center_z?;
        let raw = [pos[0], pos[1], center_z];
        let center_pos = v3::floor(&raw, units(GROUND.cell()));

        let mut lane = core::mem::take(&mut self.lane);
        lane.clear();
        for s in zone_samples() {
            let v = v3::add(&center_pos, &s.off);
            let coord = world::point(v[0], v[1], v[2]);
            // An unloaded neighbor tile falls back to the center's height; 0 would sink
            // or launch that instance.
            let delta_ele = World::now().surface_support(&coord).value().flatten().unwrap_or(center_pos[2]) - center_pos[2];
            let lifted = [s.off[0], s.off[1], s.off[2] + delta_ele];
            let v = v3::center(&lifted, units(GROUND.cell()));
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
        if self.zone.as_ref().is_none_or(|z| z.center != center_pos) || self.drawn != lane {
            self.held.write(draw, bytes(&lane));
            match self.zone.as_mut() {
                // Follows the center, so a whole-cell move does not play as a slide back.
                Some(z) => {
                    if let Some(s) = z.shown.as_mut() {
                        s[0] -= center_pos[0] - z.center[0];
                        s[1] -= center_pos[1] - z.center[1];
                    }
                    z.center = center_pos;
                }
                None => {
                    self.zone = Some(Zone {
                        center: center_pos,
                        target: [0.0; 2],
                        shown: None,
                        night: false,
                        time: 0.0,
                        visible: false,
                    });
                }
            }
            core::mem::swap(&mut self.drawn, &mut lane);
            request_world_tick();
        }
        self.lane = lane;
        Some(center_pos)
    }

    fn clear(&mut self, draw: &mut DrawList) {
        if self.zone.take().is_none() {
            return;
        }
        self.held.free(draw);
        self.drawn.clear();
        request_world_tick();
    }

    /// Whether anything is animating and wants frames. The puck is drawn
    /// where an actor would be (scene/actor.rs `tick`): in the frustum and
    /// spanning a camera cell at its distance; and it moves, its beacon
    /// sliding to its target or bobbing, only where the move spans one, so a
    /// frame is asked only for what can be seen to change. Else the beacon
    /// stands at its target and its time holds.
    pub fn tick(&mut self, frame: &Frame) -> bool {
        let Some(p) = self.zone.as_mut() else { return false };
        let dt = frame.delta_s;
        let to = v3::sub(&frame.world_of(p.center), &frame.camera);
        let cell_there = crate::view::lens::cell_there(v3::len(&to));
        p.visible = frame.sees(p.center, ZONE_RADIUS) && 2.0 * ZONE_RADIUS >= cell_there;
        let shown = p.shown.get_or_insert(p.target);
        let gap = libm::hypot(p.target[0] - shown[0], p.target[1] - shown[1]);
        let slides = p.visible && gap <= ZONE_RADIUS && gap >= cell_there;
        if slides {
            let a = 1.0 - libm::exp(-BEACON_SMOOTH_RATE * dt);
            shown[0] += (p.target[0] - shown[0]) * a;
            shown[1] += (p.target[1] - shown[1]) * a;
        } else {
            *shown = p.target;
        }
        let bobs = p.visible && 2.0 * BEACON_BOB >= cell_there;
        if bobs {
            p.time += dt;
        }
        slides || bobs
    }

    pub fn draw(&self, draw: &mut DrawList, eye: V3) {
        let Some(p) = &self.zone else { return };
        if !p.visible || self.held.handle == 0 {
            return;
        }
        let center = DrawList::translation(p.center, eye);
        let count = (self.drawn.len() / ZONE_INSTANCE_FLOATS) as u32;
        draw.draw_as(PIPELINE_PUCK_ZONE, self.held.handle, count, 0, center, 0.0, 1.0);
        let shown = p.shown.unwrap_or(p.target);
        let beacon = [
            center[0] + shown[0] as f32,
            center[1] + shown[1] as f32,
            center[2] + (BEACON_Z_BIAS + libm::sin(p.time * 2.0) * BEACON_BOB) as f32,
        ];
        draw.draw_as(PIPELINE_PUCK_BEACON, 0, p.night as u32, 0, beacon, (p.time * 1.2) as f32, 1.0);
    }
}

/// The columns grounding the zone at `device` reads, corners in units: its
/// cells, floored to the cell, and a cell about each.
fn read(device: &Q3) -> [[f64; 2]; 2] {
    let at = [0, 1].map(|k| pxl_spec::space::coord::q3::to_real(i64::from(device[k])));
    let reach = (ZONE_RADIUS_CELL.ceil() + 2.0) * units(GROUND.cell());
    [at.map(|v| v - reach), at.map(|v| v + reach)]
}
