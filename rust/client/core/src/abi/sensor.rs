// pxl-core/abi/sensor.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use std::rc::Rc;
use xpute_guest::link;
use xpute_guest::wait;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `pxl:core/sensor` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x5534df;

/// `pos`
pub const POS: Method = 0;
/// `orient`
pub const ORIENT: Method = 1;
/// `err`
pub const ERR: Method = 2;
/// `permission`
pub const PERMISSION: Method = 3;
/// `platform`
pub const PLATFORM: Method = 4;
/// `locate`
pub const LOCATE: Method = 5;

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

pub trait Sensor {
    fn pos(
        &self,
        lon: f64,
        lat: f64,
        alt: Option<f64>,
        accuracy: Option<f64>,
        course: Option<f64>,
    ) -> Result<(), Errno>;
    fn orient(
        &self,
        webkit_compass_heading: Option<f64>,
        alpha: Option<f64>,
        beta: Option<f64>,
        gamma: Option<f64>,
    ) -> Result<(), Errno>;
    fn err(
        &self,
        sensor: super::types::Sensor,
        code: Option<GeolocationErrorCode>,
        watching: bool,
    ) -> Result<(), Errno>;
    fn permission(
        &self,
        sensor: super::types::Sensor,
        answer: PermissionAnswer,
    ) -> Result<(), Errno>;
    fn platform(
        &self,
        geolocation: bool,
        orient: bool,
        orient_permission: bool,
        orient_silence_is_loss: bool,
    ) -> Result<(), Errno>;
    fn locate(&self) -> Result<(), Errno>;
}

struct SensorObject(Rc<dyn Sensor>);

impl Object for SensorObject {
    fn call(&self, m_: Method, tag_: Tag, args_: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m_ {
                POS => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let lon = b_.at(0).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let lat = b_.at(1).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let alt = b_.at(2).and_then(|c| {
                        c.opt()
                            .map(|c| c.get_number().map(|v| v as f64))
                            .transpose()
                    })?;
                    let accuracy = b_.at(3).and_then(|c| {
                        c.opt()
                            .map(|c| c.get_number().map(|v| v as f64))
                            .transpose()
                    })?;
                    let course = b_.at(4).and_then(|c| {
                        c.opt()
                            .map(|c| c.get_number().map(|v| v as f64))
                            .transpose()
                    })?;
                    match self.0.pos(lon, lat, alt, accuracy, course) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                ORIENT => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let webkit_compass_heading = b_.at(0).and_then(|c| {
                        c.opt()
                            .map(|c| c.get_number().map(|v| v as f64))
                            .transpose()
                    })?;
                    let alpha = b_.at(1).and_then(|c| {
                        c.opt()
                            .map(|c| c.get_number().map(|v| v as f64))
                            .transpose()
                    })?;
                    let beta = b_.at(2).and_then(|c| {
                        c.opt()
                            .map(|c| c.get_number().map(|v| v as f64))
                            .transpose()
                    })?;
                    let gamma = b_.at(3).and_then(|c| {
                        c.opt()
                            .map(|c| c.get_number().map(|v| v as f64))
                            .transpose()
                    })?;
                    match self.0.orient(webkit_compass_heading, alpha, beta, gamma) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                ERR => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let sensor = b_.at(0).and_then(|c| {
                        c.get_number().and_then(|v| {
                            super::types::Sensor::of(v as u32)
                                .ok_or(MarshalError::new(Errno::EBADMSG))
                        })
                    })?;
                    let code = b_.at(1).and_then(|c| {
                        c.opt()
                            .map(|c| {
                                c.get_number().and_then(|v| {
                                    GeolocationErrorCode::of(v as u32)
                                        .ok_or(MarshalError::new(Errno::EBADMSG))
                                })
                            })
                            .transpose()
                    })?;
                    let watching = b_.at(2).and_then(|c| c.get_bool())?;
                    match self.0.err(sensor, code, watching) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                PERMISSION => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let sensor = b_.at(0).and_then(|c| {
                        c.get_number().and_then(|v| {
                            super::types::Sensor::of(v as u32)
                                .ok_or(MarshalError::new(Errno::EBADMSG))
                        })
                    })?;
                    let answer = b_.at(1).and_then(|c| {
                        c.get_number().and_then(|v| {
                            PermissionAnswer::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                        })
                    })?;
                    match self.0.permission(sensor, answer) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                PLATFORM => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let geolocation = b_.at(0).and_then(|c| c.get_bool())?;
                    let orient = b_.at(1).and_then(|c| c.get_bool())?;
                    let orient_permission = b_.at(2).and_then(|c| c.get_bool())?;
                    let orient_silence_is_loss = b_.at(3).and_then(|c| c.get_bool())?;
                    match self.0.platform(
                        geolocation,
                        orient,
                        orient_permission,
                        orient_silence_is_loss,
                    ) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                LOCATE => match self.0.locate() {
                    Ok(v) => Reply::from(0),
                    Err(e) => Reply::from(e as i32),
                },
                _ => Reply::from(Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
    }
}

/// Serves `imp` under the interface's handle.
pub fn mount(imp: Rc<dyn Sensor>) {
    link::mount_named(HANDLE, Rc::new(SensorObject(imp)));
}
