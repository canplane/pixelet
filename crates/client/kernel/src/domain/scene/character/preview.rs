// pxl-kernel/domain/scene/character/preview.rs

//! Display cases for the variant picker: a character idling and spinning in
//! place, drawn into the case's own canvas from far below the world. The
//! model is registered again at one unit tall, since the cases compare
//! silhouettes, not scale.

use crate::domain::query::model::asset::{model_of, ModelHandle};
use crate::domain::query::model::variant_spec::IDLE_CLIP;
use crate::domain::scene::character::variant::{character_variant, CharacterVariant, Group};
use crate::kernel;
use crate::laid;
use crate::sys::io::load::load_image;
use crate::sys::load::Load;
use crate::sys::sched::edge::request_world_tick;
use crate::sys::sched::tick::TickContext;
use xpute_guest::global::Global;

const SPIN_RATE_RAD_S: f64 = 0.6;
/// Under the sea below the eye, past any terrain, so the world never draws it.
const CASE_DEPTH_U: f64 = 100_000.0;

struct Case {
    id: u32,
    variant: CharacterVariant,
    actor: Option<u32>,
    model: Option<ModelHandle>,
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
        if let Some(k) = laid() {
            k.scene.actors.remove_actor(&mut k.draw, actor);
        }
    }
    true
}

/// An open case is what asks the world's tick to run, while it turns: one
/// still loading draws nothing and asks nothing, its model or skin landing
/// waking the turn that lays it.
pub fn preview_step(ctx: &mut TickContext) {
    let c = cases();
    if c.cases.is_empty() {
        return;
    }
    // Under the sea below the eye, out of sight and near it, where the f32 of
    // its draw resolves a unit-tall model.
    let mut at = crate::domain::view::frame::pose::sea_under_eye();
    at[2] -= CASE_DEPTH_U;
    let mut turned = false;
    for case in &mut c.cases {
        if case.model.is_none() {
            case.model = model_of(case.variant.model, Some(1.0));
        }
        if case.actor.is_none() {
            if let (Some(model), Some(_)) = (&case.model, load_image(Load::Asset(case.variant.skin))) {
                let id = model.model;
                case.actor = laid().and_then(|k| k.scene.actors.new_actor(&mut k.draw, crate::domain::query::model::models(), id));
                if let (Some(actor), true) = (case.actor, IDLE_CLIP < model.clip_count) {
                    kernel().scene.actors.play(actor, IDLE_CLIP, 0.0);
                }
            }
        }
        let Some(actor) = case.actor else { continue };
        case.spin += SPIN_RATE_RAD_S * ctx.delta;
        if let Some(a) = kernel().scene.actors.actor(actor) {
            a.at = at;
            a.rotation_z = case.spin as f32;
            a.visible = true;
            turned = true;
        }
    }
    if turned {
        ctx.invalidate();
    }
}

/// (case, model, skin image), the image as the loader has it now.
pub fn preview_rows() -> impl Iterator<Item = (u32, u32, (u32, &'static [u8]))> {
    let c: &'static _ = cases();
    c.cases.iter().filter_map(|case| match (&case.model, case.actor) {
        (Some(model), Some(_)) => Some((case.id, model.model, load_image(Load::Asset(case.variant.skin))?)),
        _ => None,
    })
}
