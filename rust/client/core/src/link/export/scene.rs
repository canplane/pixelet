// pxl-core/link/export/scene.rs

//! The SCENE major's commands: what a surface draws, and a chat line said.

use super::{applied, or_refuse, refuse_unless};
use crate::domain::scene::character::surface::character_surface_open;
use crate::domain::scene::marker;
use crate::domain::scene::resolution;
use crate::abi::scene::SceneContent;
use pxl_spec::asset::character::Group;
use pxl_render::api_spec::surface_role;
use pxl_render::gpu::surface;
use xpute_kit::status::errno::Errno;

pub(super) fn chat(text: String) -> Result<(), Errno> {
    marker::append_chat_line(&text);
    applied()
}

/// Sets what a surface draws until it closes: the world, or one group's
/// companion character alone, which a second command replaces. It is refused
/// with EBADF when the surface is not open, with EBUSY when the surface
/// already draws the other content, and with EINVAL when the character's
/// fields are missing for a character or given for the world.
pub(super) fn draw(surface_at: u32, content: SceneContent, group: Option<u8>, bg: Option<u32>) -> Result<(), Errno> {
    let role = or_refuse(
        surface::surface(surface_at).map(|open| open.role),
        Errno::EBADF,
    )?;
    match content {
        SceneContent::World => {
            refuse_unless(group.is_none() && bg.is_none(), Errno::EINVAL)?;
            refuse_unless(
                role == surface_role::NONE || role == surface_role::WORLD,
                Errno::EBUSY,
            )?;
            if role == surface_role::NONE {
                super::surface::draw_world(surface_at);
            }
            Ok(())
        }
        SceneContent::Character => {
            let (Some(group), Some(bg)) = (group, bg) else {
                return Err(Errno::EINVAL);
            };
            let group = or_refuse(Group::of(group), Errno::EINVAL)?;
            refuse_unless(
                role == surface_role::NONE || role == surface_role::CHARACTER,
                Errno::EBUSY,
            )?;
            character_surface_open(surface_at, group, bg);
            if let Some(character) = surface::surface(surface_at) {
                character.role = surface_role::CHARACTER;
                if character.px_w != 0 {
                    character.ratio = resolution::surface_ratio(
                        character.role,
                        [character.px_w, character.px_h],
                        character.dpr,
                    );
                }
            }
            applied()
        }
    }
}
