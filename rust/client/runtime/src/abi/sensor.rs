// pxl-runtime/abi/sensor.rs
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
    pub alt_ellipsoid: Option<f64>,
}

impl Position {
    pub const SZ: usize = 32;
    pub const ALIGN: usize = 8;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.f64(at, self.lon);
        w.f64(at + 8, self.lat);
        if let Some(v) = &self.alt_ellipsoid {
            w.u8(at + 16, 1);
            w.f64(at + 16 + 8, *v);
        }
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<Position, _rt::Errno> {
        Ok(Position {
            lon: r.f64(at)?,
            lat: r.f64(at + 8)?,
            alt_ellipsoid: r.case(at + 16, 2).and_then(|d| {
                if d == 0 {
                    Ok(None)
                } else {
                    r.f64(at + 16 + 8).map(Some)
                }
            })?,
        })
    }
}

pub fn pos(side_: &_rt::Side, at: Option<&Position>) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(
        HANDLE,
        POS,
        _rt::Packet::Args(Some((40, &|w_: &mut _rt::Lower| {
            if let Some(v) = &at {
                w_.u8(0, 1);
                (v).store(w_, 8);
            }
        }))),
    );
    _rt::answer(r)
}

pub fn heading(side_: &_rt::Side, deg: Option<f64>) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(
        HANDLE,
        HEADING,
        _rt::Packet::Args(Some((16, &|w_: &mut _rt::Lower| {
            if let Some(v) = &deg {
                w_.u8(0, 1);
                w_.f64(8, *v);
            }
        }))),
    );
    _rt::answer(r)
}

pub fn location(side_: &_rt::Side, status: LocationStatus) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(
        HANDLE,
        LOCATION,
        _rt::Packet::Args(Some((1, &|w_: &mut _rt::Lower| {
            w_.u8(0, status as u8);
        }))),
    );
    _rt::answer(r)
}

mod _rt {
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Packet, Side, answer};
}
