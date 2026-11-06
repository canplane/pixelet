// pxl-kernel/domain/scene/character/preview.rs

//! Previews for the character picker: a character idling and spinning in
//! place, drawn into the preview's own canvas from far below the world. A preview
//! is a holder of its actor's instance: the character's one registered model,
//! which the instance's transform scales to one unit tall, since the previews
//! compare silhouettes, not scale.

use crate::domain::scene::actor::{Actor, Transform};
use crate::domain::scene::character::{Character, Group};
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
use xpute_guest::global::Global;

const SPIN_RATE_RAD_S: f64 = 0.6;
/// Under the sea below the eye, past any terrain, so the world never draws it.
const CASE_DEPTH_U: f64 = 100_000.0;

struct Preview {
    id: u32,
    character: Character,
    /// What it clears to, 0xRRGGBB, sRGB.
    bg: u32,
    actor: Option<Actor>,
    /// Where its actor stands, once held.
    transform: Option<Transform>,
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
/// surface or no character.
pub fn preview_open(id: u32, group: Group, number: u8, bg: u32) -> bool {
    preview_close(id);
    let Some(character) = Character::of(group, number) else {
        return false;
    };
    previews().previews.push(Preview {
        id,
        character,
        bg,
        actor: None,
        transform: None,
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
    // Under the sea below the eye, out of sight of the world, and drawn
    // about its own Q3 (pxl-render gpu/pass/preview.rs `draw`), so a
    // unit-tall model resolves wherever it stands.
    let mut at = pose::sea_under_eye();
    at[2] -= CASE_DEPTH_U;
    let mut turned = false;
    for preview in &mut c.previews {
        if preview.model.is_none() {
            preview.model = model_of(preview.character.model, Some(preview.character.height_u));
        }
        if preview.actor.is_none() {
            if let (Some(model), Some(_)) = (
                &preview.model,
                load_of(preview.character.skin).and_then(load_image),
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
        // The model as registered, the character's height tall, at one unit.
        let scale = model.scale / preview.character.height_u as f32;
        preview.transform = Some(Transform {
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
        if let (Some(actor), Some(transform)) = (preview.actor.as_mut(), preview.transform) {
            moved |= actor.tick(draw, models, frame, &transform);
        }
    }
    moved
}

/// Each preview's actor into the frame's list, where it stands.
pub fn preview_draw(draw: &mut DrawList, models: &Models) {
    for preview in &previews().previews {
        if let (Some(actor), Some(transform)) = (&preview.actor, preview.transform) {
            actor.draw(draw, models, &transform);
        }
    }
}

/// (preview, its actor's palette, skin image, what it clears to), the image as
/// the loader has it now: the palette names the preview's own draw, as its
/// model is the companion's too.
pub fn preview_rows() -> impl Iterator<Item = (u32, u32, (u32, &'static [u8]), u32)> {
    let c: &'static _ = previews();
    c.previews.iter().filter_map(|preview| {
        Some((
            preview.id,
            preview.actor.as_ref()?.palette(),
            load_of(preview.character.skin).and_then(load_image)?,
            preview.bg,
        ))
    })
}
