// pxl-core/abi/sensor.rs
//
// GENERATED from spec/client/core/wit (xpute-gen spec/wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use std::cell::Cell;
use std::rc::Rc;
use xpute_core::link::{self, Handle, Method, Object, Reply, Return, Tag};
use xpute_core::wait::wait;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

/// The singleton's URI.
pub const URI: [&str; 2] = ["pxl:core", "sensor"];

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

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GeolocationErrorCode {
    #[default]
    Other = 0,
    PermissionDenied = 1,
    PositionUnavailable = 2,
    Timeout = 3,
}

impl GeolocationErrorCode {
    pub const ALL: [GeolocationErrorCode; 4] = [
        GeolocationErrorCode::Other,
        GeolocationErrorCode::PermissionDenied,
        GeolocationErrorCode::PositionUnavailable,
        GeolocationErrorCode::Timeout,
    ];

    pub fn of(v: u32) -> Option<GeolocationErrorCode> {
        GeolocationErrorCode::ALL.get(v as usize).copied()
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PermissionAnswer {
    #[default]
    Asking = 0,
    Granted = 1,
    Denied = 2,
    Failed = 3,
}

impl PermissionAnswer {
    pub const ALL: [PermissionAnswer; 4] = [
        PermissionAnswer::Asking,
        PermissionAnswer::Granted,
        PermissionAnswer::Denied,
        PermissionAnswer::Failed,
    ];

    pub fn of(v: u32) -> Option<PermissionAnswer> {
        PermissionAnswer::ALL.get(v as usize).copied()
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
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                POS => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let lon = b.at(0).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let lat = b.at(1).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let alt = b.at(2).and_then(|c| {
                        c.opt()
                            .map(|c| c.get_number().map(|v| v as f64))
                            .transpose()
                    })?;
                    let accuracy = b.at(3).and_then(|c| {
                        c.opt()
                            .map(|c| c.get_number().map(|v| v as f64))
                            .transpose()
                    })?;
                    let course = b.at(4).and_then(|c| {
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
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let webkit_compass_heading = b.at(0).and_then(|c| {
                        c.opt()
                            .map(|c| c.get_number().map(|v| v as f64))
                            .transpose()
                    })?;
                    let alpha = b.at(1).and_then(|c| {
                        c.opt()
                            .map(|c| c.get_number().map(|v| v as f64))
                            .transpose()
                    })?;
                    let beta = b.at(2).and_then(|c| {
                        c.opt()
                            .map(|c| c.get_number().map(|v| v as f64))
                            .transpose()
                    })?;
                    let gamma = b.at(3).and_then(|c| {
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
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let sensor = b.at(0).and_then(|c| {
                        c.get_number().and_then(|v| {
                            super::types::Sensor::of(v as u32)
                                .ok_or(MarshalError::new(Errno::EBADMSG))
                        })
                    })?;
                    let code = b.at(1).and_then(|c| {
                        c.opt()
                            .map(|c| {
                                c.get_number().and_then(|v| {
                                    GeolocationErrorCode::of(v as u32)
                                        .ok_or(MarshalError::new(Errno::EBADMSG))
                                })
                            })
                            .transpose()
                    })?;
                    let watching = b.at(2).and_then(|c| c.get_bool())?;
                    match self.0.err(sensor, code, watching) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                PERMISSION => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let sensor = b.at(0).and_then(|c| {
                        c.get_number().and_then(|v| {
                            super::types::Sensor::of(v as u32)
                                .ok_or(MarshalError::new(Errno::EBADMSG))
                        })
                    })?;
                    let answer = b.at(1).and_then(|c| {
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
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let geolocation = b.at(0).and_then(|c| c.get_bool())?;
                    let orient = b.at(1).and_then(|c| c.get_bool())?;
                    let orient_permission = b.at(2).and_then(|c| c.get_bool())?;
                    let orient_silence_is_loss = b.at(3).and_then(|c| c.get_bool())?;
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

/// Mounts `imp` at the URI, one object for every open.
pub fn mount(imp: Rc<dyn Sensor>) {
    let o: Rc<dyn Object> = Rc::new(SensorObject(imp));
    link::mount(link::uri(&URI), move |_, _| Ok(o.clone()));
}

thread_local! {
    static HANDLE: Cell<Option<Handle>> = const { Cell::new(None) };
}

/// Opens the singleton on the other side, once, at boot.
pub fn open() -> Result<(), Errno> {
    HANDLE.set(Some(link::open(&link::uri(&URI))?));
    Ok(())
}

/// The singleton's handle, which `open` opened.
pub fn handle() -> Handle {
    HANDLE.get().or_bug(Errno::EBADF)
}

pub fn pos(
    lon: f64,
    lat: f64,
    alt: Option<f64>,
    accuracy: Option<f64>,
    course: Option<f64>,
) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        POS,
        Some((5, &|w: &mut PacketWriter| {
            w.f64(Some(lon as f64));
            w.f64(Some(lat as f64));
            match &alt {
                Some(v) => {
                    w.f64(Some((*v) as f64));
                }
                None => {
                    w.nil();
                }
            }
            match &accuracy {
                Some(v) => {
                    w.f64(Some((*v) as f64));
                }
                None => {
                    w.nil();
                }
            }
            match &course {
                Some(v) => {
                    w.f64(Some((*v) as f64));
                }
                None => {
                    w.nil();
                }
            }
        })),
    );
    link::answer(r).map(|_| ())
}

pub fn orient(
    webkit_compass_heading: Option<f64>,
    alpha: Option<f64>,
    beta: Option<f64>,
    gamma: Option<f64>,
) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        ORIENT,
        Some((4, &|w: &mut PacketWriter| {
            match &webkit_compass_heading {
                Some(v) => {
                    w.f64(Some((*v) as f64));
                }
                None => {
                    w.nil();
                }
            }
            match &alpha {
                Some(v) => {
                    w.f64(Some((*v) as f64));
                }
                None => {
                    w.nil();
                }
            }
            match &beta {
                Some(v) => {
                    w.f64(Some((*v) as f64));
                }
                None => {
                    w.nil();
                }
            }
            match &gamma {
                Some(v) => {
                    w.f64(Some((*v) as f64));
                }
                None => {
                    w.nil();
                }
            }
        })),
    );
    link::answer(r).map(|_| ())
}

pub fn err(
    sensor: super::types::Sensor,
    code: Option<GeolocationErrorCode>,
    watching: bool,
) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        ERR,
        Some((3, &|w: &mut PacketWriter| {
            w.u32(Some(sensor as u32));
            match &code {
                Some(v) => {
                    w.u32(Some((*v) as u32));
                }
                None => {
                    w.nil();
                }
            }
            w.bool(Some(watching));
        })),
    );
    link::answer(r).map(|_| ())
}

pub fn permission(sensor: super::types::Sensor, answer: PermissionAnswer) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        PERMISSION,
        Some((2, &|w: &mut PacketWriter| {
            w.u32(Some(sensor as u32));
            w.u32(Some(answer as u32));
        })),
    );
    link::answer(r).map(|_| ())
}

pub fn platform(
    geolocation: bool,
    orient: bool,
    orient_permission: bool,
    orient_silence_is_loss: bool,
) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        PLATFORM,
        Some((4, &|w: &mut PacketWriter| {
            w.bool(Some(geolocation));
            w.bool(Some(orient));
            w.bool(Some(orient_permission));
            w.bool(Some(orient_silence_is_loss));
        })),
    );
    link::answer(r).map(|_| ())
}

pub fn locate() -> Result<(), Errno> {
    let (r, data) = link::call(handle(), LOCATE, None);
    link::answer(r).map(|_| ())
}
