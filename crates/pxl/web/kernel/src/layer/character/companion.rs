// pxl-kernel/layer/character/companion.rs

//! A companion (person + pet) that sits at map_center, facing the camera until
//! it has somewhere to walk to. Whenever map_center stops changing — after a
//! drag, a click-to-navigate tween, a GPS recenter, whatever moved it — it
//! makes a fixed-duration catch-up walk there and ends up facing its direction
//! of travel, which it then just keeps. While map_center is still actively
//! changing it stays put. Ground height (Z) is re-sampled every frame
//! regardless of walking state — it "aligns", it doesn't walk there.
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
//! variant (io/model_asset.rs) and shared with the settings variant
//! picker's preview.
//!
//! Assets: Kenney's "Mini Characters" (person) and "Cube Pets" (pet) packs,
//! both CC0, both real glTF.

use pxl_spec::space::pvf3::ivec::{self, IVec};

use crate::control::camera::config::camera_cfg;
use crate::control::camera::orbit::damp_alpha;
use crate::control::camera::store::camera_store;
use crate::global::Global;
use crate::io::load::{load_gltf, load_image, Load};
use crate::io::model_asset::{register_model, ModelHandle};
use crate::kernel::{kernel, with_world};
use crate::layer::character::chat_bubble::chat_bubble_step;
use crate::layer::character::path::{heading_to, init_settle_tracker, sample_walk_plan, update_settle_tracker, SettleTracker, Vec2, WalkPlan, WalkSample};
use crate::layer::character::store::companion_load_store;
use crate::layer::character::variant::{character_variant, CharacterGroup, CharacterVariant};
use crate::layer::character::variant_spec::IDLE_CLIP;
use crate::layer::object::store::objects_revision;
use crate::layer::terrain::store::tile_pages;
use crate::sched::tick::TickContext;
use crate::shell::setting::store::settings_store;
use crate::world::change::wake_at;
use crate::world::frame::view::view;
use crate::world::surface_height::get_surface_z_at_local;

/// A variant's model in the scheduler, registered once per model and height
/// whoever asks first; None inside while it failed. By asset, then by height's
/// bits, so a walker asking every turn while its model is away looks up by
/// what it already holds.
type ModelHandles = Vec<(Load, u64, Option<ModelHandle>)>;

static MODEL_HANDLES: Global<ModelHandles> = Global::new();

fn model_for(variant: &CharacterVariant) -> Option<ModelHandle> {
    let handles = MODEL_HANDLES.get(Vec::new);
    let (load, height) = (Load::Asset(variant.model), variant.height_u.to_bits());
    if let Some((_, _, entry)) = handles.iter().find(|(l, h, _)| *l == load && *h == height) {
        return entry.clone();
    }
    let gltf = load_gltf(load)?;
    let handle = register_model(&gltf, Some(variant.height_u));
    handles.push((load, height, handle.clone()));
    handle
}

// A little to the side of the person, so the two don't occupy the same
// footprint — arbitrary companion-following offset.
const PET_OFFSET: Vec2 = Vec2 { x: -0.6, y: -0.6 };

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

// Follow mode: the walker's turn about Z, set every frame as
// `FOLLOW_FACING - azimuth` so its world facing stays constant while the map
// spins under it. +PI turns a model whose forward is -Y into the screen, back
// to the camera.
const FOLLOW_FACING: f64 = core::f64::consts::PI;

// Follow-mode walk clip: play it while the glide has a real gap to close, then
// hold it that much longer before dropping to idle, so it doesn't strobe
// once per GPS fix. EPS is well under one fix's worth of travel (~0.5u at a
// walking pace) and over the near-zero settled gap.
const FOLLOW_WALK_EPS: f64 = 0.03;
const FOLLOW_WALK_LINGER_S: f64 = 1.5;

struct Held {
    actor: u32,
    model: ModelHandle,
    skin: u32,
}

struct Surface {
    x: f64,
    y: f64,
    hint: f64,
    origin: Option<IVec>,
    revision: u32,
    tiles: u32,
    z: Option<f64>,
}

struct Walker {
    /// The variant it walks as: its group, which is also its role, and number.
    variant: CharacterVariant,
    held: Option<Held>,
    /// Where the walker stands in the root frame and which way it faces.
    pose: [f64; 4],
    /// The clip the actor plays, by its animation's index.
    actor_clip: Option<u32>,
    was_follow_mode: bool,
    walk_plan: Option<WalkPlan>,
    settle: SettleTracker,
    /// Faces the camera exactly once — the moment it first has a pose to
    /// resolve the camera against — and never again.
    initial_facing_done: bool,
    /// The last map_origin this loop saw, to snap (not glide) across an L4
    /// rebase.
    prev_map_origin: IVec,
    /// Clock time until which the follow-mode walk clip stays on.
    follow_walk_until: f64,
    surface: Surface,
}

impl Walker {
    fn new(variant: &CharacterVariant, target: Vec2, map_origin: IVec) -> Walker {
        Walker {
            variant: *variant,
            held: None,
            pose: [0.0; 4],
            actor_clip: None,
            was_follow_mode: false,
            walk_plan: None,
            settle: init_settle_tracker(target, 0.0),
            initial_facing_done: false,
            prev_map_origin: map_origin,
            follow_walk_until: 0.0,
            surface: Surface {
                x: f64::NAN,
                y: f64::NAN,
                hint: f64::NAN,
                origin: None,
                revision: u32::MAX,
                tiles: u32::MAX,
                z: None,
            },
        }
    }

    fn set_ready(&self, ready: bool) {
        let store = companion_load_store();
        match self.variant.group {
            CharacterGroup::person => store.person_ready = ready,
            CharacterGroup::pet => store.pet_ready = ready,
        }
    }

    /// The walker's actor, created once its model and skin have arrived; on
    /// creation it is placed at the current target at once — walking there from
    /// the frame's origin would take far too long.
    fn acquire(&mut self, variant: &CharacterVariant, target: Vec2) {
        if self.held.is_some() {
            return;
        }
        let (Some(model), Some(skin)) = (model_for(variant), load_image(Load::Asset(variant.skin))) else {
            return;
        };
        let Some(Some(actor)) = with_world(|world, sched| sched.models.new_actor(world, &mut sched.draw, model.model)) else {
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
    fn step(&mut self, ctx: &TickContext, variant: &CharacterVariant, origin_page_idx_xy: IVec, map_origin: IVec, target: Vec2, target_z: f64, is_follow_mode: bool) {
        self.acquire(variant, target);
        if self.held.is_none() {
            return;
        }
        // Wall seconds. The settle delay, the catch-up walk and the follow
        // linger are spans of real time; the tick's elapsed is a sum of deltas
        // clamped at 0.1 s, which stretches every one of them across a gap
        // where the world did not tick.
        let now = crate::host::now() / 1000.0;
        let rebased = map_origin != self.prev_map_origin;
        let (px, py, is_walking);

        if is_follow_mode {
            // Snap on the entry frame and across an L4 rebase: both are one-shot
            // jumps the view frame itself snaps through.
            if rebased || !self.was_follow_mode {
                px = target.x;
                py = target.y;
            } else {
                let a = damp_alpha(FOLLOW_LERP_K, ctx.delta);
                px = self.pose[0] + (target.x - self.pose[0]) * a;
                py = self.pose[1] + (target.y - self.pose[1]) * a;
            }
            // Back to the camera, held there while the map spins underneath.
            self.pose[3] = FOLLOW_FACING - view().azimuth;
            if (target.x - px).hypot(target.y - py) > FOLLOW_WALK_EPS {
                self.follow_walk_until = now + FOLLOW_WALK_LINGER_S;
            }
            is_walking = now < self.follow_walk_until;
            self.walk_plan = None;
        } else {
            // On the frame follow mode turns off, rebase the settle tracker to
            // the live target so the accumulated while-following delta doesn't
            // read as one giant just-settled dash.
            if self.was_follow_mode {
                self.settle = init_settle_tracker(target, now);
                self.walk_plan = None;
            }
            let update = update_settle_tracker(&self.settle, target, now, SETTLE_DELAY_S);
            self.settle = update.tracker;
            if self.settle.last_seen != self.settle.settled {
                wake_at((self.settle.last_change_time + SETTLE_DELAY_S) * 1000.0);
            }
            if update.just_settled {
                self.walk_plan = Some(WalkPlan {
                    start: Vec2 { x: self.pose[0], y: self.pose[1] },
                    target: self.settle.settled,
                    start_time: now,
                    duration: CATCHUP_DURATION_S,
                });
            }
            if update.moved {
                self.walk_plan = None;
            }
            let step = match &self.walk_plan {
                Some(plan) => sample_walk_plan(plan, now),
                None => WalkSample {
                    x: self.pose[0],
                    y: self.pose[1],
                    heading: None,
                    arrived: true,
                },
            };
            if step.arrived {
                self.walk_plan = None;
            }
            px = step.x;
            py = step.y;
            is_walking = step.heading.is_some();

            // Facing: direction of travel while walking; otherwise a one-time
            // camera-facing pose, never recomputed. The Y-up to Z-up turn the
            // actor stands with maps the model's native forward to +Y, hence the
            // +PI/2 from a raw heading.
            if let Some(heading) = step.heading {
                self.pose[3] = heading + core::f64::consts::PI / 2.0;
            } else if !self.initial_facing_done {
                let eye = view().eye_local;
                self.pose[3] = heading_to(Vec2 { x: step.x, y: step.y }, Vec2 { x: eye.x, y: eye.y }) + core::f64::consts::PI / 2.0;
                self.initial_facing_done = true;
            }
        }

        self.prev_map_origin = map_origin;
        self.was_follow_mode = is_follow_mode;

        // Z aligns to the surface under the walker — terrain, or the top of a
        // placed object standing on it, whichever is higher. Asked again only
        // when something it depends on moved.
        let s = &mut self.surface;
        let revision = objects_revision();
        let tiles = tile_pages().revision();
        if s.x != px || s.y != py || s.hint != target_z || s.origin != Some(map_origin) || s.revision != revision || s.tiles != tiles {
            *s = Surface {
                x: px,
                y: py,
                hint: target_z,
                origin: Some(map_origin),
                revision,
                tiles,
                z: get_surface_z_at_local(origin_page_idx_xy, map_origin, px, py, target_z),
            };
        }
        self.pose[0] = px;
        self.pose[1] = py;
        if let Some(z) = s.z {
            self.pose[2] = z;
        }

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
        // The next frame is the scheduler's to ask for: an actor playing a clip
        // keeps the tick awake (io/load.rs), and nothing here moves otherwise.
    }
}

struct Companion {
    person: Option<Walker>,
    pet: Option<Walker>,
}

static COMPANION: Global<Companion> = Global::new();

/// Where the companion stands: map_center in the local floating-origin frame,
/// the frame's origin and its page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompanionProps {
    pub origin_page_idx_xy: IVec,
    pub map_origin: IVec,
    pub center_x: f64,
    pub center_y: f64,
    pub center_z: f64,
}

fn walker_for<'a>(slot: &'a mut Option<Walker>, variant: &CharacterVariant, target: Vec2, map_origin: IVec) -> &'a mut Walker {
    // A variant switch replaces the walker, which always starts unloaded.
    if slot.as_ref().is_some_and(|w| w.variant.number != variant.number) {
        if let Some(mut old) = slot.take() {
            old.release();
        }
    }
    slot.get_or_insert_with(|| Walker::new(variant, target, map_origin))
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
    let (Some(person_variant), Some(pet_variant)) = (
        character_variant(CharacterGroup::person, settings.person_variant),
        character_variant(CharacterGroup::pet, settings.pet_variant),
    ) else {
        return;
    };
    let (person_variant, pet_variant) = (&person_variant, &pet_variant);
    let is_follow_mode = camera_store().is_follow_mode;

    let person_target = Vec2 { x: p.center_x, y: p.center_y };
    let person = walker_for(&mut c.person, person_variant, person_target, p.map_origin);
    person.step(ctx, person_variant, p.origin_page_idx_xy, p.map_origin, person_target, p.center_z, is_follow_mode);
    // Only the person talks — the pet doesn't get a chat bubble.
    chat_bubble_step(person.held.as_ref().map(|h| h.actor), person_variant.height_u);

    let pet_target = Vec2 {
        x: p.center_x + PET_OFFSET.x,
        y: p.center_y + PET_OFFSET.y,
    };
    let pet = walker_for(&mut c.pet, pet_variant, pet_target, p.map_origin);
    pet.step(ctx, pet_variant, p.origin_page_idx_xy, p.map_origin, pet_target, p.center_z, is_follow_mode);
}

/// Which skin each live companion model is drawn with: (model, image handle),
/// for the shell's skinned pass.
pub fn companion_skins() -> impl Iterator<Item = (u32, u32)> {
    let c: &'static _ = COMPANION.get(|| Companion { person: None, pet: None });
    [&c.person, &c.pet].into_iter().flatten().filter_map(|w| w.held.as_ref()).map(|h| (h.model.model, h.skin))
}
