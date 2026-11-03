// pxl-kernel/door/apply/edit.rs

//! EDIT: the edit's: the tool and what it takes, and the marker it left waiting.

use super::State;
use super::{given, or_refuse};
use crate::domain::control::camera::input::camera_rehover;
use crate::domain::control::cmd::{self as abi, Command};
use crate::domain::control::edit::set_tool;
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
            use crate::domain::control::edit::Tool;
            use crate::domain::layer::object::asset::Asset;
            let p = abi::EditTool::read(given(pkt)?)?;
            set_tool(match p.tool {
                abi::Tool::Pointer => Tool::Pointer,
                abi::Tool::Eraser => Tool::Eraser,
                abi::Tool::Item => match Asset::of(p.val) {
                    Asset::Unknown => return Err(MarshalError::new(Errno::EINVAL)),
                    asset => Tool::Item {
                        asset,
                        scale_steps: p.scale_steps,
                    },
                },
            });
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
