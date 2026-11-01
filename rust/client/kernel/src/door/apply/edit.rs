// pxl-kernel/door/apply/edit.rs

//! EDIT: the edit's: the tool and what it takes, and the marker it left waiting.

use super::State;
use super::{given, or_refuse};
use crate::domain::control::camera::input::camera_rehover;
use crate::domain::control::cmd::{self as abi, Command};
use crate::domain::control::edit::set_tool;
use crate::domain::control::edit::tool;
use crate::domain::control::edit::write;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

pub(super) fn apply(
    _s: &mut State,
    cmd: Command,
    pkt: &Option<&[u8]>,
) -> Result<Option<i32>, MarshalError> {
    match cmd {
        Command::EDIT_TOOL => {
            use pxl_spec::asset::cube::cube_of;
            use pxl_spec::asset::DecorId;
            let p = abi::EditTool::read(given(pkt)?)?;
            let mut tool = tool();
            tool.active_tool = p.tool;
            tool.object_tier = p.tier;
            match p.tool {
                abi::ActiveTool::Object => {
                    tool.object_cube = or_refuse(cube_of(p.val), Errno::EINVAL)?;
                }
                abi::ActiveTool::Decor => {
                    tool.decor_id = or_refuse(
                        u16::try_from(p.val).ok().and_then(DecorId::of),
                        Errno::EINVAL,
                    )?;
                }
                abi::ActiveTool::Pointer | abi::ActiveTool::Eraser | abi::ActiveTool::Marker => {}
            }
            set_tool(tool);
            camera_rehover();
            Ok(Some(0))
        }
        Command::EDIT_MARKER_TEXT => {
            let text = abi::EditMarkerText::read(given(pkt)?)?.text;
            or_refuse(write::set_draft_text(text).ok(), Errno::EMSGSIZE)?;
            Ok(Some(0))
        }
        Command::EDIT_MARKER_COMMIT => {
            write::commit_draft();
            Ok(Some(0))
        }
        Command::EDIT_MARKER_CANCEL => {
            write::cancel_draft();
            Ok(Some(0))
        }
        _ => Ok(None),
    }
}
