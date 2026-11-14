// pxl-core/abi/edit.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/edit` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x453522;

/// `hold`
pub const HOLD: _rt::Method = 0;
/// `marker-text`
pub const MARKER_TEXT: _rt::Method = 1;
/// `marker-cancel`
pub const MARKER_CANCEL: _rt::Method = 2;
/// `marker-commit`
pub const MARKER_COMMIT: _rt::Method = 3;

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

pub trait Guest {
    fn hold(&self, tool: Tool, val: u32, scale_step: i32) -> Result<(), _rt::Errno>;
    fn marker_text(&self, text: String) -> Result<Result<(), super::types::ErrorCode>, _rt::Errno>;
    fn marker_cancel(&self) -> Result<(), _rt::Errno>;
    fn marker_commit(&self) -> Result<(), _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Guest>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                HOLD => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let tool = r_
                        .case(0, 3)
                        .and_then(|v| Tool::of(v).ok_or(_rt::Errno::EBADMSG))?;
                    let val = r_.u32(4)?;
                    let scale_step = r_.i32(8)?;
                    match self.0.hold(tool, val, scale_step) {
                        Ok(()) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                MARKER_TEXT => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let text = r_.str(0).map(str::to_owned)?;
                    match self.0.marker_text(text) {
                        Ok(v) => _rt::Reply {
                            result: 0,
                            data: Some((
                                2,
                                Box::new(move |w: &mut _rt::Lower| match &v {
                                    Ok(_) => {}
                                    Err(v) => {
                                        w.u8(0, 1);
                                        w.u8(1, *v as u8);
                                    }
                                }),
                            )),
                        },
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                MARKER_CANCEL => match self.0.marker_cancel() {
                    Ok(()) => _rt::Reply::from(0),
                    Err(e) => _rt::Reply::from(e as i32),
                },
                MARKER_COMMIT => match self.0.marker_commit() {
                    Ok(()) => _rt::Reply::from(0),
                    Err(e) => _rt::Reply::from(e as i32),
                },
                _ => _rt::Reply::from(_rt::Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| _rt::Reply::from(e as i32))
    }
}

/// Serves `implementation` under the interface's handle.
pub fn mount(implementation: _rt::Rc<dyn Guest>) {
    _rt::link::mount_named(HANDLE, _rt::Rc::new(_Object(implementation)));
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_guest::link;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Reply, Tag};
}
