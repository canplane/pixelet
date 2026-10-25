// pxl-kernel/domain/scene/character/preview.rs

//! Display cases for the variant picker: a character idling and spinning in
//! place, drawn into the case's own canvas from far below the world. A case
//! is a holder of its actor's instance: the variant's one registered model,
//! which the instance's stance scales to one unit tall, since the cases
//! compare silhouettes, not scale.

use crate::domain::query::model::asset::{model_of, ModelHandle};
use crate::domain::query::model::draw_list;
use crate::domain::query::model::registry::Models;
use crate::domain::scene::actor::{Actor, Stance};
use crate::domain::scene::character::variant::{character_variant, CharacterVariant, Group};
use crate::domain::scene::frame::Frame;
use crate::sys::io::loader::load_image;
use crate::sys::load::Load;
use crate::sys::mem::section;
use crate::sys::sched::edge::request_world_tick;
use crate::sys::sched::tick::TickContext;
use pxl_render::frame::draw::DrawList;
use pxl_spec::asset::actor::IDLE_CLIP;
use pxl_spec::space::coord::v3::V3;
use xpute_guest::global::Global;

const SPIN_RATE_RAD_S: f64 = 0.6;
/// Under the sea below the eye, past any terrain, so the world never draws it.
const CASE_DEPTH_U: f64 = 100_000.0;

struct Case {
    id: u32,
    variant: CharacterVariant,
    actor: Option<Actor>,
    /// Where its actor stands, once held.
    stance: Option<Stance>,
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
        stance: None,
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
        if section::map().is_some() {
            actor.free(draw_list());
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
            case.model = model_of(case.variant.model, Some(case.variant.height_u));
        }
        if case.actor.is_none() {
            if let (Some(model), Some(_)) = (&case.model, load_image(Load::File(case.variant.skin))) {
                let id = model.model;
                case.actor = section::map().and_then(|_| Actor::new(draw_list(), crate::domain::query::model::models(), id));
                if let (Some(actor), true) = (case.actor.as_mut(), IDLE_CLIP < model.clip_count) {
                    actor.play(IDLE_CLIP, 0.0);
                }
            }
        }
        let (Some(_), Some(model)) = (&case.actor, &case.model) else { continue };
        case.spin += SPIN_RATE_RAD_S * ctx.delta;
        // The model as registered, the variant's height tall, at one unit.
        let scale = model.scale / case.variant.height_u as f32;
        case.stance = Some(Stance {
            at,
            rotation_z: case.spin as f32,
            scale,
        });
        turned = true;
    }
    if turned {
        ctx.invalidate();
    }
}

/// Each case's actor advanced where it stands; whether one the eye can see
/// moved (`Actor::tick`).
pub fn preview_animate(draw: &mut DrawList, models: &Models, frame: &Frame) -> bool {
    let mut moved = false;
    for case in &mut cases().cases {
        if let (Some(actor), Some(stance)) = (case.actor.as_mut(), case.stance) {
            moved |= actor.tick(draw, models, frame, &stance);
        }
    }
    moved
}

/// Each case's actor into the frame's list, where it stands.
pub fn preview_draw(draw: &mut DrawList, models: &Models, eye: V3) {
    for case in &cases().cases {
        if let (Some(actor), Some(stance)) = (&case.actor, case.stance) {
            actor.draw(draw, models, &stance, eye);
        }
    }
}

/// (case, its actor's palette, skin image), the image as the loader has it
/// now: the palette names the case's own draw, as its model is the
/// companion's too.
pub fn preview_rows() -> impl Iterator<Item = (u32, u32, (u32, &'static [u8]))> {
    let c: &'static _ = cases();
    c.cases
        .iter()
        .filter_map(|case| Some((case.id, case.actor.as_ref()?.palette(), load_image(Load::File(case.variant.skin))?)))
}
