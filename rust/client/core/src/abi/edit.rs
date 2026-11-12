// pxl-core/abi/edit.rs
//
// GENERATED from spec/client/core/wit (xpute-gen spec/wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use std::cell::Cell;
use std::rc::Rc;
use xpute_core::link::{self, Handle, Method, Object, Reply, Return, Tag};
use xpute_core::wait::wait;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

/// The singleton's URI.
pub const URI: [&str; 2] = ["pxl:core", "edit"];

/// `hold`
pub const HOLD: Method = 0;
/// `marker-text`
pub const MARKER_TEXT: Method = 1;
/// `marker-cancel`
pub const MARKER_CANCEL: Method = 2;
/// `marker-commit`
pub const MARKER_COMMIT: Method = 3;

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Pointer = 0,
    Item = 1,
    Eraser = 2,
}

impl Tool {
    pub const ALL: [Tool; 3] = [Tool::Pointer, Tool::Item, Tool::Eraser];

    pub fn of(v: u32) -> Option<Tool> {
        Tool::ALL.get(v as usize).copied()
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

/// Mounts `imp` at the URI, one object for every open.
pub fn mount(imp: Rc<dyn Edit>) {
    let o: Rc<dyn Object> = Rc::new(EditObject(imp));
    link::mount(link::uri(&URI), move |_, _| Ok(o.clone()));
}

thread_local! {
    static HANDLE: Cell<Option<Handle>> = const { Cell::new(None) };
}

/// Opens the singleton on the other side, once, at boot.
pub fn open() -> Result<(), Errno> {
    HANDLE.set(Some(link::open(&link::uri(&URI))?));
    Ok(())
}

/// The singleton's handle, which `open` opened.
pub fn handle() -> Handle {
    HANDLE.get().or_bug(Errno::EBADF)
}

pub fn hold(tool: Tool, val: u32, scale_steps: i32) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        HOLD,
        Some((3, &|w: &mut PacketWriter| {
            w.u32(Some(tool as u32));
            w.u32(Some(val as u32));
            w.i32(Some(scale_steps as i32));
        })),
    );
    link::answer(r).map(|_| ())
}

pub fn marker_text(text: &str) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        MARKER_TEXT,
        Some((1, &|w: &mut PacketWriter| {
            w.str(Some(&*text));
        })),
    );
    link::answer(r).map(|_| ())
}

pub fn marker_cancel() -> Result<(), Errno> {
    let (r, data) = link::call(handle(), MARKER_CANCEL, None);
    link::answer(r).map(|_| ())
}

pub fn marker_commit() -> Result<(), Errno> {
    let (r, data) = link::call(handle(), MARKER_COMMIT, None);
    link::answer(r).map(|_| ())
}
