// pxl-core/door/apply/edit.rs

//! EDIT: the edit's: the tool and what it takes, and the marker it left waiting.

use super::or_refuse;
use crate::domain::control::edit::set_tool;
use crate::domain::control::edit::write;
use crate::domain::control::edit::Tool;
use crate::domain::control::input::camera_rehover;
use crate::domain::layer::object::asset::Asset;
use crate::door::cmd as abi;
use xpute_core::ipc::door::Outcome;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

pub(super) fn tool(c: abi::EditTool) -> Outcome {
    set_tool(match c.tool {
        abi::Tool::Pointer => Tool::Pointer,
        abi::Tool::Eraser => Tool::Eraser,
        abi::Tool::Item => match Asset::of(c.val) {
            Asset::Unknown => return Err(MarshalError::new(Errno::EINVAL)),
            asset => Tool::Item {
                asset,
                scale_steps: c.scale_steps,
            },
        },
    });
    camera_rehover();
    Ok(Some(0))
}

pub(super) fn marker_text(c: abi::EditMarkerText<'_>) -> Outcome {
    or_refuse(write::set_draft_text(c.text).ok(), Errno::EMSGSIZE)?;
    Ok(Some(0))
}

pub(super) fn marker_commit() -> Outcome {
    write::commit_draft();
    Ok(Some(0))
}

pub(super) fn marker_cancel() -> Outcome {
    write::cancel_draft();
    Ok(Some(0))
}
