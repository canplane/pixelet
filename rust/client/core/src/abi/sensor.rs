// pxl-core/abi/sensor.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/sensor` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x5534df;

/// `pos`
pub const POS: _rt::Method = 0;
/// `heading`
pub const HEADING: _rt::Method = 1;
/// `location`
pub const LOCATION: _rt::Method = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum LocationStatus {
    Idle = 0,
    Loading = 1,
    Ready = 2,
    Error = 3,
}

impl LocationStatus {
    /// Every member, in its source's own order.
    pub const ALL: [LocationStatus; 4] = [
        LocationStatus::Idle,
        LocationStatus::Loading,
        LocationStatus::Ready,
        LocationStatus::Error,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<LocationStatus> {
        match val {
            0 => Some(LocationStatus::Idle),
            1 => Some(LocationStatus::Loading),
            2 => Some(LocationStatus::Ready),
            3 => Some(LocationStatus::Error),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            LocationStatus::Idle => "Idle",
            LocationStatus::Loading => "Loading",
            LocationStatus::Ready => "Ready",
            LocationStatus::Error => "Error",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Position {
    pub lon: f64,
    pub lat: f64,
    pub alt: Option<f64>,
}

impl Position {
    pub const SZ: usize = 32;
    pub const ALIGN: usize = 8;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.f64(at, self.lon);
        w.f64(at + 8, self.lat);
        if let Some(v) = &self.alt {
            w.u8(at + 16, 1);
            w.f64(at + 16 + 8, *v);
        }
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<Position, _rt::Errno> {
        Ok(Position {
            lon: r.f64(at)?,
            lat: r.f64(at + 8)?,
            alt: r.case(at + 16, 2).and_then(|d| {
                if d == 0 {
                    Ok(None)
                } else {
                    r.f64(at + 16 + 8).map(Some)
                }
            })?,
        })
    }
}

pub trait Guest {
    fn pos(&self, at: Option<Position>) -> Result<(), _rt::Errno>;
    fn heading(&self, deg: Option<f64>) -> Result<(), _rt::Errno>;
    fn location(&self, status: LocationStatus) -> Result<(), _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Guest>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                POS => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let at = r_.case(0, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            Position::load(r_, 8).map(Some)
                        }
                    })?;
                    match self.0.pos(at) {
                        Ok(()) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                HEADING => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let deg = r_.case(0, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            r_.f64(8).map(Some)
                        }
                    })?;
                    match self.0.heading(deg) {
                        Ok(()) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                LOCATION => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let status = r_
                        .case(0, 4)
                        .and_then(|v| LocationStatus::of(v).ok_or(_rt::Errno::EBADMSG))?;
                    match self.0.location(status) {
                        Ok(()) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
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
