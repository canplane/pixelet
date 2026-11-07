// pxl-kernel/domain/scene/character/surface.rs

//! The character surfaces: what each canvas of the character picker shows, as
//! a world surface shows the world. Each shows the companion's person or pet
//! as it is, by an eye of its own, over the color it clears to
//! (`companion_surface_draw`, pxl-render gpu/pass/character.rs).

use crate::domain::scene::character::companion::companion_surface_draw;
use crate::domain::scene::character::Group;
use crate::sys::sched::edge::request_world_tick;
use pxl_render::gpu::api::Image;
use pxl_render::gpu::draw::Draw;
use xpute_core::global::Global;

/// Each: its surface, the group of the companion's character it shows, and
/// what it clears to, 0xRRGGBB, sRGB.
static SURFACES: Global<Vec<(u32, Group, u32)>> = Global::new();

fn surfaces() -> &'static mut Vec<(u32, Group, u32)> {
    SURFACES.get(Vec::new)
}

/// Surface `id` shows the companion's character of `group` over `bg`, in
/// place of what it showed.
pub fn character_surface_open(id: u32, group: Group, bg: u32) {
    character_surface_close(id);
    surfaces().push((id, group, bg));
    request_world_tick();
}

/// Whether it showed a character.
pub fn character_surface_close(id: u32) -> bool {
    let s = surfaces();
    let Some(pos) = s.iter().position(|s| s.0 == id) else {
        return false;
    };
    s.remove(pos);
    true
}

/// Whether a surface shows the person, then the pet.
pub fn character_surface_shows() -> [bool; 2] {
    Group::ALL.map(|group| surfaces().iter().any(|s| s.1 == group))
}

/// (surface, its character's draw there, skin image, what it clears to), the
/// image as the loader has it now.
pub fn character_surface_rows() -> impl Iterator<Item = (u32, Draw, Image, u32)> {
    let s: &'static _ = surfaces();
    s.iter().filter_map(|&(id, group, bg)| {
        let (draw, skin) = companion_surface_draw(group)?;
        Some((id, draw, skin, bg))
    })
}
