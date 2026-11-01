// pxl-kernel/door/apply/scene.rs

//! SCENE: the scene's: a character drawn alone into a surface, a line said.

use super::State;
use super::{ask_turn, given, refuse_unless};
use crate::domain::control::cmd::{self as abi, Command};
use crate::domain::scene::character::preview;
use crate::domain::scene::marker;
use pxl_render::gpu::surface;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

pub(super) fn apply(
    s: &mut State,
    cmd: Command,
    pkt: &Option<&[u8]>,
) -> Result<Option<i32>, MarshalError> {
    match cmd {
        Command::SCENE_CHAT => {
            marker::append_chat_line(abi::SceneChat::read(given(pkt)?)?.text);
            ask_turn(s)
        }
        Command::SCENE_PREVIEW_OPEN => {
            let p = abi::ScenePreviewOpen::read(given(pkt)?)?;
            refuse_unless(surface::surface(p.surface).is_some(), Errno::EBADF)?;
            refuse_unless(
                preview::preview_open(p.surface, p.group, p.number),
                Errno::EINVAL,
            )?;
            ask_turn(s)
        }
        Command::SCENE_PREVIEW_CLOSE => {
            refuse_unless(
                preview::preview_close(abi::ScenePreviewClose::read(given(pkt)?)?.surface),
                Errno::EBADF,
            )?;
            ask_turn(s)
        }
        _ => Ok(None),
    }
}
