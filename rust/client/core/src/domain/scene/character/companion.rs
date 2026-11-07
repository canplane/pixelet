// pxl-core/domain/scene/character/companion.rs

//! The companion: the person and the pet the player chose, which it keeps,
//! and how each walks to the pivot. When the pivot comes
//! to rest each makes a fixed-duration catch-up walk there; in follow mode,
//! where the pivot is a ~1 Hz GPS stream that never rests, or steered by
//! the keys, each instead glides at the world-scroll rate so the two cancel
//! and it holds at screen center. Neither is anchored to the other: the pet
//! walks to its own place beside the person's. While the eye is the pivot
//! neither is placed, and each is placed again at its target once the eye
//! leaves it. Each is drawn clear of the wall beside it (`clear_of_walls`).
//! Assets: Kenney's "Mini Characters" and "Cube Pets", CC0.

use crate::domain::param;
use crate::domain::query::changes::Memo;
use crate::domain::query::World;
use crate::domain::query::{cell_zoom_at, over, under, walks};
use crate::domain::scene::character::param::{
    BODY_SHARE, CATCHUP_DURATION_S, CLIP_FADE_S, FOLLOW_WALK_LINGER_S, HEAD_CLEARANCE_U, PET_OFF,
    SETTLE_DELAY_S, WALK_EPS,
};
use crate::domain::scene::character::{Character, Group};
use crate::domain::scene::frame::Frame;
use crate::domain::scene::model::registry::Models;
use crate::domain::scene::model::{draw_list, models};
use crate::domain::scene::transform::Transform;
use crate::domain::smooth::smoothing_factor;
use crate::domain::view::frame::coord::{to_world, world_to_canonical};
use crate::domain::view::setting::store::settings_store;
use crate::sys::sched::edge::wake_at;
use crate::sys::sched::tick::TickContext;
use pxl_render::gpu::api::Image;
use pxl_render::gpu::draw::{Draw, DrawList};
use pxl_spec::asset::character::IDLE_CLIP;
use pxl_spec::asset::marker::marker_shows;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::layer::lattice::Cells;
use pxl_spec::space::layer::lattice::GROUND;
use xpute_core::clock;
use xpute_core::global::Global;

// The world scroll's own rate: the two cancel, holding the character at
// screen center, so this is the only position smoothing while following.
const FOLLOW_LERP_K: f64 = param::POS_DAMP;

// +PI turns a model whose forward is -Y into the screen, back to the camera.
const FOLLOW_FACING: f64 = core::f64::consts::PI;

/// Turned against the azimuth, which is the screen's up laid on the ground.
fn back_to_camera(azimuth: f64) -> f64 {
    FOLLOW_FACING - azimuth
}

/// Radians, 0 = +x.
pub fn heading_to(from: [f64; 2], to: [f64; 2]) -> f64 {
    libm::atan2(to[1] - from[1], to[0] - from[0])
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WalkPlan {
    pub start: [f64; 2],
    pub target: [f64; 2],
    pub start_time: f64,
    pub duration: f64,
}

impl WalkPlan {
    /// Where it is at `now`, and its heading, none once arrived. It takes
    /// `duration` whatever the distance, so the person and the pet catch up
    /// on the same beat after a pan of 1 unit or 1000.
    pub fn at(&self, now: f64) -> ([f64; 2], Option<f64>) {
        let frac = if self.duration > 0.0 {
            ((now - self.start_time) / self.duration).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let at = std::array::from_fn(|k| self.start[k] + (self.target[k] - self.start[k]) * frac);
        (
            at,
            (frac < 1.0).then(|| heading_to(self.start, self.target)),
        )
    }
}

/// Settles when the target stops changing, whatever moved it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettleTracker {
    pub last_seen: [f64; 2],
    pub last_change_time: f64,
    pub settled_at: [f64; 2],
}

impl SettleTracker {
    pub fn new(at: [f64; 2], now: f64) -> SettleTracker {
        SettleTracker {
            last_seen: at,
            last_change_time: now,
            settled_at: at,
        }
    }

    /// Whether it settled just now.
    pub fn update(&mut self, target: [f64; 2], now: f64, delay: f64) -> bool {
        if target != self.last_seen {
            (self.last_seen, self.last_change_time) = (target, now);
        }
        let settles = now - self.last_change_time >= delay && self.last_seen != self.settled_at;
        if settles {
            self.settled_at = self.last_seen;
        }
        settles
    }
}

/// What the companion's walk reads of the camera each turn, in the world's
/// doubles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Where its pivot is going.
    pub pivot: [f64; 2],
    /// The top of the floor the pivot is drawn on; none while nothing is
    /// known there, when the person and the pet hold where they are.
    pub floor: Option<f64>,
    /// The camera's heading, which the person and the pet turn their backs to.
    pub azimuth: f64,
    /// Whether the camera is being dragged, whose release ends a catch-up.
    pub dragging: bool,
    /// Whether the keys steer it, and whether the map follows the device.
    pub steered: bool,
    pub following: bool,
    /// The eye, which decides whether it walks or the world is a map, and
    /// what it is drawn standing on.
    pub eye: Q3,
}

#[derive(Default)]
struct Companion {
    /// Its person and its pet, by their group's number, each once it has
    /// landed.
    characters: [Option<Character>; Group::ALL.len()],
    /// The floor the pivot is drawn on, of the pivot (`pivot_floor`).
    pivot_floor: Memo<Q3, Option<f64>>,
    /// The pet's floor beside the person, walked to from the person's: of
    /// the pivot and its floor, the pet's target and whether the eye sees a
    /// walk there (`walks`).
    pet_floor: Memo<([f64; 2], f64, [f64; 2], bool), f64>,
}

static COMPANION: Global<Companion> = Global::new();

fn companion() -> &'static mut Companion {
    COMPANION.get(Companion::default)
}

/// The floor the pivot `goal` is drawn on: `ask`'s of the world, kept while
/// the pivot stands and no change touched what it read (`Memo`), as the
/// companion's own floors are.
pub fn pivot_floor(goal: Q3, ask: impl FnOnce(World<'_>) -> Option<f64>) -> Option<f64> {
    let c = companion();
    c.pivot_floor.forget_touched();
    c.pivot_floor.of(World::now(), goal, ask)
}

/// The companion's person and pet, each the row its setting names, a
/// character of another row let go for it; each walked to the pivot, or
/// none placed while the eye is the pivot (`camera` none).
pub fn companion_step(ctx: &TickContext, camera: Option<Camera>) {
    let s = companion();
    s.pet_floor.forget_touched();
    // Looked at each frame turn, a pivot not asked of this one too.
    s.pivot_floor.forget_touched();
    let settings = settings_store();
    for (slot, (group, number)) in s
        .characters
        .iter_mut()
        .zip([(Group::person, settings.person), (Group::pet, settings.pet)])
    {
        if slot.as_ref().is_some_and(|c| c.number != number) {
            if let Some(c) = slot.take() {
                c.free(draw_list());
            }
        }
        if slot.is_none() {
            *slot = Character::of(group, number);
        }
        if let Some(c) = slot {
            c.walk.drawn.forget_touched();
        }
    }
    let [person, pet] = &mut s.characters;
    let Some(camera) = camera else {
        for c in [person, pet].into_iter().flatten() {
            (c.transform, c.walk) = (None, Walk::new(c.walk.at));
        }
        return;
    };
    // While nothing is known under the pivot each holds where it is, and
    // the rest of it steps on.
    let hold = |c: &Character| c.walk.stands_z.map(|z| (c.walk.at, z));
    if let Some(person) = person {
        if let Some((target, z)) = camera
            .floor
            .map(|z| (camera.pivot, z))
            .or_else(|| hold(person))
        {
            person.step(ctx, target, z, &camera);
        }
    }
    let Some(pet) = pet else { return };
    let target = [camera.pivot[0] + PET_OFF[0], camera.pivot[1] + PET_OFF[1]];
    // Its own floor beside the person, walked to from the person.
    let reached = |(beside, _): &([f64; 2], f64)| {
        libm::hypot(beside[0] - target[0], beside[1] - target[1]) <= WALK_EPS
    };
    let from = |floor: f64| world_to_canonical(&[camera.pivot[0], camera.pivot[1], over(floor)]);
    let pet_floor = &mut s.pet_floor;
    let mut walked = |floor: f64| {
        let of = (
            camera.pivot,
            floor,
            target,
            walks(&from(floor), &camera.eye),
        );
        pet_floor.of(World::now(), of, |world| {
            walk_to(world, camera.pivot, floor, target, &camera)
                .filter(reached)
                .map_or(floor, |(_, z)| z)
        })
    };
    if let Some((target, z)) = camera
        .floor
        .map(|z| (target, walked(z)))
        .or_else(|| hold(pet))
    {
        pet.step(ctx, target, z, &camera);
    }
}

impl Character {
    /// Walked toward `target`, standing at `arrive_z` there, and stood where
    /// its walk ends, playing its walk clip while it walks; a walk begun
    /// afresh where it stood nowhere.
    fn step(&mut self, ctx: &TickContext, target: [f64; 2], arrive_z: f64, camera: &Camera) {
        if self.transform.is_none() {
            self.walk = Walk::new(target);
        }
        let half = BODY_SHARE * self.height_u();
        let (at, rot, walking) = self.walk.step(ctx, target, arrive_z, half, camera);
        // Its model as registered, the character's height tall.
        self.transform = Some(Transform {
            at,
            rot,
            scale: 1.0,
        });
        let clip = if walking { self.move_clip() } else { IDLE_CLIP };
        self.play(models(), clip, CLIP_FADE_S);
    }
}

/// Where a walk is drawn, kept by where it stands, its floor there and the
/// zoom the eye draws it at.
type Drawn = Memo<([f64; 2], f64, Zoom), ([f64; 2], f64)>;

/// A character's walk: where it is, its heading, its plan and the floor it
/// stands on.
pub struct Walk {
    at: [f64; 2],
    heading: f64,
    was_following: bool,
    walk_plan: Option<WalkPlan>,
    /// Its falling edge, a gesture's end, fixes the destination.
    was_dragging: bool,
    settle: SettleTracker,
    init_facing_done: bool,
    follow_walk_until: f64,
    stands_z: Option<f64>,
    /// Where it is drawn and on what floor (`drawn_at`).
    drawn: Drawn,
}

impl Walk {
    /// At `target`, standing nowhere yet.
    pub fn new(target: [f64; 2]) -> Walk {
        Walk {
            at: target,
            heading: 0.0,
            was_following: false,
            walk_plan: None,
            was_dragging: false,
            settle: SettleTracker::new(target, 0.0),
            init_facing_done: false,
            follow_walk_until: 0.0,
            stands_z: None,
            drawn: Memo::NONE,
        }
    }

    /// Toward `target`, standing at `arrive_z` there: where it is drawn, a
    /// body `half` wide each way, its turn about Z and whether it walks.
    /// `follows`: the target is followed
    /// as it moves, by a fix or by the keys, rather than walked to once it
    /// settles; a steered one faces its way.
    fn step(
        &mut self,
        ctx: &TickContext,
        target: [f64; 2],
        arrive_z: f64,
        half: f64,
        camera: &Camera,
    ) -> ([f64; 3], f32, bool) {
        let (follows, steered) = (camera.following || camera.steered, camera.steered);
        // Wall seconds: the tick's elapsed is clamped per delta, which stretches
        // these spans across a gap where the world did not tick.
        let now = clock::now() / 1000.0;
        let dragging = camera.dragging;
        let released = self.was_dragging && !dragging;
        let gap = |a: [f64; 2], b: [f64; 2]| libm::hypot(a[0] - b[0], a[1] - b[1]);
        let (mut at, is_walking);

        if follows {
            // From where it stands, as the camera eases from where it is: a key moves
            // the target a step ahead at once, and a jump there started it off apart.
            let a = smoothing_factor(FOLLOW_LERP_K, ctx.delta);
            at = std::array::from_fn(|k| self.at[k] + (target[k] - self.at[k]) * a);
            if !steered {
                self.heading = back_to_camera(camera.azimuth);
            } else if gap(target, self.at) > WALK_EPS {
                self.heading = heading_to(self.at, target) + core::f64::consts::PI / 2.0;
            }
            if gap(target, at) > WALK_EPS {
                self.follow_walk_until = now + FOLLOW_WALK_LINGER_S;
            }
            is_walking = now < self.follow_walk_until;
            self.walk_plan = None;
        } else {
            // Rebase the tracker, or the delta accumulated while following reads as one
            // giant dash.
            if self.was_following {
                self.settle = SettleTracker::new(target, now);
                self.walk_plan = None;
            }
            let just_settled = self.settle.update(target, now, SETTLE_DELAY_S);
            if self.settle.last_seen != self.settle.settled_at {
                wake_at((self.settle.last_change_time + SETTLE_DELAY_S) * 1000.0);
            }
            // A release fixes the destination at once: a pan begun inside the settle
            // delay holds the target moving, so waiting would plan nothing. The settle
            // tracker covers every other way the pivot moves.
            let dst = if released {
                self.settle = SettleTracker::new(target, now);
                Some(target)
            } else if just_settled {
                Some(self.settle.settled_at)
            } else {
                None
            };
            if let Some(to) = dst {
                // Re-planning toward the same place would restart the clock and slow the
                // walk to a crawl.
                let under_way = self
                    .walk_plan
                    .as_ref()
                    .is_some_and(|plan| gap(plan.target, to) <= WALK_EPS);
                if !under_way && gap(to, self.at) > WALK_EPS {
                    self.walk_plan = Some(WalkPlan {
                        start: self.at,
                        target: to,
                        start_time: now,
                        duration: CATCHUP_DURATION_S,
                    });
                }
            }
            // A plan in flight finishes even once the pivot moves again.
            let (step, heading) = self
                .walk_plan
                .as_ref()
                .map_or((self.at, None), |plan| plan.at(now));
            if heading.is_none() {
                self.walk_plan = None;
            }
            at = step;
            is_walking = heading.is_some();

            // The character's Y-up to Z-up turn maps the model's forward to +Y, hence +PI/2.
            if let Some(heading) = heading {
                self.heading = heading + core::f64::consts::PI / 2.0;
            } else if !self.init_facing_done {
                self.heading = back_to_camera(camera.azimuth);
                self.init_facing_done = true;
            }
        }

        self.was_following = follows;
        self.was_dragging = dragging;

        // Following there is no end to wait for, so a wall its way meets puts it
        // at its target at once.
        let arrived = gap(target, at) <= WALK_EPS;
        let moved = at != self.at;
        let stands = self.stands_z.get_or_insert(arrive_z);
        if arrived {
            *stands = arrive_z;
        } else if moved {
            // Where the leaf under it, or a map's `to`, is not held, it goes on at its height.
            if let Some((to, z)) = walk_to(World::now(), self.at, *stands, at, camera) {
                (at, *stands) = if follows && gap(to, at) > WALK_EPS {
                    (target, arrive_z)
                } else {
                    (to, z)
                };
            }
        }
        self.at = at;
        let stands = *stands;
        (
            drawn_at(&mut self.drawn, at, stands, half, camera),
            self.heading as f32,
            is_walking,
        )
    }
}

/// `world`'s step-slide from `from`, its floor at `stands`, toward `to`:
/// where it ends and its floor there, short of a leaf on the way not held;
/// none where the leaf under `from`, or a map's `to`, is not held.
fn walk_to(
    world: World<'_>,
    from: [f64; 2],
    stands: f64,
    to: [f64; 2],
    camera: &Camera,
) -> Option<([f64; 2], f64)> {
    let [a, b] = [from, to].map(|[x, y]| world_to_canonical(&[x, y, over(stands)]));
    let at = to_world(&world.step_slide(&a, &b, &camera.eye, true, true)?);
    Some(([at[0], at[1]], under(at[2])))
}

/// Where a body `half` wide each way is drawn for `at`, and the floor `stands`
/// as the eye's rays draw it there: the world asked at the zoom the camera grid
/// takes there from the eye, kept in `asked` while none of it moves. A coarser
/// answer is taken where that zoom is not held, as a ray draws the ground it
/// has.
fn drawn_at(asked: &mut Drawn, at: [f64; 2], stands: f64, half: f64, camera: &Camera) -> [f64; 3] {
    let center = [at[0], at[1], over(stands)];
    let eye = to_world(&camera.eye);
    let r2 = (0..3)
        .map(|k| (center[k] - eye[k]) * (center[k] - eye[k]))
        .sum();
    let zoom = GROUND.z_at(r2);
    let ([x, y], floor) = asked.of(World::now(), (at, stands, zoom), |world| {
        let floor_at = |[x, y]: [f64; 2]| {
            world
                .floor_under(&world_to_canonical(&[x, y, center[2]]), zoom)
                .val()
                .flatten()
        };
        let floor = floor_at(at).unwrap_or(stands);
        let rises = |p| floor_at(p).is_some_and(|top| top > floor);
        (clear_of_walls(at, half, units(zoom), rises), floor)
    });
    [x, y, floor]
}

/// `at` moved off each wall a body `half` wide each way would stand in, the
/// cells `cell` wide: a wall is the near side of a cell that `rises` over the
/// floor, so what stands beside a step is drawn beside it, not through it.
pub(crate) fn clear_of_walls(
    at: [f64; 2],
    half: f64,
    cell: f64,
    rises: impl Fn([f64; 2]) -> bool,
) -> [f64; 2] {
    let mut clear = at;
    for k in 0..2 {
        for side in [-1.0, 1.0] {
            let mut edge = at;
            edge[k] += side * half;
            if rises(edge) {
                let near = libm::floor(edge[k] / cell) + if side < 0.0 { 1.0 } else { 0.0 };
                clear[k] = near * cell - side * half;
            }
        }
    }
    clear
}

/// Where the person's line hangs, over its head, while the person shows it,
/// seen from `eye`: while the walk stands it, and where the camera grid's
/// cell at its feet is one a marker shows at (`marker_shows`), so a line
/// reads where the markers about it do.
pub fn line_hangs(eye: V3) -> Option<Q3> {
    let person = companion().characters[Group::person as usize].as_ref()?;
    let feet = person.transform?.at;
    let over = [
        feet[0],
        feet[1],
        feet[2] + person.height_u() + HEAD_CLEARANCE_U,
    ];
    marker_shows(cell_zoom_at(eye, feet)).then(|| world_to_canonical(&over))
}

/// (model, image handle) for the world's skinned pass: the image as the
/// loader has it now, which a lost device takes back until it lands again.
pub fn companion_skins() -> impl Iterator<Item = (u32, Image)> {
    let c: &'static _ = companion();
    c.characters
        .iter()
        .flatten()
        .filter_map(|c| Some((c.model, c.skin()?)))
}

/// Whether the person, then the pet, has landed, for the loading toast.
pub fn companion_ready() -> [bool; 2] {
    companion().characters.each_ref().map(Option::is_some)
}

/// Each of its characters advanced; whether one a view can see moved
/// (`Character::tick`), a surface showing the person, the pet or both
/// (`on_surface`, by their group's number).
pub fn companion_animate(
    draw: &mut DrawList,
    models: &Models,
    frame: &Frame,
    on_surface: [bool; 2],
) -> bool {
    let mut moved = false;
    for (c, on_surface) in companion().characters.iter_mut().zip(on_surface) {
        if let Some(c) = c {
            moved |= c.tick(draw, models, frame, on_surface);
        }
    }
    moved
}

/// Each of its characters into the world's list (`Character::draw`).
pub fn companion_draw(draw: &mut DrawList, models: &Models) {
    for c in companion().characters.iter().flatten() {
        c.draw(draw, models);
    }
}

/// Its character of `group` as a character surface draws it, and its skin, the image
/// as the loader has it now; none while either is not in.
pub fn companion_surface_draw(group: Group) -> Option<(Draw, Image)> {
    let c = companion().characters[group as usize].as_ref()?;
    Some((c.surface_draw(models())?, c.skin()?))
}
