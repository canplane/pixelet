// pxl_core/module/world/marker/location_puck.rs

//! The device's location puck: a zone of cells about where the device is, each
//! at the ground under it, and a beacon above. The puck itself is the
//! kernel's (crates/pxl/core/src/world/marker.rs): this decides where the
//! device is, which cells the zone covers at what height and where the beacon
//! should stand; the kernel holds the zone, eases, spins and bobs the beacon,
//! and draws both.

use pxl_shared::bake::constant::U_PER_CELL;
use pxl_shared::spec::space::pvf3::projection;
use pxl_shared::spec::space::pvf3::pvf3::PVF3;
use pxl_shared::spec::space::v3::{add3, center3n, floor3n, V3};

use crate::global::Global;
use crate::kernel::kernel;
use crate::module::world::change::request_world_tick;
use crate::module::world::environment::config::render_cfg::FOG_FAR;
use crate::module::world::environment::theme::WorldThemeKey;
use crate::module::world::environment::theme_clock::use_theme_key;
use crate::module::world::terrain::elevation::get_ele;
use crate::module::world::terrain::store::tile_pages;
use crate::world::build::World;

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
                borders: [flag(d_top > ZONE_RADIUS2_CELL), flag(d_bottom > ZONE_RADIUS2_CELL), flag(d_right > ZONE_RADIUS2_CELL), flag(d_left > ZONE_RADIUS2_CELL)],
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
    last_known_device_coord: Option<PVF3>,
    /// Not yet grounded gates drawing, instead of drawing the marker at a
    /// guessed height while the device's own page isn't loaded yet.
    last_known_center_z: Option<f64>,
    /// The zone as last written: its centre, and the tiles it was grounded
    /// against. A zone sample can reach a page not yet loaded; a page arriving
    /// grounds it again.
    zone_for: Option<([f64; 3], u32)>,
    beacon_for: Option<([f64; 2], bool)>,
    shown: bool,
}

static PUCK: Global<Puck> = Global::new();

fn world_of<R>(f: impl FnOnce(&mut World, &mut crate::world::sched::Scheduler) -> R) -> Option<R> {
    let k = kernel();
    let volumes = k.volumes.as_mut()?;
    let mut world = World {
        volumes,
        materials: &k.terrain,
        store: &k.objects,
        painter: &mut k.painter,
    };
    Some(f(&mut world, &mut k.sched))
}

/// The puck's turn, for the device's coordinate as it stands.
pub fn location_puck_step(device_coord: Option<PVF3>) {
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

    let tiles = tile_pages().revision();
    if p.zone_for != Some((center_pos, tiles)) {
        p.zone_for = Some((center_pos, tiles));
        let mut lane: Vec<f32> = Vec::with_capacity(p.samples.len() * 7);
        let mut v = [0.0; 3];
        for s in &p.samples {
            add3(&center_pos, &s.off, &mut v);
            let mut coord = PVF3::default();
            projection::project(&mut coord, v[0], v[1], v[2]);
            // Falls back to flat (center height) rather than 0 when a sample
            // lands on an unloaded neighboring page — absolute 0 would sink or
            // launch that instance away from the rest of the zone.
            let delta_ele = get_ele(&coord).unwrap_or(center_pos[2]) - center_pos[2];
            let lifted = [s.off[0], s.off[1], s.off[2] + delta_ele];
            center3n(&lifted, U_PER_CELL, &mut v);
            lane.extend([v[0] as f32, v[1] as f32, v[2] as f32, s.borders[0] as f32, s.borders[1] as f32, s.borders[2] as f32, s.borders[3] as f32]);
        }
        let count = p.samples.len() as u32;
        world_of(|world, sched| {
            // SAFETY: the zone's floats are in the kernel's memory, read before this returns.
            unsafe { sched.markers.set_zone(world, &mut sched.draw, center_pos, count, lane.as_ptr() as usize, ZONE_RADIUS_CELL * U_PER_CELL, FOG_FAR) };
        });
        p.shown = true;
        request_world_tick();
    }

    // center_pos floors to the cell grid to line up with the zone's cell-sized
    // boxes; the beacon has no such constraint, so this adds the remainder back
    // — otherwise it would quantize to whole-cell jumps instead of moving
    // smoothly.
    let beacon = ([pos[0] - center_pos[0], pos[1] - center_pos[1]], use_theme_key() == WorldThemeKey::Night);
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
    world_of(|_world, sched| sched.markers.clear(&mut sched.draw));
    request_world_tick();
}
