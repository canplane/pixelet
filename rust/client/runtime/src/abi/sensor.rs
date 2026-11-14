// pxl-runtime/abi/sensor.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

/// The interface's handle, named by `pxl:core/sensor` (frame.json `NAMED`).
pub const HANDLE: _rt::Handle = 0x5534df;

/// `pos`
pub const POS: _rt::Method = 0;
/// `orient`
pub const ORIENT: _rt::Method = 1;
/// `err`
pub const ERR: _rt::Method = 2;
/// `permission`
pub const PERMISSION: _rt::Method = 3;
/// `platform`
pub const PLATFORM: _rt::Method = 4;
/// `locate`
pub const LOCATE: _rt::Method = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum GeolocationErrorCode {
    Other = 0,
    PermissionDenied = 1,
    PositionUnavailable = 2,
    Timeout = 3,
}

impl GeolocationErrorCode {
    /// Every member, in its source's own order.
    pub const ALL: [GeolocationErrorCode; 4] = [
        GeolocationErrorCode::Other,
        GeolocationErrorCode::PermissionDenied,
        GeolocationErrorCode::PositionUnavailable,
        GeolocationErrorCode::Timeout,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<GeolocationErrorCode> {
        match val {
            0 => Some(GeolocationErrorCode::Other),
            1 => Some(GeolocationErrorCode::PermissionDenied),
            2 => Some(GeolocationErrorCode::PositionUnavailable),
            3 => Some(GeolocationErrorCode::Timeout),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            GeolocationErrorCode::Other => "Other",
            GeolocationErrorCode::PermissionDenied => "PermissionDenied",
            GeolocationErrorCode::PositionUnavailable => "PositionUnavailable",
            GeolocationErrorCode::Timeout => "Timeout",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum PermissionAnswer {
    Asking = 0,
    Granted = 1,
    Denied = 2,
    Failed = 3,
}

impl PermissionAnswer {
    /// Every member, in its source's own order.
    pub const ALL: [PermissionAnswer; 4] = [
        PermissionAnswer::Asking,
        PermissionAnswer::Granted,
        PermissionAnswer::Denied,
        PermissionAnswer::Failed,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<PermissionAnswer> {
        match val {
            0 => Some(PermissionAnswer::Asking),
            1 => Some(PermissionAnswer::Granted),
            2 => Some(PermissionAnswer::Denied),
            3 => Some(PermissionAnswer::Failed),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            PermissionAnswer::Asking => "Asking",
            PermissionAnswer::Granted => "Granted",
            PermissionAnswer::Denied => "Denied",
            PermissionAnswer::Failed => "Failed",
        }
    }
}

pub fn pos(
    lon: f64,
    lat: f64,
    alt: Option<f64>,
    accuracy: Option<f64>,
    course: Option<f64>,
) -> Result<(), _rt::Errno> {
    let (r, data) = _rt::link::call(
        HANDLE,
        POS,
        Some((64, &|w_: &mut _rt::Lower| {
            w_.f64(0, *&lon);
            w_.f64(8, *&lat);
            if let Some(v) = &alt {
                w_.u8(16, 1);
                w_.f64(16 + 8, *v);
            }
            if let Some(v) = &accuracy {
                w_.u8(32, 1);
                w_.f64(32 + 8, *v);
            }
            if let Some(v) = &course {
                w_.u8(48, 1);
                w_.f64(48 + 8, *v);
            }
        })),
    );
    _rt::answer(r)
}

pub fn orient(
    webkit_compass_heading: Option<f64>,
    alpha: Option<f64>,
    beta: Option<f64>,
    gamma: Option<f64>,
) -> Result<(), _rt::Errno> {
    let (r, data) = _rt::link::call(
        HANDLE,
        ORIENT,
        Some((64, &|w_: &mut _rt::Lower| {
            if let Some(v) = &webkit_compass_heading {
                w_.u8(0, 1);
                w_.f64(0 + 8, *v);
            }
            if let Some(v) = &alpha {
                w_.u8(16, 1);
                w_.f64(16 + 8, *v);
            }
            if let Some(v) = &beta {
                w_.u8(32, 1);
                w_.f64(32 + 8, *v);
            }
            if let Some(v) = &gamma {
                w_.u8(48, 1);
                w_.f64(48 + 8, *v);
            }
        })),
    );
    _rt::answer(r)
}

pub fn err(
    sensor: super::types::Sensor,
    code: Option<GeolocationErrorCode>,
    watching: bool,
) -> Result<(), _rt::Errno> {
    let (r, data) = _rt::link::call(
        HANDLE,
        ERR,
        Some((4, &|w_: &mut _rt::Lower| {
            w_.u8(0, *&sensor as u8);
            if let Some(v) = &code {
                w_.u8(1, 1);
                w_.u8(1 + 1, *v as u8);
            }
            w_.bool(3, *&watching);
        })),
    );
    _rt::answer(r)
}

pub fn permission(
    sensor: super::types::Sensor,
    answer: PermissionAnswer,
) -> Result<(), _rt::Errno> {
    let (r, data) = _rt::link::call(
        HANDLE,
        PERMISSION,
        Some((2, &|w_: &mut _rt::Lower| {
            w_.u8(0, *&sensor as u8);
            w_.u8(1, *&answer as u8);
        })),
    );
    _rt::answer(r)
}

pub fn platform(
    geolocation: bool,
    orient: bool,
    orient_permission: bool,
    orient_silence_is_loss: bool,
) -> Result<(), _rt::Errno> {
    let (r, data) = _rt::link::call(
        HANDLE,
        PLATFORM,
        Some((4, &|w_: &mut _rt::Lower| {
            w_.bool(0, *&geolocation);
            w_.bool(1, *&orient);
            w_.bool(2, *&orient_permission);
            w_.bool(3, *&orient_silence_is_loss);
        })),
    );
    _rt::answer(r)
}

pub fn locate() -> Result<(), _rt::Errno> {
    let (r, data) = _rt::link::call(HANDLE, LOCATE, None);
    _rt::answer(r)
}

mod _rt {
    pub use xpute_kit::status::errno::Errno;
    pub use xpute_link::{answer, Handle, Lower, Method};
    pub use xpute_native_runtime::link;
}
