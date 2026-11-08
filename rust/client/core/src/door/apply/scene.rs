// pxl-core/door/apply/scene.rs

//! The SCENE major's commands: what a surface draws, and a chat line said.

use super::State;
use super::{ask_turn, or_refuse, refuse_unless};
use crate::domain::scene::character::surface::character_surface_open;
use crate::domain::scene::marker;
use crate::domain::scene::resolution;
use crate::door::cmd as abi;
use pxl_render::api_spec::surface_role;
use pxl_render::gpu::surface;
use xpute_core::ipc::door::Outcome;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

pub(super) fn chat(s: &mut State, c: abi::SceneChat<'_>) -> Outcome {
    marker::append_chat_line(c.text);
    ask_turn(s)
}

/// Sets what a surface draws until it closes: the world, or one group's
/// companion character alone, which a second command replaces. It is refused
/// with EBADF when the surface is not open, with EBUSY when the surface
/// already draws the other content, and with EINVAL when the character's
/// fields are missing for a character or given for the world.
pub(super) fn draw(s: &mut State, c: abi::SceneDraw) -> Outcome {
    let role = or_refuse(
        surface::surface(c.surface).map(|open| open.role),
        Errno::EBADF,
    )?;
    match c.content {
        abi::SceneContent::World => {
            refuse_unless(c.group.is_none() && c.bg.is_none(), Errno::EINVAL)?;
            refuse_unless(
                role == surface_role::NONE || role == surface_role::WORLD,
                Errno::EBUSY,
            )?;
            if role == surface_role::NONE {
                super::surface::draw_world(c.surface);
            }
            Ok(Some(0))
        }
        abi::SceneContent::Character => {
            let (Some(group), Some(bg)) = (c.group, c.bg) else {
                return Err(MarshalError::new(Errno::EINVAL));
            };
            refuse_unless(
                role == surface_role::NONE || role == surface_role::CHARACTER,
                Errno::EBUSY,
            )?;
            character_surface_open(c.surface, group, bg);
            if let Some(drawn) = surface::surface(c.surface) {
                drawn.role = surface_role::CHARACTER;
                if drawn.px_w != 0 {
                    drawn.ratio =
                        resolution::surface_ratio(drawn.role, [drawn.px_w, drawn.px_h], drawn.dpr);
                }
            }
            ask_turn(s)
        }
    }
}
