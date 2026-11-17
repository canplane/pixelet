// pxl-core/domain/scene/character/companion.rs

//! The companion: the person and the pet the player chose, and how each walks
//! to the pivot. When the pivot comes to rest, each makes a catch-up walk of
//! fixed duration to it. When the pivot never rests, because the map follows
//! a GPS stream of about one fix a second or the keys steer it, each instead
//! glides at the world's scroll rate; the two motions cancel, and the
//! character holds at the screen's center. Neither is anchored to the other:
//! the pet walks to its own place beside the person's. While the eye is the
//! pivot neither is placed, and each is placed again at its target once the
//! eye leaves it. Each is drawn clear of the wall beside it
//! (`clear_of_walls`). The models are Kenney's "Mini Characters" and "Cube
//! Pets", CC0.

use crate::domain::param;
use crate::domain::query::World;
use crate::domain::query::change::Memo;
use crate::domain::query::{cell_zoom_at, floor_of, is_unknown, over};
use crate::domain::scene::character::param::{
    BODY_SHARE, CATCHUP_DURATION_S, CLIP_FADE_S, FOLLOW_WALK_LINGER_S, HEAD_CLEARANCE_U, PET_OFF,
    SETTLE_DELAY_S, WALK_EPS,
};
use crate::domain::scene::character::{Character, Group};
use crate::domain::scene::frame::Frame;
use crate::domain::scene::model::registry::ModelTable;
use crate::domain::scene::model::{draw_list, models};
use crate::domain::scene::transform::Transform;
use crate::domain::setting::settings;
use crate::domain::smooth::smoothing_factor;
use crate::domain::view::frame::coord::{to_world, world_to_canonical};
use crate::spec::step::InvalidationReason;
use crate::sys::sched::invalidate_at;
use crate::sys::sched::tick::TickContext;
use pxl_global::Global;
use pxl_render::gpu::api::Image;
use pxl_render::gpu::draw::{Draw, DrawList};
use pxl_spec::asset::character::IDLE_CLIP;
use pxl_spec::asset::marker::marker_shows;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::{Zoom, units};
use pxl_spec::space::layer::lattice::{GROUND, Grid};
use xpute_guest::clock;

// The world's scroll rate. A character that glides at it cancels the scroll
// and holds at the screen's center, so this is the only smoothing of its
// position while following.
const FOLLOW_LERP_K: f64 = param::POS_DAMP;

// A turn of PI faces a model whose forward is -Y into the screen, with its
// back to the camera.
const FOLLOW_FACING: f64 = core::f64::consts::PI;

/// The heading that turns a character's back to the camera. The azimuth is
/// the screen's up laid on the ground.
fn back_to_camera(azimuth: f64) -> f64 {
    FOLLOW_FACING - azimuth
}

/// The heading from `from` to `to`, in radians, where 0 points along +x.
pub(crate) fn heading_to(from: [f64; 2], to: [f64; 2]) -> f64 {
    libm::atan2(to[1] - from[1], to[0] - from[0])
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WalkPlan {
    pub start: [f64; 2],
    pub target: [f64; 2],
    pub start_time: f64,
    pub duration: f64,
}

impl WalkPlan {
    /// Where the walk is at `now`, and its heading, which is None once it has
    /// arrived. A walk takes `duration` whatever the distance, so the person
    /// and the pet catch up on the same beat after a pan of 1 unit or of 1000.
    pub(crate) fn at(&self, now: f64) -> ([f64; 2], Option<f64>) {
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

/// A target settles once it has not changed for a delay, whatever moved it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SettleTracker {
    pub last_seen: [f64; 2],
    pub last_change_time: f64,
    pub settled_at: [f64; 2],
}

impl SettleTracker {
    pub(crate) fn new(at: [f64; 2], now: f64) -> SettleTracker {
        SettleTracker {
            last_seen: at,
            last_change_time: now,
            settled_at: at,
        }
    }

    /// Notes `target` at `now`, and returns whether the target settled on
    /// this call, having stood still for `delay`.
    pub(crate) fn update(&mut self, target: [f64; 2], now: f64, delay: f64) -> bool {
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

/// What the companion's walk reads of the camera each frame, in world
/// coordinates as f64.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Camera {
    /// Where the camera's pivot is going.
    pub pivot: [f64; 2],
    /// The top of the floor the pivot is drawn on. It is None while nothing
    /// is known there, and then the person and the pet hold where they are.
    pub floor: Option<f64>,
    /// The camera's heading, which the person and the pet turn their backs to.
    pub azimuth: f64,
    /// Whether the camera is being dragged. The release fixes where a
    /// catch-up walks to.
    pub dragging: bool,
    /// Whether the keys steer the camera.
    pub steered: bool,
    /// Whether the map follows the device.
    pub following: bool,
    /// The eye, which decides what a character is drawn standing on.
    pub eye: Q3,
}

#[derive(Default)]
struct Companion {
    /// The person and the pet, indexed by their group's number, each once it
    /// has loaded.
    characters: [Option<Character>; Group::ALL.len()],
    /// Each character's walk to the pivot, by the same index, made with its
    /// character. A character that has no place starts a fresh one
    /// (`Character::step`).
    walks: [Walk; Group::ALL.len()],
    /// The floor the pivot is drawn on, kept by the pivot (`pivot_floor`).
    pivot_floor: Memo<Q3, Option<f64>>,
    /// The pet's floor beside the person, found by walking there from the
    /// person's floor. It is kept by the pivot and its floor, and the pet's
    /// target.
    pet_floor: Memo<([f64; 2], f64, [f64; 2]), f64>,
}

static COMPANION: Global<Companion> = Global::new();

fn companion() -> &'static mut Companion {
    COMPANION.get(Companion::default)
}

/// The floor the pivot `goal` is drawn on, as `ask` finds it in the world.
/// The answer is kept while the pivot stands and no change touches what it
/// read (`Memo`), as the companion's own floors are.
pub(crate) fn pivot_floor(goal: Q3, ask: impl FnOnce(World) -> Option<f64>) -> Option<f64> {
    let c = companion();
    c.pivot_floor.forget_touched();
    c.pivot_floor.of(goal, || ask(World::now()))
}

/// Steps the person and the pet. Each is the row its setting names, and a
/// character of another row is let go for it. Each walks to the pivot, and
/// neither is placed while the eye is the pivot (`camera` is None).
pub(crate) fn companion_step(ctx: &TickContext, camera: Option<Camera>) {
    let s = companion();
    s.pet_floor.forget_touched();
    // A memo must see every change once, so the pivot's floor checks for
    // touched columns on every frame, also on one that does not ask it.
    s.pivot_floor.forget_touched();
    let settings = settings();
    for ((slot, walk), (group, number)) in s
        .characters
        .iter_mut()
        .zip(&mut s.walks)
        .zip([(Group::Person, settings.person), (Group::Pet, settings.pet)])
    {
        // A walk lives as long as its character.
        if slot.as_ref().is_some_and(|c| c.number != number)
            && let Some(c) = slot.take()
        {
            c.free(draw_list());
            *walk = Walk::default();
        }
        if slot.is_none() {
            *slot = Character::of(group, number);
        }
    }
    for w in &mut s.walks {
        w.floor_memo.forget_touched();
    }
    let [person, pet] = &mut s.characters;
    let [person_walk, pet_walk] = &mut s.walks;
    let Some(camera) = camera else {
        for (c, w) in [(person, person_walk), (pet, pet_walk)] {
            if let Some(c) = c {
                (c.transform, *w) = (None, Walk::new(w.at));
            }
        }
        return;
    };
    // While nothing is known under the pivot, each character with a height
    // holds where it is, and the rest of its step goes on.
    let hold = |w: &Walk| w.floor(World::now()).map(|z| (w.at, z));
    if let Some(person) = person
        && let Some((target, z)) = camera
            .floor
            .map(|z| (camera.pivot, z))
            .or_else(|| hold(person_walk))
    {
        person.step(person_walk, ctx, target, z, &camera);
    }
    let Some(pet) = pet else { return };
    let target = [camera.pivot[0] + PET_OFF[0], camera.pivot[1] + PET_OFF[1]];
    // The pet finds its own floor beside the person by walking there from
    // the person.
    let reached = |[x, y, _]: [f64; 3]| libm::hypot(x - target[0], y - target[1]) <= WALK_EPS;
    let pet_floor = &mut s.pet_floor;
    let mut walked = |floor: f64| {
        let from = center(camera.pivot, floor);
        let world = World::now();
        pet_floor.of((camera.pivot, floor, target), || {
            let end = walk_to(world, &from, target);
            match reached(to_world(&end)) && !is_unknown(&end) {
                true => floor_of(&end),
                false => floor,
            }
        })
    };
    if let Some((target, z)) = camera
        .floor
        .map(|z| (target, walked(z)))
        .or_else(|| hold(pet_walk))
    {
        pet.step(pet_walk, ctx, target, z, &camera);
    }
}

impl Character {
    /// Walks the character by `walk` toward `target`, where it will stand at
    /// `arrive_floor`, places it where the walk ends, and plays its walk clip
    /// while it walks. A character that had no place starts a fresh walk at
    /// `target`.
    fn step(
        &mut self,
        walk: &mut Walk,
        ctx: &TickContext,
        target: [f64; 2],
        arrive_floor: f64,
        camera: &Camera,
    ) {
        if self.transform.is_none() {
            *walk = Walk::new(target);
        }
        let half = BODY_SHARE * self.height_u();
        let (at, rot, walking) = walk.step(ctx, target, arrive_floor, half, camera);
        // Scale 1 draws the model as registered, which is the character's
        // height.
        self.transform = Some(Transform {
            at,
            rot,
            scale: 1.0,
        });
        let clip = if walking { self.move_clip() } else { IDLE_CLIP };
        self.play(models(), clip, CLIP_FADE_S);
    }
}

/// Where a walk is drawn, kept by where it stands, its floor there, and the
/// zoom the eye draws it at.
type FloorMemo = Memo<([f64; 2], f64, Zoom), ([f64; 2], f64)>;

struct Walk {
    at: [f64; 2],
    heading: f64,
    was_following: bool,
    walk_plan: Option<WalkPlan>,
    /// Whether the camera was dragged at the last step. Its falling edge, the
    /// end of a gesture, fixes the destination.
    was_dragging: bool,
    settle: SettleTracker,
    init_facing_done: bool,
    follow_walk_until: f64,
    /// Where it stands, its height unknown as the pivot's can be. None until
    /// it first has a place.
    stands: Option<Q3>,
    /// Where the character is drawn and on what floor (`drawn_at`).
    floor_memo: FloorMemo,
}

impl Default for Walk {
    fn default() -> Walk {
        Walk::new([0.0; 2])
    }
}

impl Walk {
    /// A walk at `target` that stands on no floor yet.
    fn new(target: [f64; 2]) -> Walk {
        Walk {
            at: target,
            heading: 0.0,
            was_following: false,
            walk_plan: None,
            was_dragging: false,
            settle: SettleTracker::new(target, 0.0),
            init_facing_done: false,
            follow_walk_until: 0.0,
            stands: None,
            floor_memo: Memo::NONE,
        }
    }

    /// The floor it is drawn on: its own, or where its height is not known,
    /// that of the finest ground known there, as the pivot's is.
    fn floor(&self, world: World) -> Option<f64> {
        world
            .stands(&self.stands?)
            .val()
            .flatten()
            .map(|at| floor_of(&at))
    }

    /// Steps the walk toward `target`, where it will stand at `arrive_floor`. It
    /// returns where a body `half` wide on each side is drawn, its turn about
    /// Z, and whether it walks. While the map follows a fix or the keys steer
    /// (`follows`), the target is followed as it moves rather than walked to
    /// once it settles, and a steered character faces the way it goes.
    fn step(
        &mut self,
        ctx: &TickContext,
        target: [f64; 2],
        arrive_floor: f64,
        half: f64,
        camera: &Camera,
    ) -> ([f64; 3], f32, bool) {
        let (follows, steered) = (camera.following || camera.steered, camera.steered);
        // Wall-clock seconds. The tick's elapsed time is clamped each delta,
        // which would stretch these spans across a gap in which the world did
        // not tick.
        let now = clock::now() / 1000.0;
        let dragging = camera.dragging;
        let released = self.was_dragging && !dragging;
        let gap = |a: [f64; 2], b: [f64; 2]| libm::hypot(a[0] - b[0], a[1] - b[1]);
        let (mut at, is_walking);

        if follows {
            // The character eases from where it stands, as the camera eases
            // from where it is: a key moves the target a step ahead at once,
            // and a jump starts the two far apart.
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
            // The tracker starts anew, or the distance covered while following
            // would read as one long dash.
            if self.was_following {
                self.settle = SettleTracker::new(target, now);
                self.walk_plan = None;
            }
            let just_settled = self.settle.update(target, now, SETTLE_DELAY_S);
            if self.settle.last_seen != self.settle.settled_at {
                invalidate_at(
                    (self.settle.last_change_time + SETTLE_DELAY_S) * 1000.0,
                    InvalidationReason::Pending,
                );
            }
            // A release fixes the destination at once. A pan begun within the
            // settle delay keeps the target moving, so waiting for it to settle
            // would plan nothing. The settle tracker covers every other way the
            // pivot moves.
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

            // The turn from the model's Y-up to the world's Z-up maps the
            // model's forward to +Y, so a heading takes PI/2 more.
            if let Some(heading) = heading {
                self.heading = heading + core::f64::consts::PI / 2.0;
            } else if !self.init_facing_done {
                self.heading = back_to_camera(camera.azimuth);
                self.init_facing_done = true;
            }
        }

        self.was_following = follows;
        self.was_dragging = dragging;

        // While following there is no end to wait for, so a wall in the
        // character's way puts it at its target at once.
        let arrived = gap(target, at) <= WALK_EPS;
        let moved = at != self.at;
        let stands = self.stands.get_or_insert(center(self.at, arrive_floor));
        if arrived {
            *stands = center(at, arrive_floor);
        } else if moved {
            let end = walk_to(World::now(), stands, at);
            let [x, y, _] = to_world(&end);
            (at, *stands) = if follows && gap([x, y], at) > WALK_EPS {
                (target, center(target, arrive_floor))
            } else {
                ([x, y], end)
            };
        }
        self.at = at;
        let floor = self.floor(World::now()).unwrap_or(arrive_floor);
        (
            drawn_at(&mut self.floor_memo, at, floor, half, camera),
            self.heading as f32,
            is_walking,
        )
    }
}

/// A walker's center at `at` on a floor whose top is `floor`.
fn center([x, y]: [f64; 2], floor: f64) -> Q3 {
    world_to_canonical(&[x, y, over(floor)])
}

/// Slides a step through `world` from `from` toward `to` (`World::step_slide`).
fn walk_to(world: World, from: &Q3, [x, y]: [f64; 2]) -> Q3 {
    world.step_slide(from, &world_to_canonical(&[x, y, 0.0]), true)
}

/// Where a body `half` wide on each side is drawn for `at`, on the floor the
/// eye's rays draw there in place of `floor`. The world is asked at the zoom
/// the camera grid takes at that distance from the eye, and the answer is
/// kept in `memo` while none of its inputs moves. Where that zoom is not
/// loaded a coarser answer is taken, as a ray draws whatever ground it has.
pub(crate) fn drawn_at(
    memo: &mut FloorMemo,
    at: [f64; 2],
    floor: f64,
    half: f64,
    camera: &Camera,
) -> [f64; 3] {
    let pos = [at[0], at[1], over(floor)];
    let eye = to_world(&camera.eye);
    let r2 = (0..3).map(|k| (pos[k] - eye[k]) * (pos[k] - eye[k])).sum();
    let zoom = GROUND.z_at(r2);
    let world = World::now();
    let ([x, y], drawn) = memo.of((at, floor, zoom), || {
        let floor_at = |[x, y]: [f64; 2]| {
            world
                .floor_under(&world_to_canonical(&[x, y, pos[2]]), zoom)
                .val()
                .flatten()
        };
        let drawn = floor_at(at).unwrap_or(floor);
        let rises = |p| floor_at(p).is_some_and(|top| top > drawn);
        (
            clear_of_walls(at, half, units(GROUND.cell_zoom(zoom)), rises),
            drawn,
        )
    });
    [x, y, drawn]
}

/// Moves `at` off each wall that a body `half` wide on each side would stand
/// in, on cells `cell` wide. A wall is the near side of a cell that `rises`
/// over the floor, so a character beside a step is drawn beside it, not
/// through it.
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

/// Where the person's line hangs over its head, seen from `eye`. A line
/// hangs only while the walk places the person, and only where the camera
/// grid's cell at its feet is one a marker shows at (`marker_shows`), so the
/// line reads where the markers about it do.
pub(crate) fn line_hangs(eye: V3) -> Option<Q3> {
    let person = companion().characters[Group::Person as usize].as_ref()?;
    let feet = person.transform?.at;
    let over = [
        feet[0],
        feet[1],
        feet[2] + person.height_u() + HEAD_CLEARANCE_U,
    ];
    marker_shows(cell_zoom_at(eye, feet)).then(|| world_to_canonical(&over))
}

/// Each character's model and skin image for the world's skinned pass. The
/// image is as the loader has it now: a lost device takes it back until it
/// loads again.
pub(crate) fn companion_skins() -> impl Iterator<Item = (u32, Image)> {
    let c: &'static _ = companion();
    c.characters
        .iter()
        .flatten()
        .filter_map(|c| Some((c.model, c.skin()?)))
}

/// Whether the person and the pet have both loaded, for the loading toast.
pub(crate) fn companion_ready() -> bool {
    companion().characters.iter().all(Option::is_some)
}

/// Advances each character and returns whether a frame must be asked for
/// (`Character::tick`). `on_surface`, indexed by group number, says whether a
/// surface shows the person and whether one shows the pet.
pub(crate) fn companion_animate(
    draw: &mut DrawList,
    models: &ModelTable,
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

pub(crate) fn companion_draw(draw: &mut DrawList, models: &ModelTable) {
    for c in companion().characters.iter().flatten() {
        c.draw(draw, models);
    }
}

/// The draw of the character of `group` for a character surface, and its
/// skin as the loader has it now. None while either has not loaded.
pub(crate) fn companion_surface_draw(group: Group) -> Option<(Draw, Image)> {
    let c = companion().characters[group as usize].as_ref()?;
    Some((c.surface_draw(models())?, c.skin()?))
}
