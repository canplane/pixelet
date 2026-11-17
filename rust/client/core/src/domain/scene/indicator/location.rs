// pxl-core/domain/scene/indicator/location.rs

//! The device's location indicator: a zone of cells around the location fix,
//! each cell set on the ground under it, and a beacon above the zone that
//! eases toward the fix, spins and bobs.

use crate::spec::step::InvalidationReason;
use pxl_render::frame::shader::record::PIPELINE_LOCATION_ZONE;
use pxl_render::gpu::api::bytes;
use pxl_render::gpu::draw::{DrawId, DrawList, MirroredBlock, Position, ReceiverBox};
use pxl_spec::space::camera::grid;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;

use crate::domain::query::World;
use crate::domain::query::change::Memo;
use crate::domain::scene::animation;
use crate::domain::scene::environment;
use crate::domain::scene::frame::Frame;
use crate::domain::scene::indicator::param::{
    BEACON_BOB, BEACON_BOB_RATE, BEACON_SMOOTH_RATE, BEACON_SPIN_RATE, BEACON_Z_BIAS,
    ZONE_FOOTPRINT_U,
};
use crate::domain::smooth::smoothing_factor;
use crate::domain::view::frame::coord;
use crate::domain::view::lens;
use crate::domain::view::lens::{StepClock, visible_step_s};
use crate::sys::sched::invalidate;
use pxl_global::Global;
use pxl_render::frame::shader::word;
use pxl_render::param::location::beacon;
use xpute_guest::clock::now;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

// The zone's radius in the ground's leaf cells, from its footprint
// (`ZONE_FOOTPRINT_U`). The 0.707 taken off is half a cell's diagonal, so a
// cell whose center lies within the radius lies wholly within the footprint.
const ZONE_RADIUS_CELL: f64 = ZONE_FOOTPRINT_U / units(GROUND.leaf_cell_zoom()) - 0.707;
const ZONE_RADIUS2_CELL: f64 = ZONE_RADIUS_CELL * ZONE_RADIUS_CELL;
/// The zone's radius in world units. A beacon farther than this from its
/// target snaps to it rather than sliding.
const ZONE_RADIUS: f64 = ZONE_RADIUS_CELL * units(GROUND.leaf_cell_zoom());

/// How far the zone reaches from its center along an axis, in cells.
fn zone_extent() -> i32 {
    ZONE_RADIUS_CELL.ceil() as i32
}

/// Whether the cell `at`, in the ground's cells from the center, lies in the
/// zone.
fn in_zone([x, y]: [i32; 2]) -> bool {
    f64::from(x * x + y * y) <= ZONE_RADIUS2_CELL
}

/// The zone's cells, in the ground's cells from the center.
fn zone_cells() -> impl Iterator<Item = [i32; 2]> {
    let range = zone_extent();
    (-range..=range)
        .flat_map(move |x| (-range..=range).map(move |y| [x, y]))
        .filter(|&at| in_zone(at))
}

/// A box's faces, as pass/location.rs `box_lanes` orders them: its sides
/// toward +x, -x, +y and -y, then its top and its bottom. A cell's side `k` is
/// toward its box's face `k`.
const SIDES: [[i32; 2]; 4] = [[1, 0], [-1, 0], [0, 1], [0, -1]];
const TOP: usize = 4;
const BOTTOM: usize = 5;

/// The cell beside `at` on its side `k`.
fn beside([x, y]: [i32; 2], k: usize) -> [i32; 2] {
    [x + SIDES[k][0], y + SIDES[k][1]]
}

/// The bit of a line along the edge of face `f` toward face `g`. A face takes
/// four bits, one for each face across its edges, in the faces' order with
/// its own axis left out. location_zone.wgsl reads them so.
fn line_bit(f: usize, g: usize) -> u32 {
    let idx = if g / 2 > f / 2 { g - 2 } else { g };
    1 << (4 * f + idx)
}

static LOCATION: Global<LocationIndicator> = Global::new();

fn location() -> &'static mut LocationIndicator {
    LOCATION.get(LocationIndicator::default)
}

pub(crate) fn location_step(device_coord: Option<Q3>) {
    location().step(device_coord);
}

/// Advances the beacon by one frame and lays the zone's cells while the zone
/// is drawn (`LocationIndicator::tick`). Returns whether the beacon wants
/// more frames.
pub(crate) fn location_animate(draw: &mut DrawList, frame: &Frame) -> bool {
    location().tick(draw, frame)
}

pub(crate) fn location_draw(draw: &mut DrawList) {
    location().draw(draw);
}

/// The zone's boxes about `center_pos`, `ZONE_INSTANCE_FLOATS` floats each:
/// the box's center relative to the zone's, the faces it shows and the lines
/// it draws, each a bit field. The ground is asked of `ground` once for each
/// cell of the zone and of the ring around it.
///
/// A box is the cube of its cell's top, and it shows its top and each side
/// above a lower cell. The outline lies where a face the zone shows meets a
/// face of a cell beyond the zone: along the top's edge, along the foot of a
/// side, and up the upright edge of a side.
fn lay_zone(center_pos: V3, mut ground: impl FnMut(&Q3) -> Option<f64>) -> Vec<f32> {
    let cell = units(GROUND.leaf_cell_zoom());
    let ring = zone_extent() + 1;
    let across = (2 * ring + 1) as usize;
    let idx = |[x, y]: [i32; 2]| ((y + ring) as usize) * across + (x + ring) as usize;
    // Each cell's box, relative to the center, set on the ground under it.
    let mut boxes: Vec<Option<V3>> = vec![None; across * across];
    for [x, y] in zone_cells() {
        for at in (-1..=1).flat_map(|dx| (-1..=1).map(move |dy| [x + dx, y + dy])) {
            boxes[idx(at)].get_or_insert_with(|| {
                let off = [f64::from(at[0]) * cell, f64::from(at[1]) * cell, -cell];
                let v = v3::add(&center_pos, &off);
                // Where a neighboring tile is not loaded, the cell takes the
                // center's height.
                let ele = ground(&world::point(v[0], v[1], v[2])).unwrap_or(center_pos[2]);
                v3::center(&[off[0], off[1], off[2] + ele - center_pos[2]], cell)
            });
        }
    }
    // Every cell read lies by a cell of the zone, whose box is set above.
    let box_at = |at: [i32; 2]| boxes[idx(at)].or_bug(Errno::ENOTRECOVERABLE);
    let z = |at: [i32; 2]| box_at(at)[2];
    let mut instances = Vec::new();
    for at in zone_cells() {
        let here = z(at);
        let shows = |k: usize| z(beside(at, k)) < here;
        let mut faces = 1 << TOP;
        let mut lines = 0;
        for k in 0..4 {
            faces |= u32::from(shows(k)) << k;
            // Toward a cell beyond the zone, the top's edge meets that cell's
            // top or its side, or the side dropping to it meets its top.
            if !in_zone(beside(at, k)) {
                lines |= if shows(k) {
                    line_bit(k, BOTTOM)
                } else {
                    line_bit(TOP, k)
                };
            }
            // Past the upright edge on the side `j` of a side, where the box
            // shows no side `j`, the face goes on into the side of the cell
            // beside, or turns to the cell across the corner where that cell
            // stands as high.
            for j in (0..4).filter(|j| j / 2 != k / 2) {
                if shows(k) && !shows(j) {
                    let next = beside(at, j);
                    let corner = beside(next, k);
                    if !in_zone(if z(corner) < here { next } else { corner }) {
                        lines |= line_bit(k, j);
                    }
                }
            }
        }
        let v = box_at(at);
        instances.extend([
            v[0] as f32,
            v[1] as f32,
            v[2] as f32,
            faces as f32,
            lines as f32,
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
/// (3), the faces it shows (1, a bit for each face) and the lines it draws (1,
/// `line_bit`). location_zone_place.wgsl and location_zone.wgsl read this
/// stride (pxl-render frame/shader/word.rs `LOCATION_ZONE_INSTANCE_FLOATS`).
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
    /// Where the zone is: the center it was last grounded at, or None when no
    /// height was found, and whether the height it read was exact. It is
    /// kept while the fix stays in the ground's leaf cell it was grounded in,
    /// by the cell's corner, and no change touches the ground it read
    /// (`Memo`).
    ground_center: Memo<[f64; 2], (Option<V3>, bool)>,
    /// What the zone draws: its cells laid about the center this is kept
    /// for, each on the ground under it, and whether every height they read
    /// was exact. The cells are laid only while the zone is drawn, and kept,
    /// drawn or not, until the center moves, a change touches the ground
    /// they read or the fix goes.
    ground_cells: Memo<V3, bool>,
    /// The zone's instances as last laid, which `block` holds unless the
    /// heap refused them.
    instances: Vec<f32>,
    block: MirroredBlock,
    /// None when nothing is shown.
    zone: Option<Zone>,
    zone_id: DrawId,
    beacon_id: DrawId,
}

impl LocationIndicator {
    /// Stands at the device's location as the store keeps it, which outlives
    /// a dropout and a hidden spell, and is gone where the location ends.
    fn step(&mut self, device_coord: Option<Q3>) {
        let Some(device) = device_coord else {
            if self.zone.take().is_some() {
                invalidate(InvalidationReason::Scene);
            }
            // With no fix nothing checks the memos, so neither is kept.
            (self.ground_center, self.ground_cells) = (Memo::NONE, Memo::NONE);
            return;
        };

        let pos = coord::to_world(&device);
        let [x, y, _] = v3::floor(&pos, units(GROUND.leaf_cell_zoom()));
        let at = [x, y];
        // The memos are checked every frame, here both, so each sees a change
        // once and in the same frame.
        self.ground_cells.forget_touched_at_leaf(|&exact| exact);
        let mut memo = core::mem::take(&mut self.ground_center);
        memo.forget_touched_at_leaf(|&(_, exact)| exact);
        let w = World::now();
        let (center, _) = memo.of(at, || self.ground(w, at));
        self.ground_center = memo;
        let Some(center) = center else {
            return;
        };
        // The beacon adds back what the zone's cell flooring dropped, so it moves
        // smoothly rather than in whole-cell jumps.
        let beacon_to = [pos[0] - center[0], pos[1] - center[1]];
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

    /// Grounds the zone's center on the ground's leaf cell at the corner
    /// `at`, the fix's. Returns the center, or None while the fix has no
    /// height yet, and whether the height it read was exact.
    fn ground(&mut self, world: World, at: [f64; 2]) -> (Option<V3>, bool) {
        let cell = units(GROUND.leaf_cell_zoom());
        // The fix names a place but not its altitude, so the elevation is the
        // ground's under its cell, which the query reads whole
        // (`surface_support`), and the last one found stands until a new one is.
        let mid = v3::center(&[at[0], at[1], 0.0], cell);
        let center = world.surface_support(&world::point(mid[0], mid[1], 0.0));
        let exact = center.exact().is_some();
        if let Some(z) = center.val().flatten() {
            self.last_known_center_z = Some(z);
        }
        let Some(center_z) = self.last_known_center_z else {
            return (None, exact);
        };
        let center_pos = v3::floor(&[at[0], at[1], center_z], cell);
        // The beacon's shown place moves with the center, so a whole-cell
        // move of the zone does not play as the beacon sliding back.
        if let Some(z) = self.zone.as_mut().filter(|z| z.center != center_pos) {
            if let Some(s) = z.beacon_at.as_mut() {
                s[0] -= center_pos[0] - z.center[0];
                s[1] -= center_pos[1] - z.center[1];
            }
            z.center = center_pos;
            invalidate(InvalidationReason::Scene);
        }
        (Some(center_pos), exact)
    }

    /// Lays the zone's cells about `center`, each on the ground under it, and
    /// writes its instances where they changed. It is called only while the
    /// zone is drawn, so nothing is asked of the ground for cells not seen,
    /// and what was laid is kept (`ground_cells`, which `step` checks). A
    /// write the heap refused is tried again from the instances held, with
    /// nothing laid again.
    fn lay(&mut self, draw: &mut DrawList, center: V3) {
        if self.block.handle == 0 && !self.instances.is_empty() {
            self.write(draw);
        }
        let mut memo = core::mem::take(&mut self.ground_cells);
        let world = World::now();
        memo.of(center, || {
            let mut exact = true;
            let instances = lay_zone(center, |at| {
                let support = world.surface_support(at);
                exact &= support.exact().is_some();
                support.val().flatten()
            });
            if self.instances != instances {
                self.instances = instances;
                self.write(draw);
            }
            exact
        });
        self.ground_cells = memo;
    }

    /// Writes the instances into the block and asks for a frame either way:
    /// to draw them, or, where the heap refused them, to try again, as the
    /// ghost does.
    fn write(&mut self, draw: &mut DrawList) {
        self.block.write(draw, bytes(&self.instances));
        invalidate(InvalidationReason::Scene);
    }

    /// Advances the beacon by one frame and lays the zone's cells while the
    /// zone is drawn. Returns whether the beacon slides and wants more
    /// frames. The indicator is drawn when it is in the frustum and spans a
    /// camera cell at its distance. Its zone is drawn only where the ground
    /// under it is drawn in leaf cells or finer (`zone_drawn`). The beacon
    /// slides toward its target, a step each frame, only while the slide
    /// spans a camera cell, and snaps otherwise. The bob and the spin step a
    /// seen step at a time while the bob spans a camera cell, and the frame
    /// is woken for the next step (`lens::StepClock`). Otherwise they hold
    /// still.
    pub(crate) fn tick(&mut self, draw: &mut DrawList, frame: &Frame) -> bool {
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
        if p.visible && 2.0 * BEACON_BOB >= cell_there {
            p.run += dt;
            let step = visible_step_s(v3::len(&beacon), beacon_speed());
            animation::read(&mut p.clock, now(), p.run, 1000.0, |at| at + step);
        }
        if slides {
            animation::note_animation();
        }
        // The cells are laid while the zone is drawn, in this frame, so that
        // the first frame that draws it draws them.
        if let (true, center) = (p.zone_drawn, p.center) {
            self.lay(draw, center);
        }
        slides
    }

    pub(crate) fn draw(&self, draw: &mut DrawList) {
        let Some(p) = &self.zone else { return };
        let Some(shown) = p.beacon_at.filter(|_| p.visible) else {
            return;
        };
        let time = p.clock.time();
        // The zone is drawn about its center, as a Q3 position. The beacon is
        // drawn at its own position, finer than Q3, over where it is shown.
        // The bob and the slide move only the beacon, since the zone reads no
        // time. When either draw changes, it marks where it was and where it
        // is (pxl-render pass/mark.rs).
        if p.zone_drawn && self.block.handle != 0 {
            let cnt = (self.instances.len() / ZONE_INSTANCE_FLOATS) as u32;
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
