// pxl-core/abi/sensor.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/sensor` (frame.json5 `NAMED`).
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

pub trait Guest {
    fn pos(
        &self,
        lon: f64,
        lat: f64,
        alt: Option<f64>,
        accuracy: Option<f64>,
        course: Option<f64>,
    ) -> Result<(), _rt::Errno>;
    fn orient(
        &self,
        webkit_compass_heading: Option<f64>,
        alpha: Option<f64>,
    ) -> Result<(), _rt::Errno>;
    fn err(
        &self,
        sensor: super::types::Sensor,
        code: Option<GeolocationErrorCode>,
        watching: bool,
    ) -> Result<(), _rt::Errno>;
    fn permission(
        &self,
        sensor: super::types::Sensor,
        answer: PermissionAnswer,
    ) -> Result<(), _rt::Errno>;
    fn platform(
        &self,
        geolocation: bool,
        orient: bool,
        orient_permission: bool,
        orient_silence_is_loss: bool,
    ) -> Result<(), _rt::Errno>;
    fn locate(&self) -> Result<(), _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Guest>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                POS => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let lon = r_.f64(0)?;
                    let lat = r_.f64(8)?;
                    let alt = r_.case(16, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            r_.f64(24).map(Some)
                        }
                    })?;
                    let accuracy = r_.case(32, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            r_.f64(40).map(Some)
                        }
                    })?;
                    let course = r_.case(48, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            r_.f64(56).map(Some)
                        }
                    })?;
                    match self.0.pos(lon, lat, alt, accuracy, course) {
                        Ok(()) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                ORIENT => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let webkit_compass_heading = r_.case(0, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            r_.f64(8).map(Some)
                        }
                    })?;
                    let alpha = r_.case(16, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            r_.f64(24).map(Some)
                        }
                    })?;
                    match self.0.orient(webkit_compass_heading, alpha) {
                        Ok(()) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                ERR => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let sensor = r_
                        .case(0, 2)
                        .and_then(|v| super::types::Sensor::of(v).ok_or(_rt::Errno::EBADMSG))?;
                    let code = r_.case(1, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            r_.case(2, 4)
                                .and_then(|v| {
                                    GeolocationErrorCode::of(v).ok_or(_rt::Errno::EBADMSG)
                                })
                                .map(Some)
                        }
                    })?;
                    let watching = r_.bool(3)?;
                    match self.0.err(sensor, code, watching) {
                        Ok(()) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                PERMISSION => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let sensor = r_
                        .case(0, 2)
                        .and_then(|v| super::types::Sensor::of(v).ok_or(_rt::Errno::EBADMSG))?;
                    let answer = r_
                        .case(1, 4)
                        .and_then(|v| PermissionAnswer::of(v).ok_or(_rt::Errno::EBADMSG))?;
                    match self.0.permission(sensor, answer) {
                        Ok(()) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                PLATFORM => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let geolocation = r_.bool(0)?;
                    let orient = r_.bool(1)?;
                    let orient_permission = r_.bool(2)?;
                    let orient_silence_is_loss = r_.bool(3)?;
                    match self.0.platform(
                        geolocation,
                        orient,
                        orient_permission,
                        orient_silence_is_loss,
                    ) {
                        Ok(()) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                LOCATE => match self.0.locate() {
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
    pub(super) use xpute_link::{Handle, Lift, Method, Object, Reply, Tag};
}
