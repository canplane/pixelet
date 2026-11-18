// xpute-host/abi/log.rs
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

pub trait Host {
    fn log(&self, level: Level, context: String, msg: String) -> Result<(), _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Host>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: _rt::Lift, _: &mut _rt::Lower) -> i32 {
        let run = || -> Result<i32, _rt::Errno> {
            Ok(match m_ {
                LOG => {
                    let level = args_
                        .case(0, 6)
                        .and_then(|v| Level::of(v).ok_or(_rt::Errno::EBADMSG))?;
                    let context = args_.str(4).map(str::to_owned)?;
                    let msg = args_.str(12).map(str::to_owned)?;
                    match self.0.log(level, context, msg) {
                        Ok(()) => 0,
                        Err(e) => e as i32,
                    }
                }
                _ => _rt::Errno::ENOSYS as i32,
            })
        };
        run().unwrap_or_else(|e| e as i32)
    }
}

/// Mounts `implementation` under the interface's handle on `side_`.
pub fn mount(side_: &'static _rt::Side, implementation: _rt::Rc<dyn Host>) {
    side_.mount_named(HANDLE, _rt::Rc::new(_Object(implementation)));
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Side, Tag};
}

/// The interfaces the world imports, each `(name, handle)`, which are
/// mounted before the guest boots: one with functions of its own under its
/// handle, one of resources alone by an opener at a prefix.
pub const IMPORTS: &[(&str, xpute_link::Handle)] = &[("xpute:log/log", 0x73da26)];

/// The methods of the interfaces the world imports that answer at once,
/// each `(handle, method)`, a function without WIT's `async`, which a
/// runtime refuses to carry to a UI on another thread.
pub const CALLS: &[(xpute_link::Handle, xpute_link::Method)] = &[(0x73da26, 0)];
