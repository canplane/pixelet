// pxl-kernel/domain/scene/character/preview.rs

//! What each of the character picker's preview surfaces shows: the
//! companion's person or pet as it is, drawn by the preview's own eye into
//! the surface, over the color it clears to (`companion_preview`, pxl-render
//! gpu/pass/preview.rs).

use crate::domain::scene::character::companion::companion_preview;
use crate::domain::scene::character::Group;
use crate::sys::sched::edge::request_world_tick;
use pxl_render::frame::draw::Draw;
use pxl_render::gpu::api::Image;
use pxl_render::gpu::surface;
use xpute_core::global::Global;

/// One per surface: a preview is named by the surface it draws on.
pub const PREVIEWS_MAX: usize = surface::SURFACES;

/// Each preview: its surface, the group of the companion's character it
/// shows, and what it clears to, 0xRRGGBB, sRGB.
static PREVIEWS: Global<Vec<(u32, Group, u32)>> = Global::new();

fn previews() -> &'static mut Vec<(u32, Group, u32)> {
    PREVIEWS.get(Vec::new)
}

/// Surface `id` shows the companion's character of `group` over `bg`, in
/// place of what it showed.
pub fn preview_open(id: u32, group: Group, bg: u32) {
    preview_close(id);
    previews().push((id, group, bg));
    request_world_tick();
}

/// Whether a preview was open there.
pub fn preview_close(id: u32) -> bool {
    let p = previews();
    let Some(pos) = p.iter().position(|p| p.0 == id) else {
        return false;
    };
    p.remove(pos);
    true
}

/// Whether a preview shows the person, then the pet.
pub fn preview_shows() -> [bool; 2] {
    Group::ALL.map(|group| previews().iter().any(|p| p.1 == group))
}

/// (preview, its character's draw there, skin image, what it clears to),
/// the image as the loader has it now.
pub fn preview_rows() -> impl Iterator<Item = (u32, Draw, Image, u32)> {
    let p: &'static _ = previews();
    p.iter().filter_map(|&(id, group, bg)| {
        let (draw, skin) = companion_preview(group)?;
        Some((id, draw, skin, bg))
    })
}
