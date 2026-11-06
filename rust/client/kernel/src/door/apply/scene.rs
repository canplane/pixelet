// pxl-kernel/door/apply/scene.rs

//! SCENE: the scene's: what a surface draws, a line said.

use super::State;
use super::{ask_turn, or_refuse, refuse_unless};
use crate::domain::control::cmd as abi;
use crate::domain::scene::character::preview;
use crate::domain::scene::marker;
use crate::domain::view::frame::resolution;
use pxl_render::api_spec::surface_role;
use pxl_render::gpu::surface;
use xpute_core::ipc::door::Outcome;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

pub(super) fn chat(s: &mut State, c: abi::SceneChat<'_>) -> Outcome {
    marker::append_chat_line(c.text);
    ask_turn(s)
}

/// What a surface draws until it closes: the world, or a character alone,
/// which a second replaces. Refused where the surface is not open (EBADF),
/// already draws the other content (EBUSY), or the character's fields do not
/// name one or are given for the world (EINVAL).
pub(super) fn draw(s: &mut State, c: abi::SceneDraw) -> Outcome {
    let role = or_refuse(
        surface::surface(c.surface).map(|open| open.role),
        Errno::EBADF,
    )?;
    match c.content {
        abi::SceneContent::World => {
            refuse_unless(
                c.group.is_none() && c.number.is_none() && c.bg.is_none(),
                Errno::EINVAL,
            )?;
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
            let (Some(group), Some(number), Some(bg)) = (c.group, c.number, c.bg) else {
                return Err(MarshalError::new(Errno::EINVAL));
            };
            refuse_unless(
                role == surface_role::NONE || role == surface_role::PREVIEW,
                Errno::EBUSY,
            )?;
            refuse_unless(
                preview::preview_open(c.surface, group, number, bg),
                Errno::EINVAL,
            )?;
            if let Some(drawn) = surface::surface(c.surface) {
                drawn.role = surface_role::PREVIEW;
                if drawn.px_w != 0 {
                    drawn.ratio =
                        resolution::surface_ratio(drawn.role, [drawn.px_w, drawn.px_h], drawn.dpr);
                }
            }
            ask_turn(s)
        }
    }
}
