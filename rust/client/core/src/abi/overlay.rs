// pxl-core/abi/overlay.rs
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
use xpute_link::abi::types::Token;
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `pxl:core/overlay` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x7b1393;

/// `show`
pub const SHOW: Method = 0;
/// `view`
pub const VIEW: Method = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum GroundStatus {
    Idle = 0,
    Loading = 1,
    Success = 2,
}

impl GroundStatus {
    /// Every member, in its source's own order.
    pub const ALL: [GroundStatus; 3] = [
        GroundStatus::Idle,
        GroundStatus::Loading,
        GroundStatus::Success,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<GroundStatus> {
        match val {
            0 => Some(GroundStatus::Idle),
            1 => Some(GroundStatus::Loading),
            2 => Some(GroundStatus::Success),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            GroundStatus::Idle => "Idle",
            GroundStatus::Loading => "Loading",
            GroundStatus::Success => "Success",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum SensorStatus {
    Idle = 0,
    Loading = 1,
    Ready = 2,
    Error = 3,
    Unavailable = 4,
}

impl SensorStatus {
    /// Every member, in its source's own order.
    pub const ALL: [SensorStatus; 5] = [
        SensorStatus::Idle,
        SensorStatus::Loading,
        SensorStatus::Ready,
        SensorStatus::Error,
        SensorStatus::Unavailable,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<SensorStatus> {
        match val {
            0 => Some(SensorStatus::Idle),
            1 => Some(SensorStatus::Loading),
            2 => Some(SensorStatus::Ready),
            3 => Some(SensorStatus::Error),
            4 => Some(SensorStatus::Unavailable),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            SensorStatus::Idle => "Idle",
            SensorStatus::Loading => "Loading",
            SensorStatus::Ready => "Ready",
            SensorStatus::Error => "Error",
            SensorStatus::Unavailable => "Unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Message {
    GeolocationUnsupported = 0,
    GeolocationPermissionDenied = 1,
    GeolocationPositionUnavailable = 2,
    GeolocationTimeout = 3,
    GeolocationGenericError = 4,
    /// Reads reach the server and fail; they are tried again.
    WorldReadFailing = 5,
    /// Reads failed past every retry; the world is not coming.
    WorldUnavailable = 6,
}

impl Message {
    /// Every member, in its source's own order.
    pub const ALL: [Message; 7] = [
        Message::GeolocationUnsupported,
        Message::GeolocationPermissionDenied,
        Message::GeolocationPositionUnavailable,
        Message::GeolocationTimeout,
        Message::GeolocationGenericError,
        Message::WorldReadFailing,
        Message::WorldUnavailable,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<Message> {
        match val {
            0 => Some(Message::GeolocationUnsupported),
            1 => Some(Message::GeolocationPermissionDenied),
            2 => Some(Message::GeolocationPositionUnavailable),
            3 => Some(Message::GeolocationTimeout),
            4 => Some(Message::GeolocationGenericError),
            5 => Some(Message::WorldReadFailing),
            6 => Some(Message::WorldUnavailable),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Message::GeolocationUnsupported => "GeolocationUnsupported",
            Message::GeolocationPermissionDenied => "GeolocationPermissionDenied",
            Message::GeolocationPositionUnavailable => "GeolocationPositionUnavailable",
            Message::GeolocationTimeout => "GeolocationTimeout",
            Message::GeolocationGenericError => "GeolocationGenericError",
            Message::WorldReadFailing => "WorldReadFailing",
            Message::WorldUnavailable => "WorldUnavailable",
        }
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
                    w.u32(Some(*v as u32));
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
    pub path: Vec<Token>,
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
                        .map(|i| b.at(i).and_then(|c| Token::read(c)))
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
                    w.str(Some(&**v));
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
            w.array::<u32>(Some(&*self.settings));
            w.branch(self.division.len() as u32, |w| {
                for x in self.division.iter() {
                    (*x).write(w);
                }
            });
            w.array::<u32>(Some(&*self.character_surfaces));
            w.branch(self.refused_files.len() as u32, |w| {
                for x in self.refused_files.iter() {
                    (*x).write(w);
                }
            });
            match &self.stall {
                Some(v) => {
                    w.u32(Some(*v as u32));
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
            settings: b
                .at(9)
                .and_then(|c| c.get_array::<u32>().map(<[u32]>::to_vec))?,
            division: b.at(10).and_then(|c| {
                c.as_branch().and_then(|b| {
                    (0..b.len)
                        .map(|i| b.at(i).and_then(|c| DivisionPart::read(c)))
                        .collect::<Result<Vec<_>, MarshalError>>()
                })
            })?,
            character_surfaces: b
                .at(11)
                .and_then(|c| c.get_array::<u32>().map(<[u32]>::to_vec))?,
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

pub fn show(snapshot: &Snapshot) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        SHOW,
        Some((1, &|w_: &mut PacketWriter| {
            snapshot.write(w_);
        })),
    );
    xpute_link::answer(r).map(|_| ())
}

pub fn view(azimuth: f64, m_per_px: f64) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        VIEW,
        Some((2, &|w_: &mut PacketWriter| {
            w_.f64(Some(azimuth as f64));
            w_.f64(Some(m_per_px as f64));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}
