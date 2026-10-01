// pxl-kernel/layer/character/companion.rs

//! A companion (person + pet) that stands at map_center, which is the
//! character's place and the camera's center (controller.rs `map_center`):
//! one point, never a copy the character keeps and walks after. Where the
//! drawn map center goes the person is, its height the floor the walk stood
//! the map center on (layer/terrain/ground_snap.rs `walk`) — in a cave as on
//! the surface; the pet a little to the side, on the floor the same walk
//! finds there. Moving, it plays its walk and faces the way it goes;
//! still, it keeps its facing — its back to the camera until it has first
//! moved. Follow mode keeps its back to the camera throughout: a nav-app
//! "you" marker, the map spinning under it.
//!
//! The model is registered once per
//! variant (layer/model/asset.rs) and shared with the settings variant
//! picker's preview.
//!
//! Assets: Kenney's "Mini Characters" (person) and "Cube Pets" (pet) packs,
//! both CC0, both real glTF.

use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::pvf3::pvf3::Pvf3;

use crate::control::camera::store::camera_store;
use crate::global::Global;
use crate::io::load::{load_gltf, load_image, Load};
use crate::kernel::{kernel, with_world};
use crate::layer::character::chat_bubble::chat_bubble_step;
use crate::layer::character::store::companion_load_store;
use crate::layer::character::variant::{character_variant, CharacterGroup, CharacterVariant};
use crate::layer::character::variant_spec::IDLE_CLIP;
use crate::layer::model::asset::{register_model, ModelHandle};
use crate::shell::setting::store::settings_store;
use crate::layer::terrain::ground_snap::ground_or_raw;
use crate::world::frame::coord::{canonical_to_local, local_world_to_canonical};
use crate::world::frame::view::view;

/// A variant's model in the scheduler, registered once per model and height
/// whoever asks first; None inside while it failed. By asset, then by height's
/// bits, so a walker asking every turn while its model is away looks up by
/// what it already holds.
type ModelHandles = Vec<(Load, u64, Option<ModelHandle>)>;

/// Models the companion registers at most: one a variant, by its asset and
/// height (`model_for`).
pub const MODELS_MAX: usize = crate::layer::character::variant::CHARACTER_VARIANTS;

/// Actors the companion holds at most: a walker a group, the person and the
/// pet (`Companion`).
pub const ACTORS_MAX: usize = crate::layer::character::variant_spec::Group::ALL.len();

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
const PET_OFFSET: [f64; 2] = [-0.6, -0.6];

// +PI turns a model whose forward is -Y into the screen, back to the camera.
const FOLLOW_FACING: f64 = core::f64::consts::PI;

/// The walker's turn about Z that puts its back to the camera, looking where
/// the screen does: the screen's up laid on the ground, which is a quarter
/// turn from east at a heading of zero.
fn back_to_camera() -> f64 {
    FOLLOW_FACING + view().up_heading() - core::f64::consts::FRAC_PI_2
}

// A move shorter than this in a turn is no step: the drawn map center's last
// creep toward its target, or a rotate or a pinch, which leave it where it
// stood — the walk would play in place and turn the walker to the heading of
// a zero-length step. The walk is held this long past the last step, so a
// map moved in bursts — a GPS fix a second — does not strobe it.
const WALK_EPS: f64 = 0.03;
const WALK_LINGER_S: f64 = 1.5;

struct Held {
    actor: u32,
    model: ModelHandle,
    skin: u32,
}

struct Walker {
    /// The variant it walks as: its group, which is also its role, and number.
    variant: CharacterVariant,
    held: Option<Held>,
    /// Where it was drawn last turn, in world units, and which way it faces:
    /// a turn's step is the way it went since.
    at: Option<[f64; 3]>,
    rotation_z: f64,
    /// The clip the actor plays, by its animation's index.
    actor_clip: Option<u32>,
    /// Clock time until which the walk clip stays on.
    walk_until: f64,
}

impl Walker {
    fn new(variant: &CharacterVariant) -> Walker {
        Walker {
            variant: *variant,
            held: None,
            at: None,
            rotation_z: back_to_camera(),
            actor_clip: None,
            walk_until: 0.0,
        }
    }

    fn set_ready(&self, ready: bool) {
        let store = companion_load_store();
        match self.variant.group {
            CharacterGroup::person => store.person_ready = ready,
            CharacterGroup::pet => store.pet_ready = ready,
        }
    }

    /// The walker's actor, created once its model and skin have arrived.
    fn acquire(&mut self, variant: &CharacterVariant) {
        if self.held.is_some() {
            return;
        }
        let (Some(model), Some(skin)) = (model_for(variant), load_image(Load::Asset(variant.skin))) else {
            return;
        };
        let Some(Some(actor)) = with_world(|_, sched| sched.models.new_actor(&mut sched.draw, model.model)) else {
            return;
        };
        self.held = Some(Held { actor, model, skin });
        self.set_ready(true);
    }

    fn release(&mut self) {
        if let Some(held) = self.held.take() {
            with_world(|_world, sched| sched.models.remove_actor(&mut sched.draw, held.actor));
        }
        self.set_ready(false);
    }

    /// The walker drawn at `at`, in world units: where it is, and nothing it
    /// keeps of its own but which way it faced and whether it walks.
    fn step(&mut self, variant: &CharacterVariant, at: [f64; 3], is_follow_mode: bool) {
        self.acquire(variant);
        if self.held.is_none() {
            return;
        }
        // Wall seconds: the linger is a span of real time.
        let now = crate::host::now() / 1000.0;
        let went = self.at.map_or([0.0; 3], |was| [at[0] - was[0], at[1] - was[1], at[2] - was[2]]);
        if went[0].hypot(went[1]) > WALK_EPS {
            self.walk_until = now + WALK_LINGER_S;
            // The Y-up to Z-up turn the actor stands with maps the model's
            // native forward to +Y, hence the quarter turn from a heading.
            self.rotation_z = went[1].atan2(went[0]) + core::f64::consts::FRAC_PI_2;
        }
        if is_follow_mode {
            self.rotation_z = back_to_camera();
        }
        self.at = Some(at);
        let is_walking = now < self.walk_until;

        let Some(held) = &self.held else { return };
        let sched = &mut kernel().sched;
        if let Some(a) = sched.models.actor(held.actor) {
            a.at = at;
            a.rotation_z = self.rotation_z as f32;
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
        // keeps the tick awake (layer/model/store.rs), and nothing here moves otherwise.
    }
}

struct Companion {
    person: Option<Walker>,
    pet: Option<Walker>,
}

static COMPANION: Global<Companion> = Global::new();

/// Where the companion stands: map_center, the camera's center and the
/// character's place, in world units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompanionProps {
    pub map_center: Pvf3,
}

fn walker_for<'a>(slot: &'a mut Option<Walker>, variant: &CharacterVariant) -> &'a mut Walker {
    // A variant switch replaces the walker, which always starts unloaded.
    if slot.as_ref().is_some_and(|w| w.variant.number != variant.number) {
        if let Some(mut old) = slot.take() {
            old.release();
        }
    }
    slot.get_or_insert_with(|| Walker::new(variant))
}

/// The companion's turn, in the interaction phase; None removes it.
pub fn companion_step(props: Option<CompanionProps>) {
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
    let (Some(person_variant), Some(pet_variant)) = (character_variant(CharacterGroup::person, settings.person), character_variant(CharacterGroup::pet, settings.pet)) else {
        return;
    };
    let (person_variant, pet_variant) = (&person_variant, &pet_variant);
    let is_follow_mode = camera_store().is_follow_mode;

    let person = walker_for(&mut c.person, person_variant);
    person.step(person_variant, world_at(&p.map_center), is_follow_mode);
    // Only the person talks — the pet doesn't get a chat bubble.
    chat_bubble_step(person.held.as_ref().map(|h| h.actor), person_variant.height_u);

    // The pet beside it, on the floor the walk finds there from the person's
    // (layer/terrain/ground_snap.rs): derived from the map center as the
    // person is, never a place of its own.
    let origin = view().origin;
    let center = canonical_to_local(&p.map_center, origin);
    let mut beside = Pvf3::default();
    local_world_to_canonical(&mut beside, &[center[0] + PET_OFFSET[0], center[1] + PET_OFFSET[1], center[2]], origin);
    let pet = walker_for(&mut c.pet, pet_variant);
    pet.step(pet_variant, world_at(&ground_or_raw(&beside)), is_follow_mode);
}

/// A canonical point in world units, as an actor is placed.
fn world_at(at: &Pvf3) -> [f64; 3] {
    let o = view().origin;
    let local = canonical_to_local(at, o);
    [local[0] + ivec::to_real(ivec::X(o)), local[1] + ivec::to_real(ivec::Y(o)), local[2] + ivec::to_real(ivec::Z(o))]
}

/// Which skin each live companion model is drawn with: (model, image handle),
/// for the shell's skinned pass.
pub fn companion_skins() -> impl Iterator<Item = (u32, u32)> {
    let c: &'static _ = COMPANION.get(|| Companion { person: None, pet: None });
    [&c.person, &c.pet].into_iter().flatten().filter_map(|w| w.held.as_ref()).map(|h| (h.model.model, h.skin))
}
