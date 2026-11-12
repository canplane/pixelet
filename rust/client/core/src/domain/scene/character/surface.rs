// pxl-core/domain/scene/character/surface.rs

//! The character surfaces. Each canvas of the character picker is a surface
//! that shows the companion's person or pet as it is now, the way a world
//! surface shows the world. A surface draws its character by an eye of its
//! own, over the color it clears to (`companion_surface_draw`, pxl-render
//! pass/character.rs).

use crate::domain::scene::character::companion::companion_surface_draw;
use crate::domain::scene::character::Group;
use crate::sys::sched::invalidate;
use pxl_render::gpu::api::Image;
use pxl_render::gpu::draw::Draw;
use pxl_global::Global;

/// The open surfaces: each surface's id, the group of the companion's
/// character it shows, and the color it clears to as sRGB 0xRRGGBB.
static SURFACES: Global<Vec<(u32, Group, u32)>> = Global::new();

fn surfaces() -> &'static mut Vec<(u32, Group, u32)> {
    SURFACES.get(Vec::new)
}

/// Makes surface `id` show the companion's character of `group` over `bg`,
/// in place of whatever it showed.
pub fn character_surface_open(id: u32, group: Group, bg: u32) {
    character_surface_close(id);
    surfaces().push((id, group, bg));
    invalidate();
}

/// Stops surface `id` showing a character, and returns whether it showed
/// one.
pub fn character_surface_close(id: u32) -> bool {
    let s = surfaces();
    let Some(pos) = s.iter().position(|s| s.0 == id) else {
        return false;
    };
    s.remove(pos);
    true
}

/// Whether any surface shows the person, and whether any shows the pet.
pub fn character_surface_shows() -> [bool; 2] {
    Group::ALL.map(|group| surfaces().iter().any(|s| s.1 == group))
}

/// For each surface whose character can be drawn: the surface's id, its
/// character's draw there, the skin image as the loader has it now, and the
/// color the surface clears to.
pub fn character_surface_rows() -> impl Iterator<Item = (u32, Draw, Image, u32)> {
    let s: &'static _ = surfaces();
    s.iter().filter_map(|&(id, group, bg)| {
        let (draw, skin) = companion_surface_draw(group)?;
        Some((id, draw, skin, bg))
    })
}
