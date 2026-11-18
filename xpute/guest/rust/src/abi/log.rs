// xpute-guest/abi/log.rs
//
// GENERATED from wit/log (xpute-link-gen wit.rs)

/// The interface's handle, named by `xpute:log/log` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x73da26;

/// `log`
pub const LOG: _rt::Method = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Level {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
    Critical = 5,
}

impl Level {
    /// Every member, in its source's own order.
    pub const ALL: [Level; 6] = [
        Level::Trace,
        Level::Debug,
        Level::Info,
        Level::Warn,
        Level::Error,
        Level::Critical,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<Level> {
        match val {
            0 => Some(Level::Trace),
            1 => Some(Level::Debug),
            2 => Some(Level::Info),
            3 => Some(Level::Warn),
            4 => Some(Level::Error),
            5 => Some(Level::Critical),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Level::Trace => "Trace",
            Level::Debug => "Debug",
            Level::Info => "Info",
            Level::Warn => "Warn",
            Level::Error => "Error",
            Level::Critical => "Critical",
        }
    }
}

pub fn log(level: Level, context: &str, msg: &str) -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(
        HANDLE,
        LOG,
        Some((20, &|w_: &mut _rt::Lower| {
            w_.u8(0, level as u8);
            w_.str(4, context);
            w_.str(12, msg);
        })),
    );
    _rt::answer(r)
}

mod _rt {
    pub(super) use crate::link;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lower, Method, answer};
}
