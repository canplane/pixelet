// pxl-kernel/layer/character/companion.rs

//! A companion (person + pet) at map_center. When map_center comes to rest it
//! makes a fixed-duration catch-up walk there; in follow mode, where
//! map_center is a ~1 Hz GPS stream that never rests, it instead glides at
//! the world-scroll rate so the two cancel and it holds at screen center.
//! Assets: Kenney's "Mini Characters" and "Cube Pets", CC0.

use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;

use crate::control::camera::approach::damp_alpha;
use crate::control::camera::config::camera_cfg;
use crate::control::camera::store::{camera_store, CameraStatus};
use crate::control::navigation::support::over;
use crate::param::COMPANION_STEP_CELLS;
use crate::global::Global;
use crate::io::load::{load_image, Load};
use crate::kernel::{kernel, with_world};
use crate::layer::character::chat_bubble::chat_bubble_step;
use crate::layer::character::path::{init_settle_tracker, sample_walk_plan, update_settle_tracker, SettleTracker, Vec2, WalkPlan, WalkSample};
use crate::layer::character::store::companion_load_store;
use crate::layer::character::variant::{character_variant, model_of, CharacterVariant, Group};
use crate::layer::character::variant_spec::IDLE_CLIP;
use crate::layer::model::asset::ModelHandle;
use crate::sched::tick::TickContext;
use crate::shell::setting::store::settings_store;
use crate::world::access::Access;
use crate::world::change::wake_at;
use crate::world::frame::coord::{local_height, local_world_to_canonical};
use crate::world::frame::view::view;

pub const ACTORS_MAX: usize = Group::ALL.len();

// Arbitrary, so the pet does not share the person's footprint.
const PET_OFFSET: Vec2 = Vec2 { x: -0.6, y: -0.6 };

// A fixed duration, not a fixed speed (see sample_walk_plan).
const CATCHUP_DURATION_S: f64 = 0.6;

const SETTLE_DELAY_S: f64 = 0.15;

// The world scroll's own rate: the two cancel, holding the character at
// screen center, so this is the only position smoothing in follow mode.
const FOLLOW_LERP_K: f64 = camera_cfg::smooth::POS_DAMP;

// +PI turns a model whose forward is -Y into the screen, back to the camera.
const FOLLOW_FACING: f64 = core::f64::consts::PI;

/// The screen's up laid on the ground is a quarter turn from east at heading zero.
fn back_to_camera() -> f64 {
    FOLLOW_FACING + view().up_heading() - core::f64::consts::FRAC_PI_2
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
    pose: [f64; 4],
    actor_clip: Option<u32>,
    was_follow_mode: bool,
    walk_plan: Option<WalkPlan>,
    /// Its falling edge, a gesture's end, fixes the destination.
    was_dragging: bool,
    settle: SettleTracker,
    initial_facing_done: bool,
    /// To snap rather than glide across an L4 rebase.
    prev_map_origin: IVec,
    follow_walk_until: f64,
    /// In world units, since the frame's own Z moves on a rebase.
    stands_z: Option<f64>,
}

impl Figure {
    fn new(variant: &CharacterVariant, target: Vec2, map_origin: IVec) -> Figure {
        Figure {
            variant: *variant,
            held: None,
            pose: [0.0; 4],
            actor_clip: None,
            was_follow_mode: false,
            walk_plan: None,
            was_dragging: false,
            settle: init_settle_tracker(target, 0.0),
            initial_facing_done: false,
            prev_map_origin: map_origin,
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
    fn acquire(&mut self, target: Vec2) {
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
        self.pose = [target.x, target.y, 0.0, self.pose[3]];
        self.set_ready(true);
    }

    fn release(&mut self) {
        if let Some(held) = self.held.take() {
            with_world(|_world, sched| sched.models.remove_actor(&mut sched.draw, held.actor));
        }
        self.set_ready(false);
    }

    #[allow(clippy::too_many_arguments)]
    fn step(&mut self, ctx: &TickContext, map_origin: IVec, target: Vec2, arrive_z: f64, is_follow_mode: bool) {
        let variant = self.variant;
        self.acquire(target);
        if self.held.is_none() {
            return;
        }
        // Wall seconds: the tick's elapsed is clamped per delta, which stretches
        // these spans across a gap where the world did not tick.
        let now = crate::host::now() / 1000.0;
        let rebased = map_origin != self.prev_map_origin;
        let dragging = camera_store().camera_status == CameraStatus::Drag;
        let released = self.was_dragging && !dragging;
        let (mut px, mut py, is_walking);

        if is_follow_mode {
            if rebased || !self.was_follow_mode {
                px = target.x;
                py = target.y;
            } else {
                let a = damp_alpha(FOLLOW_LERP_K, ctx.delta);
                px = self.pose[0] + (target.x - self.pose[0]) * a;
                py = self.pose[1] + (target.y - self.pose[1]) * a;
            }
            self.pose[3] = back_to_camera();
            if (target.x - px).hypot(target.y - py) > WALK_EPS {
                self.follow_walk_until = now + FOLLOW_WALK_LINGER_S;
            }
            is_walking = now < self.follow_walk_until;
            self.walk_plan = None;
        } else {
            // Rebase the tracker, or the delta accumulated while following reads as one
            // giant dash.
            if self.was_follow_mode {
                self.settle = init_settle_tracker(target, now);
                self.walk_plan = None;
            }
            let update = update_settle_tracker(&self.settle, target, now, SETTLE_DELAY_S);
            self.settle = update.tracker;
            if self.settle.last_seen != self.settle.settled {
                wake_at((self.settle.last_change_time + SETTLE_DELAY_S) * 1000.0);
            }
            // A release fixes the destination at once: a pan begun inside the settle
            // delay holds the target moving, so waiting would plan nothing. The settle
            // tracker covers every other way the map moves.
            let destination = if released {
                self.settle = init_settle_tracker(target, now);
                Some(target)
            } else if update.just_settled {
                Some(self.settle.settled)
            } else {
                None
            };
            if let Some(to) = destination {
                // Re-planning toward the same place would restart the clock and slow the
                // walk to a crawl.
                let under_way = self.walk_plan.as_ref().is_some_and(|plan| (plan.target.x - to.x).hypot(plan.target.y - to.y) <= WALK_EPS);
                if !under_way && (to.x - self.pose[0]).hypot(to.y - self.pose[1]) > WALK_EPS {
                    self.walk_plan = Some(WalkPlan {
                        start: Vec2 { x: self.pose[0], y: self.pose[1] },
                        target: to,
                        start_time: now,
                        duration: CATCHUP_DURATION_S,
                    });
                }
            }
            // A plan in flight finishes even once the map moves again: dropping it
            // stalled the figure during gestures and a run of quick pans ended in one jump.
            let step = match &self.walk_plan {
                Some(plan) => sample_walk_plan(plan, now),
                None => WalkSample {
                    x: self.pose[0],
                    y: self.pose[1],
                    heading: None,
                },
            };
            if step.heading.is_none() {
                self.walk_plan = None;
            }
            px = step.x;
            py = step.y;
            is_walking = step.heading.is_some();

            // The actor's Y-up to Z-up turn maps the model's forward to +Y, hence +PI/2.
            if let Some(heading) = step.heading {
                self.pose[3] = heading + core::f64::consts::PI / 2.0;
            } else if !self.initial_facing_done {
                self.pose[3] = back_to_camera();
                self.initial_facing_done = true;
            }
        }

        self.prev_map_origin = map_origin;
        self.was_follow_mode = is_follow_mode;
        self.was_dragging = dragging;

        // A wall holds it walking in place until the walk ends; following a fix
        // there is no end to wait for, so a wall puts it at its target at once.
        let arrived = (target.x - px).hypot(target.y - py) <= WALK_EPS;
        let moved = (px, py) != (self.pose[0], self.pose[1]);
        let stands = self.stands_z.get_or_insert(arrive_z);
        if arrived {
            *stands = arrive_z;
        } else if moved {
            match step_from(map_origin, [px, py], *stands) {
                Some(Some(z)) => *stands = z,
                Some(None) if is_follow_mode => {
                    (px, py) = (target.x, target.y);
                    *stands = arrive_z;
                }
                Some(None) => (px, py) = (self.pose[0], self.pose[1]),
                None => {}
            }
        }
        self.pose[0] = px;
        self.pose[1] = py;
        self.pose[2] = local_height(*stands, map_origin);

        let Some(held) = &self.held else { return };
        let sched = &mut kernel().sched;
        if let Some(a) = sched.models.actor(held.actor) {
            a.at = [
                ivec::to_real(ivec::X(map_origin)) + px,
                ivec::to_real(ivec::Y(map_origin)) + py,
                ivec::to_real(ivec::Z(map_origin)) + self.pose[2],
            ];
            a.rotation_z = self.pose[3] as f32;
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompanionProps {
    pub map_origin: IVec,
    pub center_x: f64,
    pub center_y: f64,
    /// The top of the floor under the map center, in world units.
    pub floor: f64,
}

fn figure_for<'a>(slot: &'a mut Option<Figure>, variant: &CharacterVariant, target: Vec2, map_origin: IVec) -> &'a mut Figure {
    if slot.as_ref().is_some_and(|w| w.variant.number != variant.number) {
        if let Some(mut old) = slot.take() {
            old.release();
        }
    }
    slot.get_or_insert_with(|| Figure::new(variant, target, map_origin))
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
    let is_follow_mode = camera_store().is_follow_mode;

    let person_target = Vec2 { x: p.center_x, y: p.center_y };
    let person = figure_for(&mut c.person, person_variant, person_target, p.map_origin);
    person.step(ctx, p.map_origin, person_target, p.floor, is_follow_mode);
    chat_bubble_step(person.held.as_ref().map(|h| h.actor), person_variant.height_u);

    let pet_target = Vec2 {
        x: p.center_x + PET_OFFSET.x,
        y: p.center_y + PET_OFFSET.y,
    };
    let pet = figure_for(&mut c.pet, pet_variant, pet_target, p.map_origin);
    // Its own floor beside the person, stepped to from the person's height.
    let pet_z = step_from(p.map_origin, [pet_target.x, pet_target.y], p.floor).flatten().unwrap_or(p.floor);
    pet.step(ctx, p.map_origin, pet_target, pet_z, is_follow_mode);
}

/// (model, image handle) for the shell's skinned pass.
pub fn companion_skins() -> impl Iterator<Item = (u32, u32)> {
    let c: &'static _ = companion();
    [&c.person, &c.pet].into_iter().flatten().filter_map(|w| w.held.as_ref()).map(|h| (h.model.model, h.skin))
}

/// None inside where a wall holds it; None outside where no page is resident.
fn step_from(map_origin: IVec, [x, y]: [f64; 2], stands: f64) -> Option<Option<f64>> {
    let at = *local_world_to_canonical(&mut Pvf3::default(), &[x, y, local_height(over(stands), map_origin)], map_origin);
    Access::now().support(&at, COMPANION_STEP_CELLS).value()
}
