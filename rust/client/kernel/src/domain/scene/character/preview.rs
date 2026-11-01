// pxl-kernel/domain/scene/character/preview.rs

//! Previews for the variant picker: a character idling and spinning in
//! place, drawn into the preview's own canvas from far below the world. A preview
//! is a holder of its actor's instance: the variant's one registered model,
//! which the instance's stance scales to one unit tall, since the previews
//! compare silhouettes, not scale.

use crate::domain::scene::actor::{Actor, Stance};
use crate::domain::scene::character::variant::{character_variant, CharacterVariant, Group};
use crate::domain::scene::frame::Frame;
use crate::domain::scene::model;
use crate::domain::scene::model::asset::{model_of, ModelHandle};
use crate::domain::scene::model::draw_list;
use crate::domain::scene::model::registry::Models;
use crate::domain::view::frame::pose;
use crate::sys::io::loader::{load_image, load_of};
use crate::sys::mem::section;
use crate::sys::sched::edge::request_world_tick;
use crate::sys::sched::tick::TickContext;
use pxl_render::frame::draw::DrawList;
use pxl_render::gpu::surface;
use pxl_spec::asset::actor::IDLE_CLIP;
use pxl_spec::space::coord::v3::V3;
use xpute_guest::global::Global;

const SPIN_RATE_RAD_S: f64 = 0.6;
/// Under the sea below the eye, past any terrain, so the world never draws it.
const CASE_DEPTH_U: f64 = 100_000.0;

struct Preview {
    id: u32,
    variant: CharacterVariant,
    actor: Option<Actor>,
    /// Where its actor stands, once held.
    stance: Option<Stance>,
    model: Option<ModelHandle>,
    spin: f64,
}

struct Previews {
    previews: Vec<Preview>,
}

static PREVIEWS: Global<Previews> = Global::new();

fn previews() -> &'static mut Previews {
    PREVIEWS.get(|| Previews {
        previews: Vec::new(),
    })
}

/// One per surface: a preview is named by the surface it draws on.
pub const PREVIEWS_MAX: usize = surface::SURFACES;

/// Replaces a preview already open under `id`. False where `id` names no
/// surface or no variant.
/// On a live surface; false for a variant there is none of.
pub fn preview_open(id: u32, group: Group, number: u8) -> bool {
    preview_close(id);
    let Some(variant) = character_variant(group, number) else {
        return false;
    };
    previews().previews.push(Preview {
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

/// Whether a preview was open there.
pub fn preview_close(id: u32) -> bool {
    let c = previews();
    let Some(pos) = c.previews.iter().position(|preview| preview.id == id) else {
        return false;
    };
    let preview = c.previews.remove(pos);
    if let Some(actor) = preview.actor {
        if section::map().is_some() {
            actor.free(draw_list());
        }
    }
    true
}

/// An open preview is what asks the world's tick to run, while it turns: one
/// still loading draws nothing and asks nothing, its model or skin landing
/// waking the turn that lays it.
pub fn preview_step(ctx: &mut TickContext) {
    let c = previews();
    if c.previews.is_empty() {
        return;
    }
    // Under the sea below the eye, out of sight and near it, where the f32 of
    // its draw resolves a unit-tall model.
    let mut at = pose::sea_under_eye();
    at[2] -= CASE_DEPTH_U;
    let mut turned = false;
    for preview in &mut c.previews {
        if preview.model.is_none() {
            preview.model = model_of(preview.variant.model, Some(preview.variant.height_u));
        }
        if preview.actor.is_none() {
            if let (Some(model), Some(_)) = (
                &preview.model,
                load_of(preview.variant.skin).and_then(load_image),
            ) {
                let id = model.model;
                preview.actor =
                    section::map().and_then(|_| Actor::new(draw_list(), model::models(), id));
                if let (Some(actor), true) = (preview.actor.as_mut(), IDLE_CLIP < model.clip_cnt) {
                    actor.play(IDLE_CLIP, 0.0);
                }
            }
        }
        let (Some(_), Some(model)) = (&preview.actor, &preview.model) else {
            continue;
        };
        preview.spin += SPIN_RATE_RAD_S * ctx.delta;
        // The model as registered, the variant's height tall, at one unit.
        let scale = model.scale / preview.variant.height_u as f32;
        preview.stance = Some(Stance {
            at,
            rot_z: preview.spin as f32,
            scale,
        });
        turned = true;
    }
    if turned {
        ctx.invalidate();
    }
}

/// Each preview's actor advanced where it stands; whether one the eye can see
/// moved (`Actor::tick`).
pub fn preview_animate(draw: &mut DrawList, models: &Models, frame: &Frame) -> bool {
    let mut moved = false;
    for preview in &mut previews().previews {
        if let (Some(actor), Some(stance)) = (preview.actor.as_mut(), preview.stance) {
            moved |= actor.tick(draw, models, frame, &stance);
        }
    }
    moved
}

/// Each preview's actor into the frame's list, where it stands.
pub fn preview_draw(draw: &mut DrawList, models: &Models, eye: V3) {
    for preview in &previews().previews {
        if let (Some(actor), Some(stance)) = (&preview.actor, preview.stance) {
            actor.draw(draw, models, &stance, eye);
        }
    }
}

/// (preview, its actor's palette, skin image), the image as the loader has it
/// now: the palette names the preview's own draw, as its model is the
/// companion's too.
pub fn preview_rows() -> impl Iterator<Item = (u32, u32, (u32, &'static [u8]))> {
    let c: &'static _ = previews();
    c.previews.iter().filter_map(|preview| {
        Some((
            preview.id,
            preview.actor.as_ref()?.palette(),
            load_of(preview.variant.skin).and_then(load_image)?,
        ))
    })
}
