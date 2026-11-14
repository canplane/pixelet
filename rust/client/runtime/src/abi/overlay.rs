// pxl-runtime/abi/overlay.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/overlay` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x7b1393;

/// `show`
pub const SHOW: _rt::Method = 0;
/// `view`
pub const VIEW: _rt::Method = 1;

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
    pub const SZ: usize = 16;
    pub const ALIGN: usize = 8;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.f64(at, self.lon);
        w.f64(at + 8, self.lat);
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<LonLat, _rt::Errno> {
        Ok(LonLat {
            lon: r.f64(at)?,
            lat: r.f64(at + 8)?,
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
    pub const SZ: usize = 40;
    pub const ALIGN: usize = 8;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.u8(at, self.status as u8);
        w.bool(at + 1, self.is_off_center);
        if let Some(v) = &self.lon_lat {
            w.u8(at + 8, 1);
            (v).store(w, at + 8 + 8);
        }
        w.bool(at + 32, self.outside);
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<PivotState, _rt::Errno> {
        Ok(PivotState {
            status: r
                .case(at, 3)
                .and_then(|v| GroundStatus::of(v).ok_or(_rt::Errno::EBADMSG))?,
            is_off_center: r.bool(at + 1)?,
            lon_lat: r.case(at + 8, 2).and_then(|d| {
                if d == 0 {
                    Ok(None)
                } else {
                    LonLat::load(r, at + 8 + 8).map(Some)
                }
            })?,
            outside: r.bool(at + 32)?,
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
    pub const SZ: usize = 5;
    pub const ALIGN: usize = 1;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.u8(at, self.status as u8);
        w.bool(at + 1, self.has_location);
        w.bool(at + 2, self.watching);
        if let Some(v) = &self.err {
            w.u8(at + 3, 1);
            w.u8(at + 3 + 1, *v as u8);
        }
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<LocationState, _rt::Errno> {
        Ok(LocationState {
            status: r
                .case(at, 5)
                .and_then(|v| SensorStatus::of(v).ok_or(_rt::Errno::EBADMSG))?,
            has_location: r.bool(at + 1)?,
            watching: r.bool(at + 2)?,
            err: r.case(at + 3, 2).and_then(|d| {
                if d == 0 {
                    Ok(None)
                } else {
                    r.case(at + 3 + 1, 7)
                        .and_then(|v| Message::of(v).ok_or(_rt::Errno::EBADMSG))
                        .map(Some)
                }
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
    pub const SZ: usize = 2;
    pub const ALIGN: usize = 1;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.bool(at, self.person_ready);
        w.bool(at + 1, self.pet_ready);
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<CompanionState, _rt::Errno> {
        Ok(CompanionState {
            person_ready: r.bool(at)?,
            pet_ready: r.bool(at + 1)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OrientationState {
    pub status: SensorStatus,
}

impl OrientationState {
    pub const SZ: usize = 1;
    pub const ALIGN: usize = 1;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.u8(at, self.status as u8);
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<OrientationState, _rt::Errno> {
        Ok(OrientationState {
            status: r
                .case(at, 5)
                .and_then(|v| SensorStatus::of(v).ok_or(_rt::Errno::EBADMSG))?,
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
    pub const SZ: usize = 16;
    pub const ALIGN: usize = 4;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.u8(at, self.subtype);
        w.str(at + 4, &self.name);
        w.bool(at + 12, self.shown);
        w.bool(at + 13, self.area);
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<DivisionPart, _rt::Errno> {
        Ok(DivisionPart {
            subtype: r.u8(at)?,
            name: r.str(at + 4).map(str::to_owned)?,
            shown: r.bool(at + 12)?,
            area: r.bool(at + 13)?,
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
    pub stall: Option<Message>,
    pub renderer_progress: f64,
    pub area_progress: f64,
}

impl Snapshot {
    pub const SZ: usize = 112;
    pub const ALIGN: usize = 8;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        self.pivot.store(w, at);
        self.location.store(w, at + 40);
        self.companion.store(w, at + 45);
        if let Some(v) = &self.draft {
            w.u8(at + 48, 1);
            w.str(at + 48 + 4, v);
        }
        self.orient.store(w, at + 60);
        w.bool(at + 61, self.is_follow_mode);
        w.bool(at + 62, self.camera_busy);
        w.bool(at + 63, self.zooming);
        w.bool(at + 64, self.over_marker);
        w.arr(at + 68, &self.settings);
        if let Some(b) = w.list(
            at + 76,
            self.division.len(),
            DivisionPart::SZ,
            DivisionPart::ALIGN,
        ) {
            for (i, x) in self.division.iter().enumerate() {
                (x).store(w, b + i * DivisionPart::SZ);
            }
        }
        w.arr(at + 84, &self.character_surfaces);
        if let Some(v) = &self.stall {
            w.u8(at + 92, 1);
            w.u8(at + 92 + 1, *v as u8);
        }
        w.f64(at + 96, self.renderer_progress);
        w.f64(at + 104, self.area_progress);
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<Snapshot, _rt::Errno> {
        Ok(Snapshot {
            pivot: PivotState::load(r, at)?,
            location: LocationState::load(r, at + 40)?,
            companion: CompanionState::load(r, at + 45)?,
            draft: r.case(at + 48, 2).and_then(|d| {
                if d == 0 {
                    Ok(None)
                } else {
                    r.str(at + 48 + 4).map(str::to_owned).map(Some)
                }
            })?,
            orient: OrientationState::load(r, at + 60)?,
            is_follow_mode: r.bool(at + 61)?,
            camera_busy: r.bool(at + 62)?,
            zooming: r.bool(at + 63)?,
            over_marker: r.bool(at + 64)?,
            settings: r.arr::<u32>(at + 68)?,
            division: r
                .list(at + 76, DivisionPart::SZ, DivisionPart::ALIGN)
                .and_then(|(b, n)| {
                    (0..n)
                        .map(|i| DivisionPart::load(r, b + i * DivisionPart::SZ))
                        .collect::<Result<Vec<_>, _rt::Errno>>()
                })?,
            character_surfaces: r.arr::<u32>(at + 84)?,
            stall: r.case(at + 92, 2).and_then(|d| {
                if d == 0 {
                    Ok(None)
                } else {
                    r.case(at + 92 + 1, 7)
                        .and_then(|v| Message::of(v).ok_or(_rt::Errno::EBADMSG))
                        .map(Some)
                }
            })?,
            renderer_progress: r.f64(at + 96)?,
            area_progress: r.f64(at + 104)?,
        })
    }
}

pub trait Host {
    fn show(&self, snapshot: Snapshot) -> Result<(), _rt::Errno>;
    fn view(&self, azimuth: f64, m_per_px: f64) -> Result<(), _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Host>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                SHOW => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let snapshot = Snapshot::load(r_, 0)?;
                    match self.0.show(snapshot) {
                        Ok(()) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                VIEW => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let azimuth = r_.f64(0)?;
                    let m_per_px = r_.f64(8)?;
                    match self.0.view(azimuth, m_per_px) {
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
pub fn mount(implementation: _rt::Rc<dyn Host>) {
    _rt::link::mount_named(HANDLE, _rt::Rc::new(_Object(implementation)));
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Reply, Tag};
    pub(super) use xpute_native_runtime::link;
}
