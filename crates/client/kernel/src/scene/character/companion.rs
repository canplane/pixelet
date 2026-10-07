// pxl-kernel/scene/character/companion.rs

//! A companion (person + pet) at focus. When focus comes to rest it
//! makes a fixed-duration catch-up walk there; in follow mode, where
//! focus is a ~1 Hz GPS stream that never rests, it instead glides at
//! the world-scroll rate so the two cancel and it holds at screen center.
//! Assets: Kenney's "Mini Characters" and "Cube Pets", CC0.

use crate::control::camera::approach::damp_alpha;
use crate::control::camera::input::steering;
use crate::control::camera::state::{camera_state, CameraStatus};
use crate::control::navigation::support::over;
use crate::io::load::{load_image, Load};
use crate::kernel;
use crate::laid;
use crate::model::asset::ModelHandle;
use crate::param::COMPANION_STEP_CELLS;
use crate::query::World;
use crate::scene::character::chat_bubble::chat_bubble_step;
use crate::scene::character::path::{heading_to, SettleTracker, WalkPlan};
use crate::scene::character::variant::{character_variant, model_of, CharacterVariant, Group};
use crate::scene::character::variant_spec::IDLE_CLIP;
use crate::sched::edge::wake_at;
use crate::sched::tick::TickContext;
use crate::setting::store::settings_store;
use crate::view::frame::coord::world_to_canonical;
use xpute_guest::global::Global;

pub const ACTORS_MAX: usize = Group::ALL.len();

// Arbitrary, so the pet does not share the person's footprint.
const PET_OFFSET: [f64; 2] = [-0.6, -0.6];

// A fixed duration, not a fixed speed (see sample_walk_plan).
const CATCHUP_DURATION_S: f64 = 0.6;

const SETTLE_DELAY_S: f64 = 0.15;

// The world scroll's own rate: the two cancel, holding the character at
// screen center, so this is the only position smoothing in follow mode.
const FOLLOW_LERP_K: f64 = crate::param::POS_DAMP;

// +PI turns a model whose forward is -Y into the screen, back to the camera.
const FOLLOW_FACING: f64 = core::f64::consts::PI;

/// Turned against the azimuth, which is the screen's up laid on the ground.
fn back_to_camera() -> f64 {
    FOLLOW_FACING - crate::control::camera::controller::azimuth()
}

// Under one GPS fix's travel at a walking pace (~0.5u), over the near-zero gap
// a still map leaves. A rotate or pinch leaves such a gap, and a catch-up
// across it would play the walk clip in place.
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
    /// Its falling edge, a gesture's end, fixes the destination.
    was_dragging: bool,
    settle: SettleTracker,
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
        let Some(Some(actor)) = laid().map(|k| k.scene.actors.new_actor(&mut k.draw, &k.models, model.model)) else {
            return;
        };
        self.held = Some(Held { actor, model, skin });
        self.at = target;
        self.set_ready(true);
    }

    fn release(&mut self) {
        if let Some(held) = self.held.take() {
            if let Some(k) = laid() {
                k.scene.actors.remove_actor(&mut k.draw, held.actor);
            }
        }
        self.set_ready(false);
    }

    /// `follows`: the target is followed as it moves, by a fix or by the keys,
    /// rather than walked to once it settles; a steered one faces its way.
    fn step(&mut self, ctx: &TickContext, target: [f64; 2], arrive_z: f64, follows: bool, steered: bool) {
        let variant = self.variant;
        self.acquire(target);
        if self.held.is_none() {
            return;
        }
        // Wall seconds: the tick's elapsed is clamped per delta, which stretches
        // these spans across a gap where the world did not tick.
        let now = xpute_guest::clock::now() / 1000.0;
        let dragging = camera_state().camera_status == CameraStatus::Drag;
        let released = self.was_dragging && !dragging;
        let gap = |a: [f64; 2], b: [f64; 2]| libm::hypot(a[0] - b[0], a[1] - b[1]);
        let (mut at, is_walking);

        if follows {
            at = if self.was_follow_mode {
                let a = damp_alpha(FOLLOW_LERP_K, ctx.delta);
                std::array::from_fn(|k| self.at[k] + (target[k] - self.at[k]) * a)
            } else {
                target
            };
            if !steered {
                self.heading = back_to_camera();
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
            if self.was_follow_mode {
                self.settle = SettleTracker::new(target, now);
                self.walk_plan = None;
            }
            let just_settled = self.settle.update(target, now, SETTLE_DELAY_S);
            if self.settle.last_seen != self.settle.settled {
                wake_at((self.settle.last_change_time + SETTLE_DELAY_S) * 1000.0);
            }
            // A release fixes the destination at once: a pan begun inside the settle
            // delay holds the target moving, so waiting would plan nothing. The settle
            // tracker covers every other way the focus moves.
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
            // A plan in flight finishes even once the focus moves again: dropping it
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

        self.was_follow_mode = follows;
        self.was_dragging = dragging;

        // A wall holds it walking in place until the walk ends; following a fix
        // there is no end to wait for, so a wall puts it at its target at once.
        let arrived = gap(target, at) <= WALK_EPS;
        let moved = at != self.at;
        let stands = self.stands_z.get_or_insert(arrive_z);
        if arrived {
            *stands = arrive_z;
        } else if moved {
            match step_from(at, *stands) {
                Some(Some(z)) => *stands = z,
                Some(None) if follows => {
                    at = target;
                    *stands = arrive_z;
                }
                Some(None) => at = self.at,
                None => {}
            }
        }
        self.at = at;

        let Some(held) = &self.held else { return };
        let sched = &mut kernel().scene;
        if let Some(a) = sched.actors.actor(held.actor) {
            a.at = [at[0], at[1], *stands];
            a.rotation_z = self.heading as f32;
            a.visible = true;
        }
        let clip = if is_walking { variant.move_clip } else { IDLE_CLIP };
        if self.actor_clip != Some(clip) {
            if clip < held.model.clip_count {
                sched.actors.play(held.actor, clip, if self.actor_clip.is_none() { 0.0 } else { 0.2 });
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

/// In the world's doubles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompanionProps {
    pub center: [f64; 2],
    /// The top of the floor under the focus.
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

pub fn companion_step(ctx: &TickContext, props: Option<CompanionProps>) {
    let c = companion();
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
    let steered = steering();
    let follows = crate::control::navigation::follow_mode::following() || steered;

    let person_target = p.center;
    let person = figure_for(&mut c.person, person_variant, person_target);
    person.step(ctx, person_target, p.floor, follows, steered);
    chat_bubble_step(person.held.as_ref().map(|h| h.actor), person_variant.height_u);

    let pet_target = [p.center[0] + PET_OFFSET[0], p.center[1] + PET_OFFSET[1]];
    let pet = figure_for(&mut c.pet, pet_variant, pet_target);
    // Its own floor beside the person, stepped to from the person's height.
    let pet_z = step_from(pet_target, p.floor).flatten().unwrap_or(p.floor);
    pet.step(ctx, pet_target, pet_z, follows, steered);
}

/// (model, image handle) for the shell's skinned pass.
pub fn companion_skins() -> impl Iterator<Item = (u32, u32)> {
    let c: &'static _ = companion();
    [&c.person, &c.pet].into_iter().flatten().filter_map(|w| w.held.as_ref()).map(|h| (h.model.model, h.skin))
}

/// None inside where a wall holds it; None outside where no tile is resident.
fn step_from([x, y]: [f64; 2], stands: f64) -> Option<Option<f64>> {
    let at = world_to_canonical(&[x, y, over(stands)]);
    World::now().support(&at, COMPANION_STEP_CELLS).value()
}

/// Whether the companion's models have loaded: a live, unpersisted signal
/// for the loading toast. A fresh figure always starts unloaded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompanionLoadStore {
    pub person_ready: bool,
    pub pet_ready: bool,
}

static STORE: Global<CompanionLoadStore> = Global::new();

pub fn companion_load_store() -> &'static mut CompanionLoadStore {
    STORE.get(CompanionLoadStore::default)
}
