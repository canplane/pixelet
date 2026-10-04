// pxl-kernel/layer/character/preview.rs

//! Display cases for the variant picker: a character idling and spinning in
//! place, drawn into the case's own canvas from far below the world. The
//! model is registered again at one unit tall, since the cases compare
//! silhouettes, not scale.

use pxl_spec::space::pvf3::ivec;

use crate::io::load::{load_image, Load};
use crate::kernel::{kernel, with_world};
use crate::layer::character::variant::{character_variant, model_of, CharacterVariant, Group};
use crate::layer::character::variant_spec::IDLE_CLIP;
use crate::layer::model::asset::ModelHandle;
use crate::sched::tick::TickContext;
use crate::world::change::request_world_tick;
use crate::world::frame::view::view;
use xpute_runtime::global::Global;

const SPIN_RATE_RAD_S: f64 = 0.6;
/// Past any terrain and the fog, so the world never draws it.
const CASE_DEPTH_U: f64 = 100_000.0;

struct Case {
    id: u32,
    variant: CharacterVariant,
    actor: Option<u32>,
    model: Option<ModelHandle>,
    skin: Option<u32>,
    spin: f64,
}

struct Cases {
    cases: Vec<Case>,
}

static CASES: Global<Cases> = Global::new();

fn cases() -> &'static mut Cases {
    CASES.get(|| Cases { cases: Vec::new() })
}

/// One per surface: a case is named by the surface it draws on.
pub const CASES_MAX: usize = pxl_render::gpu::surface::SURFACES;

/// Replaces a case already open under `id`. False where `id` names no
/// surface or no variant.
/// On a live surface; false for a variant there is none of.
pub fn preview_open(id: u32, group: Group, number: u8) -> bool {
    preview_close(id);
    let Some(variant) = character_variant(group, number) else { return false };
    cases().cases.push(Case {
        id,
        variant,
        actor: None,
        model: None,
        skin: None,
        spin: 0.0,
    });
    request_world_tick();
    true
}

/// Whether a case was open there.
pub fn preview_close(id: u32) -> bool {
    let c = cases();
    let Some(pos) = c.cases.iter().position(|case| case.id == id) else { return false };
    let case = c.cases.remove(pos);
    if let Some(actor) = case.actor {
        with_world(|_world, sched| sched.models.remove_actor(&mut sched.draw, actor));
    }
    true
}

/// An open case is what asks the world's tick to run.
pub fn preview_step(ctx: &mut TickContext) {
    let c = cases();
    if c.cases.is_empty() {
        return;
    }
    // Under the map origin: the case reads the actor's place from an f32
    // uniform, which cannot resolve a unit-tall model millions of units out.
    let origin = view().origin;
    let mut at = ivec::to_v3(origin);
    at[2] -= CASE_DEPTH_U;
    for case in &mut c.cases {
        if case.model.is_none() {
            case.model = model_of(case.variant.model, 1.0);
        }
        if case.skin.is_none() {
            case.skin = load_image(Load::Asset(case.variant.skin));
        }
        if case.actor.is_none() {
            if let (Some(model), Some(_)) = (&case.model, case.skin) {
                let id = model.model;
                case.actor = with_world(|_, sched| sched.models.new_actor(&mut sched.draw, id)).flatten();
                if let (Some(actor), true) = (case.actor, IDLE_CLIP < model.clip_count) {
                    kernel().sched.models.play(actor, IDLE_CLIP, 0.0);
                }
            }
        }
        let Some(actor) = case.actor else { continue };
        case.spin += SPIN_RATE_RAD_S * ctx.delta;
        if let Some(a) = kernel().sched.models.actor(actor) {
            a.at = at;
            a.rotation_z = case.spin as f32;
            a.visible = true;
        }
    }
    ctx.invalidate();
}

/// (case, model, skin image).
pub fn preview_rows() -> impl Iterator<Item = (u32, u32, u32)> {
    let c: &'static _ = cases();
    c.cases.iter().filter_map(|case| match (&case.model, case.skin, case.actor) {
        (Some(model), Some(skin), Some(_)) => Some((case.id, model.model, skin)),
        _ => None,
    })
}
