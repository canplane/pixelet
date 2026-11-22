// pxl-core/link/export/edit.rs

//! The edit interface's methods: the tool held and what it places, and the
//! marker draft it left waiting.

use crate::abi::edit;
use crate::abi::types::ErrorCode;
use crate::domain::control::edit::Tool;
use crate::domain::control::edit::set_tool;
use crate::domain::control::edit::write;
use crate::domain::control::input::camera_rehover;
use crate::domain::layer::object::asset::Asset;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::XputeError;

pub(super) fn hold(tool: edit::Tool, val: u32, scale_step: i32) -> Result<(), XputeError> {
    set_tool(match tool {
        edit::Tool::Pointer => Tool::Pointer,
        edit::Tool::Eraser => Tool::Eraser,
        edit::Tool::Item => match Asset::of(val) {
            Asset::Unknown => return Err(Errno::EINVAL.into()),
            asset => Tool::Item {
                asset,
                zoom: asset.zoom(scale_step),
            },
        },
    });
    // The tool's preview stands where the pointer hovers.
    camera_rehover();
    Ok(())
}

pub(super) fn marker_text(text: String) -> Result<Result<(), ErrorCode>, XputeError> {
    Ok(write::set_draft_text(&text).map_err(|_| ErrorCode::TooLong))
}

pub(super) fn marker_commit() -> Result<(), XputeError> {
    write::commit_draft();
    Ok(())
}

pub(super) fn marker_cancel() -> Result<(), XputeError> {
    write::cancel_draft();
    Ok(())
}
