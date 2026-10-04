// pxl-kernel/layer/character/companion.rs

//! A companion (person + pet) that sits at map_center, its back to the camera
//! and looking where the screen looks, until it has somewhere to walk to. Whenever map_center stops changing — after a
//! drag, a click-to-navigate tween, a GPS recenter, whatever moved it — it
//! makes a fixed-duration catch-up walk there and ends up facing its direction
//! of travel, which it then just keeps. While map_center is still actively
//! changing it stays put. On the way it steps as a walker does, from where
//! it stood — down to the floor beside, up a step, held walking in place by
//! a wall — and come to map_center it takes map_center's height:
//! map_center is the character's place, and the walk only shows it getting
//! there.
//!
//! Follow mode (is_follow_mode) is a different regime: map_center is a live
//! ~1 Hz GPS stream, so the settle-then-dash above never gets to rest and its
//! fixed dash curve fights the world-scroll damp. There the walker instead
//! glides toward the target every frame at the world-scroll rate (the two
//! cancel, holding it at screen center) and keeps its back to the camera by
//! counter-rotating azimuth — a nav-app "you" marker, map spinning under it.
//!
//! Position/facing math lives in path.rs; this file reads that math and tells
//! the scheduler's actor where to stand. The model is registered once per
//! variant (layer/model/asset.rs) and shared with the settings variant
//! picker's preview.
//!
//! Assets: Kenney's "Mini Characters" (person) and "Cube Pets" (pet) packs,
//! both CC0, both real glTF.

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;

use crate::control::camera::approach::damp_alpha;
use crate::control::camera::config::camera_cfg;
use crate::control::camera::store::{camera_store, CameraStatus};
use crate::control::navigation::support::over;
use crate::param::COMPANION_STEP_CELLS;
use crate::global::Global;
use crate::io::load::{load_gltf, load_image, Load};
use crate::kernel::{kernel, with_world};
use crate::layer::character::chat_bubble::chat_bubble_step;
use crate::layer::character::path::{SettleTracker, WalkPlan};
use crate::layer::character::store::companion_load_store;
use crate::layer::character::variant::{character_variant, model_of, CharacterVariant, Group};
use crate::layer::character::variant_spec::IDLE_CLIP;
use crate::layer::model::asset::{register_model, ModelHandle};
use crate::sched::tick::TickContext;
use crate::shell::setting::store::settings_store;
use crate::world::access::Access;
use crate::world::change::wake_at;
use crate::world::frame::coord::{local_height, local_world_to_canonical};

pub const ACTORS_MAX: usize = Group::ALL.len();

// Arbitrary, so the pet does not share the person's footprint.
const PET_OFFSET: [f64; 2] = [-0.6, -0.6];

// Catch-up always takes this long, whatever the distance — see
// sample_walk_plan for why a fixed duration beats a fixed speed here.
const CATCHUP_DURATION_S: f64 = 0.6;

// How long map_center has to sit still before we call it "settled" and plan a
// catch-up — see update_settle_tracker for why this reacts to the target
// itself rather than to any one specific gesture.
const SETTLE_DELAY_S: f64 = 0.15;

// Follow mode only: glides toward the target with a plain per-frame
// exponential lerp at the SAME rate the world scroll damps. Because the two run
// at the same rate toward the same point, they cancel: the character's world
// position stays at screen center. follow_mode.rs drops its own position
// EMA to match, so this lerp is the one and only position-smoothing stage.
const FOLLOW_LERP_K: f64 = camera_cfg::smooth::POS_DAMP;

// +PI turns a model whose forward is -Y into the screen, back to the camera.
const FOLLOW_FACING: f64 = core::f64::consts::PI;

/// Turned against the azimuth, which is the screen's up laid on the ground.
fn back_to_camera() -> f64 {
    FOLLOW_FACING - crate::control::camera::controller::azimuth()
}

// A gap narrower than this is not worth walking to: well under one GPS fix's
// worth of travel (~0.5u at a walking pace) and over the near-zero gap a map
// that has not moved leaves. Follow mode plays the walk clip while the glide
// has this much to close and holds it LINGER longer before dropping to idle,
// so it doesn't strobe once per fix; a catch-up is not planned at all across
// a gap this small, which a rotate or a pinch leaves — the walk clip would
// play in place and turn the walker to the heading of a zero-length step.
const WALK_EPS: f64 = 0.03;
const FOLLOW_WALK_LINGER_S: f64 = 1.5;

struct Held {
    actor: u32,
    model: ModelHandle,
    skin: u32,
}

struct Figure {
    variant: CharacterVariant,
    held: Option<Held>,
    at: [f64; 2],
    heading: f64,
    actor_clip: Option<u32>,
    was_follow_mode: bool,
    walk_plan: Option<WalkPlan>,
    /// The camera was under a pointer last turn: its falling edge is a
    /// gesture's end, which is when a destination is fixed.
    was_dragging: bool,
    settle: SettleTracker,
    /// Faces the camera exactly once — the moment it first has a pose to
    /// resolve the camera against — and never again.
    initial_facing_done: bool,
    follow_walk_until: f64,
    stands_z: Option<f64>,
}

impl Figure {
    fn new(variant: &CharacterVariant, target: [f64; 2]) -> Figure {
        Figure {
            variant: *variant,
            held: None,
            at: [0.0; 2],
            heading: 0.0,
            actor_clip: None,
            was_follow_mode: false,
            walk_plan: None,
            was_dragging: false,
            settle: SettleTracker::new(target, 0.0),
            initial_facing_done: false,
            follow_walk_until: 0.0,
            stands_z: None,
        }
    }

    fn set_ready(&self, ready: bool) {
        let store = companion_load_store();
        match self.variant.group {
            Group::person => store.person_ready = ready,
            Group::pet => store.pet_ready = ready,
        }
    }

    /// Placed at the target on creation; walking from the frame's origin would
    /// take far too long.
    fn acquire(&mut self, target: [f64; 2]) {
        let variant = self.variant;
        if self.held.is_some() {
            return;
        }
        let (Some(model), Some(skin)) = (model_of(variant.model, variant.height_u), load_image(Load::Asset(variant.skin))) else {
            return;
        };
        let Some(Some(actor)) = with_world(|_, sched| sched.models.new_actor(&mut sched.draw, model.model)) else {
            return;
        };
        self.held = Some(Held { actor, model, skin });
        self.at = target;
        self.set_ready(true);
    }

    fn release(&mut self) {
        if let Some(held) = self.held.take() {
            with_world(|_world, sched| sched.models.remove_actor(&mut sched.draw, held.actor));
        }
        self.set_ready(false);
    }

    fn step(&mut self, ctx: &TickContext, map_origin: IVec, target: [f64; 2], arrive_z: f64, is_follow_mode: bool) {
        let variant = self.variant;
        self.acquire(target);
        if self.held.is_none() {
            return;
        }
        // Wall seconds. The settle delay, the catch-up walk and the follow
        // linger are spans of real time; the tick's elapsed is a sum of deltas
        // clamped at 0.1 s, which stretches every one of them across a gap
        // where the world did not tick.
        let now = crate::host::now() / 1000.0;
        let dragging = camera_store().camera_status == CameraStatus::Drag;
        let released = self.was_dragging && !dragging;
        let gap = |a: [f64; 2], b: [f64; 2]| libm::hypot(a[0] - b[0], a[1] - b[1]);
        let (mut at, is_walking);

        if is_follow_mode {
            at = if self.was_follow_mode {
                let a = damp_alpha(FOLLOW_LERP_K, ctx.delta);
                std::array::from_fn(|k| self.at[k] + (target[k] - self.at[k]) * a)
            } else {
                target
            };
            self.heading = back_to_camera();
            if gap(target, at) > WALK_EPS {
                self.follow_walk_until = now + FOLLOW_WALK_LINGER_S;
            }
            is_walking = now < self.follow_walk_until;
            self.walk_plan = None;
        } else {
            // On the frame follow mode turns off, rebase the settle tracker to
            // the live target so the accumulated while-following delta doesn't
            // read as one giant just-settled dash.
            if self.was_follow_mode {
                self.settle = SettleTracker::new(target, now);
                self.walk_plan = None;
            }
            let just_settled = self.settle.update(target, now, SETTLE_DELAY_S);
            if self.settle.last_seen != self.settle.settled {
                wake_at((self.settle.last_change_time + SETTLE_DELAY_S) * 1000.0);
            }
            // Where the walker is headed is decided when a gesture ends: the
            // map stands where the pointer left it, and the walker leaves for
            // there from wherever it has got to. The settle tracker answers
            // for every other way the map moves — a search jump, a GPS
            // recenter, a keyboard nudge — which is why a release is not the
            // only trigger; but a release cannot wait for the tracker's
            // delay, since a pan begun inside it holds the target moving and
            // no catch-up was ever planned (camera/input.rs asks for the turn
            // that carries the release).
            let destination = if released {
                self.settle = SettleTracker::new(target, now);
                Some(target)
            } else if just_settled {
                Some(self.settle.settled)
            } else {
                None
            };
            if let Some(to) = destination {
                // Re-planning toward the same place would restart the clock and slow the
                // walk to a crawl.
                let under_way = self.walk_plan.as_ref().is_some_and(|plan| gap(plan.target, to) <= WALK_EPS);
                if !under_way && gap(to, self.at) > WALK_EPS {
                    self.walk_plan = Some(WalkPlan {
                        start: self.at,
                        target: to,
                        start_time: now,
                        duration: CATCHUP_DURATION_S,
                    });
                }
            }
            // A plan in flight finishes even once the map moves again: dropping it
            // stalled the figure during gestures and a run of quick pans ended in one jump.
            let (step, heading) = self.walk_plan.as_ref().map_or((self.at, None), |plan| plan.at(now));
            if heading.is_none() {
                self.walk_plan = None;
            }
            at = step;
            is_walking = heading.is_some();

            // The actor's Y-up to Z-up turn maps the model's forward to +Y, hence +PI/2.
            if let Some(heading) = heading {
                self.heading = heading + core::f64::consts::PI / 2.0;
            } else if !self.initial_facing_done {
                self.heading = back_to_camera();
                self.initial_facing_done = true;
            }
        }

        self.was_follow_mode = is_follow_mode;
        self.was_dragging = dragging;

        // A wall holds it walking in place until the walk ends; following a fix
        // there is no end to wait for, so a wall puts it at its target at once.
        let arrived = gap(target, at) <= WALK_EPS;
        let moved = at != self.at;
        let stands = self.stands_z.get_or_insert(arrive_z);
        if arrived {
            *stands = arrive_z;
        } else if moved {
            match step_from(map_origin, at, *stands) {
                Some(Some(z)) => *stands = z,
                Some(None) if is_follow_mode => {
                    at = target;
                    *stands = arrive_z;
                }
                Some(None) => at = self.at,
                None => {}
            }
        }
        self.at = at;

        let Some(held) = &self.held else { return };
        let sched = &mut kernel().sched;
        if let Some(a) = sched.models.actor(held.actor) {
            a.at = [at[0], at[1], *stands];
            a.rotation_z = self.heading as f32;
            a.visible = true;
        }
        let clip = if is_walking { variant.move_clip } else { IDLE_CLIP };
        if self.actor_clip != Some(clip) {
            if clip < held.model.clip_count {
                sched.models.play(held.actor, clip, if self.actor_clip.is_none() { 0.0 } else { 0.2 });
            }
            self.actor_clip = Some(clip);
        }
    }
}

struct Companion {
    person: Option<Figure>,
    pet: Option<Figure>,
}

static COMPANION: Global<Companion> = Global::new();

fn companion() -> &'static mut Companion {
    COMPANION.get(|| Companion { person: None, pet: None })
}

/// In world units, so a rebase moves nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompanionProps {
    pub map_origin: IVec,
    pub center: [f64; 2],
    /// The top of the floor under the map center.
    pub floor: f64,
}

fn figure_for<'a>(slot: &'a mut Option<Figure>, variant: &CharacterVariant, target: [f64; 2]) -> &'a mut Figure {
    if slot.as_ref().is_some_and(|w| w.variant.number != variant.number) {
        if let Some(mut old) = slot.take() {
            old.release();
        }
    }
    slot.get_or_insert_with(|| Figure::new(variant, target))
}

/// The companion's turn, in the interaction phase; None removes it.
pub fn companion_step(ctx: &TickContext, props: Option<CompanionProps>) {
    let c = COMPANION.get(|| Companion { person: None, pet: None });
    let Some(p) = props else {
        for slot in [&mut c.person, &mut c.pet] {
            if let Some(mut w) = slot.take() {
                w.release();
            }
        }
        chat_bubble_step(None, 0.0);
        return;
    };
    let settings = settings_store();
    let (Some(person_variant), Some(pet_variant)) = (character_variant(Group::person, settings.person), character_variant(Group::pet, settings.pet)) else {
        return;
    };
    let (person_variant, pet_variant) = (&person_variant, &pet_variant);
    let is_follow_mode = camera_store().is_follow_mode;

    let person_target = p.center;
    let person = figure_for(&mut c.person, person_variant, person_target);
    person.step(ctx, p.map_origin, person_target, p.floor, is_follow_mode);
    chat_bubble_step(person.held.as_ref().map(|h| h.actor), person_variant.height_u);

    let pet_target = [p.center[0] + PET_OFFSET[0], p.center[1] + PET_OFFSET[1]];
    let pet = figure_for(&mut c.pet, pet_variant, pet_target);
    // Its own floor beside the person, stepped to from the person's height.
    let pet_z = step_from(p.map_origin, pet_target, p.floor).flatten().unwrap_or(p.floor);
    pet.step(ctx, p.map_origin, pet_target, pet_z, is_follow_mode);
}

/// (model, image handle) for the shell's skinned pass.
pub fn companion_skins() -> impl Iterator<Item = (u32, u32)> {
    let c: &'static _ = companion();
    [&c.person, &c.pet].into_iter().flatten().filter_map(|w| w.held.as_ref()).map(|h| (h.model.model, h.skin))
}

// ---- where a walker steps ----

/// Where a walker standing at height `stands`, in world units, stands at
/// local `[x, y]` (world/access.rs `support`): the height, in world units;
/// none inside where a wall holds it; none outside where no page over the
/// point is resident.
fn step_from(map_origin: IVec, [x, y]: [f64; 2], stands: f64) -> Option<Option<f64>> {
    let o = ivec::to_v3(map_origin);
    let at = *local_world_to_canonical(&mut Pvf3::default(), &[x - o[0], y - o[1], local_height(over(stands), map_origin)], map_origin);
    Access::now().support(&at, COMPANION_STEP_CELLS).value()
}
