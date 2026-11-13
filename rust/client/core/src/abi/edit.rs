// pxl-core/abi/edit.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use std::rc::Rc;
use xpute_guest::link;
use xpute_guest::wait::wait;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `pxl:core/edit` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x453522;

/// The interface's qualified name: its package and its name.
pub const NAME: [&str; 2] = ["pxl:core", "edit"];

/// `hold`
pub const HOLD: Method = 0;
/// `marker-text`
pub const MARKER_TEXT: Method = 1;
/// `marker-cancel`
pub const MARKER_CANCEL: Method = 2;
/// `marker-commit`
pub const MARKER_COMMIT: Method = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Tool {
    Pointer = 0,
    Item = 1,
    Eraser = 2,
}

impl Tool {
    /// Every member, in its source's own order.
    pub const ALL: [Tool; 3] = [Tool::Pointer, Tool::Item, Tool::Eraser];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<Tool> {
        match val {
            0 => Some(Tool::Pointer),
            1 => Some(Tool::Item),
            2 => Some(Tool::Eraser),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Tool::Pointer => "Pointer",
            Tool::Item => "Item",
            Tool::Eraser => "Eraser",
        }
    }
}

pub trait Edit {
    fn hold(&self, tool: Tool, val: u32, scale_steps: i32) -> Result<(), Errno>;
    fn marker_text(&self, text: String) -> Result<(), Errno>;
    fn marker_cancel(&self) -> Result<(), Errno>;
    fn marker_commit(&self) -> Result<(), Errno>;
}

struct EditObject(Rc<dyn Edit>);

impl Object for EditObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                HOLD => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let tool = b.at(0).and_then(|c| {
                        c.get_number().and_then(|v| {
                            Tool::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                        })
                    })?;
                    let val = b.at(1).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let scale_steps = b.at(2).and_then(|c| c.get_number().map(|v| v as i32))?;
                    match self.0.hold(tool, val, scale_steps) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                MARKER_TEXT => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let text = b.at(0).and_then(|c| c.get_str_ref().map(str::to_owned))?;
                    match self.0.marker_text(text) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                MARKER_CANCEL => match self.0.marker_cancel() {
                    Ok(v) => Reply::from(0),
                    Err(e) => Reply::from(e as i32),
                },
                MARKER_COMMIT => match self.0.marker_commit() {
                    Ok(v) => Reply::from(0),
                    Err(e) => Reply::from(e as i32),
                },
                _ => Reply::from(Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
    }
}

/// Serves `imp` under the interface's handle.
pub fn mount(imp: Rc<dyn Edit>) {
    link::mount_named(HANDLE, Rc::new(EditObject(imp)));
}

pub fn hold(tool: Tool, val: u32, scale_steps: i32) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        HOLD,
        Some((3, &|w: &mut PacketWriter| {
            w.u32(Some(tool as u32));
            w.u32(Some(val as u32));
            w.i32(Some(scale_steps as i32));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}

pub fn marker_text(text: &str) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        MARKER_TEXT,
        Some((1, &|w: &mut PacketWriter| {
            w.str(Some(&*text));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}

pub fn marker_cancel() -> Result<(), Errno> {
    let (r, data) = link::call(HANDLE, MARKER_CANCEL, None);
    xpute_link::answer(r).map(|_| ())
}

pub fn marker_commit() -> Result<(), Errno> {
    let (r, data) = link::call(HANDLE, MARKER_COMMIT, None);
    xpute_link::answer(r).map(|_| ())
}
