// pxl-core/abi/overlay.rs
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
pub const URI: [&str; 2] = ["pxl:core", "overlay"];

/// `show`
pub const SHOW: Method = 0;
/// `view`
pub const VIEW: Method = 1;

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroundStatus {
    #[default]
    Idle = 0,
    Loading = 1,
    Success = 2,
}

impl GroundStatus {
    pub const ALL: [GroundStatus; 3] = [
        GroundStatus::Idle,
        GroundStatus::Loading,
        GroundStatus::Success,
    ];

    pub fn of(v: u32) -> Option<GroundStatus> {
        GroundStatus::ALL.get(v as usize).copied()
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SensorStatus {
    #[default]
    Idle = 0,
    Loading = 1,
    Ready = 2,
    Error = 3,
    Unavailable = 4,
}

impl SensorStatus {
    pub const ALL: [SensorStatus; 5] = [
        SensorStatus::Idle,
        SensorStatus::Loading,
        SensorStatus::Ready,
        SensorStatus::Error,
        SensorStatus::Unavailable,
    ];

    pub fn of(v: u32) -> Option<SensorStatus> {
        SensorStatus::ALL.get(v as usize).copied()
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Message {
    #[default]
    GeolocationUnsupported = 0,
    GeolocationPermissionDenied = 1,
    GeolocationPositionUnavailable = 2,
    GeolocationTimeout = 3,
    GeolocationGenericError = 4,
    WorldReadFailing = 5,
    WorldUnavailable = 6,
}

impl Message {
    pub const ALL: [Message; 7] = [
        Message::GeolocationUnsupported,
        Message::GeolocationPermissionDenied,
        Message::GeolocationPositionUnavailable,
        Message::GeolocationTimeout,
        Message::GeolocationGenericError,
        Message::WorldReadFailing,
        Message::WorldUnavailable,
    ];

    pub fn of(v: u32) -> Option<Message> {
        Message::ALL.get(v as usize).copied()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LonLat {
    pub lon: f64,
    pub lat: f64,
}

impl LonLat {
    pub fn write(&self, w: &mut PacketWriter) {
        w.branch(2, |w| {
            w.f64(Some(self.lon as f64));
            w.f64(Some(self.lat as f64));
        });
    }

    pub fn read(c: Cursor<'_>) -> Result<LonLat, MarshalError> {
        let b = c.as_branch()?;
        Ok(LonLat {
            lon: b.at(0).and_then(|c| c.get_number().map(|v| v as f64))?,
            lat: b.at(1).and_then(|c| c.get_number().map(|v| v as f64))?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PivotState {
    pub status: GroundStatus,
    pub is_off_center: bool,
    pub lon_lat: Option<LonLat>,
    pub outside: bool,
}

impl PivotState {
    pub fn write(&self, w: &mut PacketWriter) {
        w.branch(4, |w| {
            w.u32(Some(self.status as u32));
            w.bool(Some(self.is_off_center));
            match &self.lon_lat {
                Some(v) => {
                    (*v).write(w);
                }
                None => {
                    w.nil();
                }
            }
            w.bool(Some(self.outside));
        });
    }

    pub fn read(c: Cursor<'_>) -> Result<PivotState, MarshalError> {
        let b = c.as_branch()?;
        Ok(PivotState {
            status: b.at(0).and_then(|c| {
                c.get_number().and_then(|v| {
                    GroundStatus::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                })
            })?,
            is_off_center: b.at(1).and_then(|c| c.get_bool())?,
            lon_lat: b
                .at(2)
                .and_then(|c| c.opt().map(|c| LonLat::read(c)).transpose())?,
            outside: b.at(3).and_then(|c| c.get_bool())?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocationState {
    pub status: SensorStatus,
    pub has_location: bool,
    pub watching: bool,
    pub err: Option<Message>,
}

impl LocationState {
    pub fn write(&self, w: &mut PacketWriter) {
        w.branch(4, |w| {
            w.u32(Some(self.status as u32));
            w.bool(Some(self.has_location));
            w.bool(Some(self.watching));
            match &self.err {
                Some(v) => {
                    w.u32(Some((*v) as u32));
                }
                None => {
                    w.nil();
                }
            }
        });
    }

    pub fn read(c: Cursor<'_>) -> Result<LocationState, MarshalError> {
        let b = c.as_branch()?;
        Ok(LocationState {
            status: b.at(0).and_then(|c| {
                c.get_number().and_then(|v| {
                    SensorStatus::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                })
            })?,
            has_location: b.at(1).and_then(|c| c.get_bool())?,
            watching: b.at(2).and_then(|c| c.get_bool())?,
            err: b.at(3).and_then(|c| {
                c.opt()
                    .map(|c| {
                        c.get_number().and_then(|v| {
                            Message::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                        })
                    })
                    .transpose()
            })?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompanionState {
    pub person_ready: bool,
    pub pet_ready: bool,
}

impl CompanionState {
    pub fn write(&self, w: &mut PacketWriter) {
        w.branch(2, |w| {
            w.bool(Some(self.person_ready));
            w.bool(Some(self.pet_ready));
        });
    }

    pub fn read(c: Cursor<'_>) -> Result<CompanionState, MarshalError> {
        let b = c.as_branch()?;
        Ok(CompanionState {
            person_ready: b.at(0).and_then(|c| c.get_bool())?,
            pet_ready: b.at(1).and_then(|c| c.get_bool())?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OrientationState {
    pub status: SensorStatus,
}

impl OrientationState {
    pub fn write(&self, w: &mut PacketWriter) {
        w.branch(1, |w| {
            w.u32(Some(self.status as u32));
        });
    }

    pub fn read(c: Cursor<'_>) -> Result<OrientationState, MarshalError> {
        let b = c.as_branch()?;
        Ok(OrientationState {
            status: b.at(0).and_then(|c| {
                c.get_number().and_then(|v| {
                    SensorStatus::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                })
            })?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DivisionPart {
    pub subtype: u8,
    pub name: String,
    pub shown: bool,
    pub area: bool,
}

impl DivisionPart {
    pub fn write(&self, w: &mut PacketWriter) {
        w.branch(4, |w| {
            w.u8(Some(self.subtype));
            w.str(Some(&*self.name));
            w.bool(Some(self.shown));
            w.bool(Some(self.area));
        });
    }

    pub fn read(c: Cursor<'_>) -> Result<DivisionPart, MarshalError> {
        let b = c.as_branch()?;
        Ok(DivisionPart {
            subtype: b.at(0).and_then(|c| c.get_number().map(|v| v as u8))?,
            name: b.at(1).and_then(|c| c.get_str_ref().map(str::to_owned))?,
            shown: b.at(2).and_then(|c| c.get_bool())?,
            area: b.at(3).and_then(|c| c.get_bool())?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RefusedFile {
    pub path: Vec<xpute_core::abi::link::Token>,
}

impl RefusedFile {
    pub fn write(&self, w: &mut PacketWriter) {
        w.branch(1, |w| {
            w.branch(self.path.len() as u32, |w| {
                for x in self.path.iter() {
                    (*x).write(w);
                }
            });
        });
    }

    pub fn read(c: Cursor<'_>) -> Result<RefusedFile, MarshalError> {
        let b = c.as_branch()?;
        Ok(RefusedFile {
            path: b.at(0).and_then(|c| {
                c.as_branch().and_then(|b| {
                    (0..b.len)
                        .map(|i| b.at(i).and_then(|c| xpute_core::abi::link::Token::read(c)))
                        .collect::<Result<Vec<_>, MarshalError>>()
                })
            })?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub pivot: PivotState,
    pub location: LocationState,
    pub companion: CompanionState,
    pub draft: Option<String>,
    pub orient: OrientationState,
    pub is_follow_mode: bool,
    pub camera_busy: bool,
    pub zooming: bool,
    pub over_marker: bool,
    pub settings: Vec<u32>,
    pub division: Vec<DivisionPart>,
    pub character_surfaces: Vec<u32>,
    pub refused_files: Vec<RefusedFile>,
    pub stall: Option<Message>,
    pub renderer_progress: f64,
    pub area_progress: f64,
}

impl Snapshot {
    pub fn write(&self, w: &mut PacketWriter) {
        w.branch(16, |w| {
            self.pivot.write(w);
            self.location.write(w);
            self.companion.write(w);
            match &self.draft {
                Some(v) => {
                    w.str(Some(&*(*v)));
                }
                None => {
                    w.nil();
                }
            }
            self.orient.write(w);
            w.bool(Some(self.is_follow_mode));
            w.bool(Some(self.camera_busy));
            w.bool(Some(self.zooming));
            w.bool(Some(self.over_marker));
            w.branch(self.settings.len() as u32, |w| {
                for x in self.settings.iter() {
                    w.u32(Some((*x) as u32));
                }
            });
            w.branch(self.division.len() as u32, |w| {
                for x in self.division.iter() {
                    (*x).write(w);
                }
            });
            w.branch(self.character_surfaces.len() as u32, |w| {
                for x in self.character_surfaces.iter() {
                    w.u32(Some((*x) as u32));
                }
            });
            w.branch(self.refused_files.len() as u32, |w| {
                for x in self.refused_files.iter() {
                    (*x).write(w);
                }
            });
            match &self.stall {
                Some(v) => {
                    w.u32(Some((*v) as u32));
                }
                None => {
                    w.nil();
                }
            }
            w.f64(Some(self.renderer_progress as f64));
            w.f64(Some(self.area_progress as f64));
        });
    }

    pub fn read(c: Cursor<'_>) -> Result<Snapshot, MarshalError> {
        let b = c.as_branch()?;
        Ok(Snapshot {
            pivot: b.at(0).and_then(|c| PivotState::read(c))?,
            location: b.at(1).and_then(|c| LocationState::read(c))?,
            companion: b.at(2).and_then(|c| CompanionState::read(c))?,
            draft: b.at(3).and_then(|c| {
                c.opt()
                    .map(|c| c.get_str_ref().map(str::to_owned))
                    .transpose()
            })?,
            orient: b.at(4).and_then(|c| OrientationState::read(c))?,
            is_follow_mode: b.at(5).and_then(|c| c.get_bool())?,
            camera_busy: b.at(6).and_then(|c| c.get_bool())?,
            zooming: b.at(7).and_then(|c| c.get_bool())?,
            over_marker: b.at(8).and_then(|c| c.get_bool())?,
            settings: b.at(9).and_then(|c| {
                c.as_branch().and_then(|b| {
                    (0..b.len)
                        .map(|i| b.at(i).and_then(|c| c.get_number().map(|v| v as u32)))
                        .collect::<Result<Vec<_>, MarshalError>>()
                })
            })?,
            division: b.at(10).and_then(|c| {
                c.as_branch().and_then(|b| {
                    (0..b.len)
                        .map(|i| b.at(i).and_then(|c| DivisionPart::read(c)))
                        .collect::<Result<Vec<_>, MarshalError>>()
                })
            })?,
            character_surfaces: b.at(11).and_then(|c| {
                c.as_branch().and_then(|b| {
                    (0..b.len)
                        .map(|i| b.at(i).and_then(|c| c.get_number().map(|v| v as u32)))
                        .collect::<Result<Vec<_>, MarshalError>>()
                })
            })?,
            refused_files: b.at(12).and_then(|c| {
                c.as_branch().and_then(|b| {
                    (0..b.len)
                        .map(|i| b.at(i).and_then(|c| RefusedFile::read(c)))
                        .collect::<Result<Vec<_>, MarshalError>>()
                })
            })?,
            stall: b.at(13).and_then(|c| {
                c.opt()
                    .map(|c| {
                        c.get_number().and_then(|v| {
                            Message::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                        })
                    })
                    .transpose()
            })?,
            renderer_progress: b.at(14).and_then(|c| c.get_number().map(|v| v as f64))?,
            area_progress: b.at(15).and_then(|c| c.get_number().map(|v| v as f64))?,
        })
    }
}

pub trait Overlay {
    fn show(&self, snapshot: Snapshot) -> Result<(), Errno>;
    fn view(&self, azimuth: f64, m_per_px: f64) -> Result<(), Errno>;
}

struct OverlayObject(Rc<dyn Overlay>);

impl Object for OverlayObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                SHOW => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let snapshot = b.at(0).and_then(|c| Snapshot::read(c))?;
                    match self.0.show(snapshot) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                VIEW => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let azimuth = b.at(0).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let m_per_px = b.at(1).and_then(|c| c.get_number().map(|v| v as f64))?;
                    match self.0.view(azimuth, m_per_px) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                _ => Reply::from(Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
    }
}

/// Mounts `imp` at the URI, one object for every open.
pub fn mount(imp: Rc<dyn Overlay>) {
    let o: Rc<dyn Object> = Rc::new(OverlayObject(imp));
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

pub fn show(snapshot: &Snapshot) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        SHOW,
        Some((1, &|w: &mut PacketWriter| {
            snapshot.write(w);
        })),
    );
    link::answer(r).map(|_| ())
}

pub fn view(azimuth: f64, m_per_px: f64) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        VIEW,
        Some((2, &|w: &mut PacketWriter| {
            w.f64(Some(azimuth as f64));
            w.f64(Some(m_per_px as f64));
        })),
    );
    link::answer(r).map(|_| ())
}
