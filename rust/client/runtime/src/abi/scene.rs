// pxl-runtime/abi/scene.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/scene` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x7c289a;

/// `draw`
pub const DRAW: _rt::Method = 0;
/// `chat`
pub const CHAT: _rt::Method = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum SceneContent {
    World = 0,
    Character = 1,
}

impl SceneContent {
    /// Every member, in its source's own order.
    pub const ALL: [SceneContent; 2] = [SceneContent::World, SceneContent::Character];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<SceneContent> {
        match val {
            0 => Some(SceneContent::World),
            1 => Some(SceneContent::Character),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            SceneContent::World => "World",
            SceneContent::Character => "Character",
        }
    }
}

/// The size of the fixed part of `draw`'s args.
pub const DRAW_ARGS_SZ: usize = 16;

pub fn draw(
    side_: &_rt::Side,
    surface: u32,
    content: SceneContent,
    group: Option<u8>,
    bg: Option<u32>,
) -> Result<Result<(), super::types::ErrorCode>, _rt::XputeError> {
    let (r, data) = side_.call(
        HANDLE,
        DRAW,
        _rt::Packet::Args(Some((16, &|w_: &mut _rt::Lower| {
            w_.u32(0, surface);
            w_.u8(4, content as u8);
            if let Some(v) = &group {
                w_.u8(5, 1);
                w_.u8(6, *v);
            }
            if let Some(v) = &bg {
                w_.u8(8, 1);
                w_.u32(12, *v);
            }
        }))),
    );
    _rt::answer(r, data).and_then(|_| {
        let r_ = _rt::Lift::new(data.ok_or(_rt::Errno::EBADMSG)?);
        Ok(r_.case(0, 2).and_then(|d| {
            if d == 0 {
                Ok(Ok(()))
            } else {
                r_.case(1, 6)
                    .and_then(|v| super::types::ErrorCode::of(v).ok_or(_rt::Errno::EBADMSG))
                    .map(Err)
            }
        })?)
    })
}

/// The size of the fixed part of `chat`'s args.
pub const CHAT_ARGS_SZ: usize = 8;

pub fn chat(
    side_: &_rt::Side,
    text: &str,
) -> Result<Result<(), super::types::ErrorCode>, _rt::XputeError> {
    let (r, data) = side_.call(
        HANDLE,
        CHAT,
        _rt::Packet::Args(Some((8, &|w_: &mut _rt::Lower| {
            w_.str(0, text);
        }))),
    );
    _rt::answer(r, data).and_then(|_| {
        let r_ = _rt::Lift::new(data.ok_or(_rt::Errno::EBADMSG)?);
        Ok(r_.case(0, 2).and_then(|d| {
            if d == 0 {
                Ok(Ok(()))
            } else {
                r_.case(1, 6)
                    .and_then(|v| super::types::ErrorCode::of(v).ok_or(_rt::Errno::EBADMSG))
                    .map(Err)
            }
        })?)
    })
}

mod _rt {
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_kit::status::error::XputeError;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Packet, Side, answer};
}
