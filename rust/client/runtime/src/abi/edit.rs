// pxl-runtime/abi/edit.rs
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

pub fn hold(side_: &_rt::Side, tool: Tool, val: u32, scale_step: i32) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(
        HANDLE,
        HOLD,
        _rt::Packet::Args(Some((12, &|w_: &mut _rt::Lower| {
            w_.u8(0, tool as u8);
            w_.u32(4, val);
            w_.i32(8, scale_step);
        }))),
    );
    _rt::answer(r)
}

pub fn marker_text(
    side_: &_rt::Side,
    text: &str,
) -> Result<Result<(), super::types::ErrorCode>, _rt::Errno> {
    let (r, data) = side_.call(
        HANDLE,
        MARKER_TEXT,
        _rt::Packet::Args(Some((8, &|w_: &mut _rt::Lower| {
            w_.str(0, text);
        }))),
    );
    _rt::answer(r).and_then(|_| {
        let r_ = _rt::Lift::new(data.ok_or(_rt::Errno::EBADMSG)?);
        r_.case(0, 2).and_then(|d| {
            if d == 0 {
                Ok(Ok(()))
            } else {
                r_.case(1, 6)
                    .and_then(|v| super::types::ErrorCode::of(v).ok_or(_rt::Errno::EBADMSG))
                    .map(Err)
            }
        })
    })
}

pub fn marker_cancel(side_: &_rt::Side) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(HANDLE, MARKER_CANCEL, _rt::Packet::Args(None));
    _rt::answer(r)
}

pub fn marker_commit(side_: &_rt::Side) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(HANDLE, MARKER_COMMIT, _rt::Packet::Args(None));
    _rt::answer(r)
}

mod _rt {
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Packet, Side, answer};
}
