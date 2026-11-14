// pxl-core/domain/scene/indicator/location.rs

//! The device's location indicator: a zone of cells around the location fix,
//! each cell set on the ground under it, and a beacon above the zone that
//! eases toward the fix, spins and bobs.

use pxl_render::frame::shader::record::PIPELINE_LOCATION_ZONE;
use pxl_render::gpu::api::bytes;
use pxl_render::gpu::draw::{DrawId, DrawList, MirroredBlock, Position, ReceiverBox};
use pxl_spec::space::camera::grid;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;

use crate::domain::query::change::Memo;
use crate::domain::query::World;
use crate::domain::scene::animation;
use crate::domain::scene::environment;
use crate::domain::scene::frame::Frame;
use crate::domain::scene::indicator::param::{
    BEACON_BOB, BEACON_BOB_RATE, BEACON_SMOOTH_RATE, BEACON_SPIN_RATE, BEACON_Z_BIAS,
    ZONE_FOOTPRINT_U,
};
use crate::domain::scene::model::draw_list;
use crate::domain::smooth::smoothing_factor;
use crate::domain::view::frame::coord;
use crate::domain::view::lens;
use crate::domain::view::lens::{visible_step_s, StepClock};
use crate::sys::sched::{invalidate, invalidate_at};
use pxl_global::Global;
use pxl_render::frame::shader::word;
use pxl_render::param::location::beacon;
use xpute_guest::clock::now;

// The zone's radius in the ground's leaf cells, from its footprint
// (`ZONE_FOOTPRINT_U`). The 0.707 taken off is half a cell's diagonal, so a
// cell whose center lies within the radius lies wholly within the footprint.
const ZONE_RADIUS_CELL: f64 = ZONE_FOOTPRINT_U / units(GROUND.leaf_cell_zoom()) - 0.707;
const ZONE_RADIUS2_CELL: f64 = ZONE_RADIUS_CELL * ZONE_RADIUS_CELL;
/// The zone's radius in world units. A beacon farther than this from its
/// target snaps to it rather than sliding.
const ZONE_RADIUS: f64 = ZONE_RADIUS_CELL * units(GROUND.leaf_cell_zoom());

/// A cell of the zone. `borders` flags which of its sides (top, bottom,
/// right, left) lie on the zone's border.
struct ZoneSample {
    /// Its place, in the ground's cells from the center.
    at: [i32; 2],
    off: V3,
    borders: [f64; 4],
}

/// How far the zone reaches from its center along an axis, in cells.
fn zone_extent() -> i32 {
    ZONE_RADIUS_CELL.ceil() as i32
}

fn zone_samples() -> impl Iterator<Item = ZoneSample> {
    let range = zone_extent();
    let flag = |b: bool| if b { 1.0 } else { 0.0 };
    (-range..=range)
        .flat_map(move |x| (-range..=range).map(move |y| (x as f64, y as f64)))
        .filter_map(move |(dx, dy)| {
            let (dx2, dy2) = (dx * dx, dy * dy);
            if dx2 + dy2 > ZONE_RADIUS2_CELL {
                return None;
            }
            let d_top = dx2 + (dy + 1.0) * (dy + 1.0);
            let d_bottom = dx2 + (dy - 1.0) * (dy - 1.0);
            let d_right = (dx + 1.0) * (dx + 1.0) + dy2;
            let d_left = (dx - 1.0) * (dx - 1.0) + dy2;
            Some(ZoneSample {
                at: [dx as i32, dy as i32],
                off: [
                    dx * units(GROUND.leaf_cell_zoom()),
                    dy * units(GROUND.leaf_cell_zoom()),
                    -units(GROUND.leaf_cell_zoom()),
                ],
                borders: [
                    flag(d_top > ZONE_RADIUS2_CELL),
                    flag(d_bottom > ZONE_RADIUS2_CELL),
                    flag(d_right > ZONE_RADIUS2_CELL),
                    flag(d_left > ZONE_RADIUS2_CELL),
                ],
            })
        })
}

/// The inputs the zone's grounding is built from, compared by value: the fix,
/// and where the fix is in the eye's frame. Beyond these, grounding depends
/// only on the ground its queries read, which the memo tracks (`Memo`).
#[derive(Clone, Copy, PartialEq)]
struct Key {
    device: Q3,
    pos: V3,
}

static LOCATION: Global<LocationIndicator> = Global::new();

fn location() -> &'static mut LocationIndicator {
    LOCATION.get(LocationIndicator::default)
}

pub(crate) fn location_step(device_coord: Option<Q3>) {
    location().step(draw_list(), device_coord);
}

/// Advances the beacon by one frame (`LocationIndicator::tick`). Returns
/// whether it wants more frames.
pub(crate) fn location_animate(frame: &Frame) -> bool {
    location().tick(frame)
}

pub(crate) fn location_draw(draw: &mut DrawList) {
    location().draw(draw);
}

/// The zone's instances about `center_pos`, `ZONE_INSTANCE_FLOATS` floats
/// each. The ground is asked of `ground` once
/// for each cell of the zone and of the ring around it, which the zone's
/// sides look across to.
fn lay_zone(center_pos: V3, mut ground: impl FnMut(&Q3) -> Option<f64>) -> Vec<f32> {
    let mut instances = Vec::new();
    let cell = units(GROUND.leaf_cell_zoom());
    const BESIDE: [[i32; 2]; 4] = [[1, 0], [-1, 0], [0, 1], [0, -1]];
    let ring = zone_extent() + 1;
    let across = (2 * ring + 1) as usize;
    let idx = |[x, y]: [i32; 2]| ((y + ring) as usize) * across + (x + ring) as usize;
    // Each cell's ground height above the zone's center.
    let mut rise: Vec<Option<f64>> = vec![None; across * across];
    for s in zone_samples() {
        for d in [[0, 0]].iter().chain(&BESIDE) {
            let at = [s.at[0] + d[0], s.at[1] + d[1]];
            if rise[idx(at)].is_none() {
                let off = [
                    s.off[0] + f64::from(d[0]) * cell,
                    s.off[1] + f64::from(d[1]) * cell,
                ];
                let v = v3::add(&center_pos, &[off[0], off[1], s.off[2]]);
                // Where a neighboring tile is not loaded, the cell takes the
                // center's height.
                let ground = ground(&world::point(v[0], v[1], v[2])).unwrap_or(center_pos[2]);
                rise[idx(at)] = Some(ground - center_pos[2]);
            }
        }
    }
    // The box on the ground of the cell `d` away from the sample, relative to
    // the center.
    let box_at = |s: &ZoneSample, d: [i32; 2]| {
        let delta_ele = rise[idx([s.at[0] + d[0], s.at[1] + d[1]])].unwrap_or_default();
        let off = [
            s.off[0] + f64::from(d[0]) * cell,
            s.off[1] + f64::from(d[1]) * cell,
        ];
        v3::center(&[off[0], off[1], s.off[2] + delta_ele], cell)
    };
    for s in zone_samples() {
        let v = box_at(&s, [0, 0]);
        // A side face is open to the air where the cell beside it is lower.
        // The top is always open and the bottom never is. Bit k stands for
        // face k, in the box's order (+x, -x, +y, -y, +z, -z).
        let open = BESIDE.iter().enumerate().fold(1 << 4, |open, (k, &d)| {
            open | u32::from(box_at(&s, d)[2] < v[2]) << k
        });
        instances.extend([
            v[0] as f32,
            v[1] as f32,
            v[2] as f32,
            s.borders[0] as f32,
            s.borders[1] as f32,
            s.borders[2] as f32,
            s.borders[3] as f32,
            open as f32,
        ]);
    }
    instances
}

/// The beacon's fastest motion, in units per second. The bob moves its
/// middle, and the spin moves its tips, one octahedron radius out
/// (pxl-render param.rs `location::beacon::RADIUS`). The two motions are at
/// right angles, and this bound takes both at their peaks together.
fn beacon_speed() -> f64 {
    libm::hypot(
        BEACON_BOB * BEACON_BOB_RATE,
        BEACON_SPIN_RATE * f64::from(beacon::RADIUS),
    )
}

/// The floats of one zone instance: the box's center relative to the zone's
/// (3), its border flags (4), and the faces it draws open to the air (1, a bit
/// for each face). location_zone.wgsl reads this stride (pxl-render
/// frame/shader/word.rs `LOCATION_ZONE_INSTANCE_FLOATS`).
pub(crate) const ZONE_INSTANCE_FLOATS: usize = word::LOCATION_ZONE_INSTANCE_FLOATS as usize;

/// The zone shown, and its beacon: where the fix puts them (`center`,
/// `beacon_to`), and what a frame moves or finds (`tick`).
struct Zone {
    center: V3,
    /// Where the beacon is going, relative to the center.
    beacon_to: [f64; 2],
    /// Where the beacon is shown, relative to the center, or None before a
    /// frame has placed it.
    beacon_at: Option<[f64; 2]>,
    /// The sum of each frame's step (`frame.delta_s`), which a stall does not
    /// inflate, while the bob spans a camera cell.
    run: f64,
    /// The seconds the bob and the spin are drawn at, stepping a seen step at
    /// a time from `run`.
    clock: StepClock,
    visible: bool,
    /// Whether the ground under the zone is drawn in leaf cells (`GROUND`) or
    /// finer. Where the ground is drawn coarser, the zone's cells would be
    /// smaller than the cells they stand on, so the zone is not drawn.
    zone_drawn: bool,
}

#[derive(Default)]
pub(crate) struct LocationIndicator {
    /// The last height found under the fix, or None until one is found.
    last_known_center_z: Option<f64>,
    /// The center the zone was last grounded at, or None when no height was
    /// found, and whether every height it read was exact. It is kept while
    /// the fix stays and no change touches the ground its grounding read
    /// (`Memo`).
    ground_center: Memo<Key, (Option<V3>, bool)>,
    /// The zone's instances `block` holds. A grounding that comes out the
    /// same as these writes nothing new.
    drawn: Vec<f32>,
    block: MirroredBlock,
    /// None when nothing is shown.
    zone: Option<Zone>,
    zone_id: DrawId,
    beacon_id: DrawId,
}

impl LocationIndicator {
    /// Stands at the device's location as the store keeps it, which outlives
    /// a dropout and a hidden spell, and is gone where the location ends.
    fn step(&mut self, draw: &mut DrawList, device_coord: Option<Q3>) {
        let Some(device) = device_coord else {
            if self.zone.take().is_some() {
                invalidate();
            }
            return;
        };

        let seen = Key {
            device,
            pos: coord::to_world(&device),
        };
        // The memo is checked every frame, so it sees each change once. A zone
        // whose block the heap refused is laid again.
        let mut grounded = core::mem::take(&mut self.ground_center);
        if self.zone.is_some() && self.block.handle == 0 {
            grounded = Memo::default();
        }
        grounded.forget_touched_at_leaf(|&(_, exact)| exact);
        let w = World::now();
        let (center, _) = grounded.of(seen, || self.ground(w, draw, seen));
        self.ground_center = grounded;
        let Some(center) = center else {
            return;
        };
        // The beacon adds back what the zone's cell flooring dropped, so it moves
        // smoothly rather than in whole-cell jumps.
        let beacon_to = [seen.pos[0] - center[0], seen.pos[1] - center[1]];
        match self.zone.as_mut() {
            Some(zone) => zone.beacon_to = beacon_to,
            None => {
                self.zone = Some(Zone {
                    center,
                    beacon_to,
                    beacon_at: None,
                    run: 0.0,
                    clock: StepClock::default(),
                    visible: false,
                    zone_drawn: false,
                });
            }
        }
    }

    /// Grounds the zone at the fix and writes it where it changed. Returns the
    /// zone's center, or None while the fix has no height yet, and whether
    /// every height it read was exact.
    fn ground(&mut self, world: World, draw: &mut DrawList, key: Key) -> (Option<V3>, bool) {
        let (device, pos) = (key.device, key.pos);
        // The fix names a place but not its altitude, so the elevation is the
        // ground's under it, and the last one found stands until a new one is.
        let center = world.surface_support(&device);
        let mut exact = center.exact().is_some();
        if let Some(z) = center.val().flatten() {
            self.last_known_center_z = Some(z);
        }
        let Some(center_z) = self.last_known_center_z else {
            return (None, exact);
        };
        let raw = [pos[0], pos[1], center_z];
        let center_pos = v3::floor(&raw, units(GROUND.leaf_cell_zoom()));

        let instances = lay_zone(center_pos, |at| {
            let support = world.surface_support(at);
            exact &= support.exact().is_some();
            support.val().flatten()
        });
        let changed = self.zone.as_ref().is_some_and(|z| z.center != center_pos);
        if self.block.handle == 0 || changed || self.drawn != instances {
            self.block.write(draw, bytes(&instances));
            // The beacon's shown place moves with the center, so a whole-cell
            // move of the zone does not play as the beacon sliding back.
            if let Some(z) = self.zone.as_mut() {
                if let Some(s) = z.beacon_at.as_mut() {
                    s[0] -= center_pos[0] - z.center[0];
                    s[1] -= center_pos[1] - z.center[1];
                }
                z.center = center_pos;
            }
            self.drawn = instances;
            invalidate();
        }
        (Some(center_pos), exact)
    }

    /// Advances the beacon by one frame. Returns whether it slides and wants
    /// more frames. The indicator is drawn by the rule a character is drawn
    /// by (domain/scene/character.rs `tick`): when it is in the frustum and
    /// spans a camera cell at its distance. Its zone is drawn only where the
    /// ground under it is drawn in leaf cells or finer (`zone_drawn`). The
    /// beacon slides toward its target, a step each frame, only while the
    /// slide spans a camera cell, and snaps otherwise. The bob and the spin
    /// step a seen step at a time while the bob spans a camera cell, and the
    /// frame is woken for the next step (`lens::StepClock`). Otherwise they
    /// hold still.
    pub(crate) fn tick(&mut self, frame: &Frame) -> bool {
        let Some(p) = self.zone.as_mut() else {
            return false;
        };
        let dt = frame.delta_s;
        let to = v3::sub(&frame.world_of(p.center), &frame.camera);
        let cell_there = lens::cell_there(v3::len(&to));
        p.visible = frame.sees(p.center, ZONE_RADIUS) && 2.0 * ZONE_RADIUS >= cell_there;
        p.zone_drawn =
            p.visible && grid::cell_zoom_at(v3::dot(&to, &to) as f32) >= GROUND.leaf_cell_zoom();
        let shown = p.beacon_at.get_or_insert(p.beacon_to);
        let gap = libm::hypot(p.beacon_to[0] - shown[0], p.beacon_to[1] - shown[1]);
        let slides = p.visible && gap <= ZONE_RADIUS && gap >= cell_there;
        if slides {
            let a = smoothing_factor(BEACON_SMOOTH_RATE, dt);
            shown[0] += (p.beacon_to[0] - shown[0]) * a;
            shown[1] += (p.beacon_to[1] - shown[1]) * a;
        } else {
            *shown = p.beacon_to;
        }
        // The step is measured at the beacon, which stands over the zone's
        // center by its bias.
        let beacon = v3::add(&to, &[shown[0], shown[1], BEACON_Z_BIAS]);
        let mut stepped = false;
        if p.visible && 2.0 * BEACON_BOB >= cell_there {
            p.run += dt;
            let step = visible_step_s(v3::len(&beacon), beacon_speed());
            let (_, next, s) = p.clock.at(p.run, |at| at + step);
            stepped = s;
            invalidate_at(now() + (next - p.run) * 1000.0);
        }
        if slides || stepped {
            animation::note_animation();
        }
        slides
    }

    pub(crate) fn draw(&self, draw: &mut DrawList) {
        let Some(p) = &self.zone else { return };
        let Some(shown) = p.beacon_at.filter(|_| p.visible && self.block.handle != 0) else {
            return;
        };
        let time = p.clock.time();
        // The zone is drawn about its center, as a Q3 position. The beacon is
        // drawn at its own position, finer than Q3, over where it is shown.
        // The bob and the slide move only the beacon, since the zone reads no
        // time. When either draw changes, it marks where it was and where it
        // is (pxl-render pass/mark.rs).
        if p.zone_drawn {
            let cnt = (self.drawn.len() / ZONE_INSTANCE_FLOATS) as u32;
            draw.draw_as(
                &self.zone_id,
                PIPELINE_LOCATION_ZONE,
                self.block.handle,
                cnt,
                0,
                Position::of(p.center),
                None,
            );
        }
        let (x, y) = (p.center[0] + shown[0], p.center[1] + shown[1]);
        let rest = p.center[2] + BEACON_Z_BIAS;
        let beacon = Position::of([x, y, rest + libm::sin(time * BEACON_BOB_RATE) * BEACON_BOB]);
        // The beacon reads the sun's map, so it declares the box it stands
        // within: the box its whole bob sweeps, which stays still while it
        // bobs (pxl-render gpu/draw.rs `ReceiverBox`).
        let r = f64::from(beacon::RADIUS);
        let bob = BEACON_BOB + r;
        let stand = ReceiverBox::of([x - r, y - r, rest - bob], [x + r, y + r, rest + bob]);
        let spin = (time * BEACON_SPIN_RATE) as f32;
        draw.draw_beacon(&self.beacon_id, beacon, spin, environment::night(), stand);
    }
}

#[cfg(test)]
#[path = "location.test.rs"]
mod test;
