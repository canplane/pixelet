// pxl-core/abi/overlay.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/overlay` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x7b1393;

/// `show`
pub const SHOW: _rt::Method = 0;
/// `draft`
pub const DRAFT: _rt::Method = 1;
/// `view`
pub const VIEW: _rt::Method = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Message {
    /// Reads reach the server and fail; they are tried again.
    WorldReadFailing = 0,
    /// Reads failed past every retry; the world is not coming.
    WorldUnavailable = 1,
}

impl Message {
    /// Every member, in its source's own order.
    pub const ALL: [Message; 2] = [Message::WorldReadFailing, Message::WorldUnavailable];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<Message> {
        match val {
            0 => Some(Message::WorldReadFailing),
            1 => Some(Message::WorldUnavailable),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
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
    pub ground_shown: bool,
    pub is_off_center: bool,
    pub lon_lat: Option<LonLat>,
    pub outside: bool,
}

impl PivotState {
    pub const SZ: usize = 40;
    pub const ALIGN: usize = 8;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.bool(at, self.ground_shown);
        w.bool(at + 1, self.is_off_center);
        if let Some(v) = &self.lon_lat {
            w.u8(at + 8, 1);
            (v).store(w, at + 8 + 8);
        }
        w.bool(at + 32, self.outside);
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<PivotState, _rt::Errno> {
        Ok(PivotState {
            ground_shown: r.bool(at)?,
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
    pub companion_ready: bool,
    pub drafting: bool,
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
    pub const SZ: usize = 96;
    pub const ALIGN: usize = 8;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        self.pivot.store(w, at);
        w.bool(at + 40, self.companion_ready);
        w.bool(at + 41, self.drafting);
        w.bool(at + 42, self.is_follow_mode);
        w.bool(at + 43, self.camera_busy);
        w.bool(at + 44, self.zooming);
        w.bool(at + 45, self.over_marker);
        w.arr(at + 48, &self.settings);
        if let Some(b) = w.list(
            at + 56,
            self.division.len(),
            DivisionPart::SZ,
            DivisionPart::ALIGN,
        ) {
            for (i, x) in self.division.iter().enumerate() {
                (x).store(w, b + i * DivisionPart::SZ);
            }
        }
        w.arr(at + 64, &self.character_surfaces);
        if let Some(v) = &self.stall {
            w.u8(at + 72, 1);
            w.u8(at + 72 + 1, *v as u8);
        }
        w.f64(at + 80, self.renderer_progress);
        w.f64(at + 88, self.area_progress);
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<Snapshot, _rt::Errno> {
        Ok(Snapshot {
            pivot: PivotState::load(r, at)?,
            companion_ready: r.bool(at + 40)?,
            drafting: r.bool(at + 41)?,
            is_follow_mode: r.bool(at + 42)?,
            camera_busy: r.bool(at + 43)?,
            zooming: r.bool(at + 44)?,
            over_marker: r.bool(at + 45)?,
            settings: r.arr::<u32>(at + 48)?,
            division: r
                .list(at + 56, DivisionPart::SZ, DivisionPart::ALIGN)
                .and_then(|(b, n)| {
                    (0..n)
                        .map(|i| DivisionPart::load(r, b + i * DivisionPart::SZ))
                        .collect::<Result<Vec<_>, _rt::Errno>>()
                })?,
            character_surfaces: r.arr::<u32>(at + 64)?,
            stall: r.case(at + 72, 2).and_then(|d| {
                if d == 0 {
                    Ok(None)
                } else {
                    r.case(at + 72 + 1, 2)
                        .and_then(|v| Message::of(v).ok_or(_rt::Errno::EBADMSG))
                        .map(Some)
                }
            })?,
            renderer_progress: r.f64(at + 80)?,
            area_progress: r.f64(at + 88)?,
        })
    }
}

pub fn show(snapshot: &Snapshot) -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(
        HANDLE,
        SHOW,
        Some((96, &|w_: &mut _rt::Lower| {
            (snapshot).store(w_, 0);
        })),
    );
    _rt::answer(r)
}

pub fn draft(text: &str) -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(
        HANDLE,
        DRAFT,
        Some((8, &|w_: &mut _rt::Lower| {
            w_.str(0, text);
        })),
    );
    _rt::answer(r)
}

pub fn view(azimuth: f64, m_per_px: f64) -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(
        HANDLE,
        VIEW,
        Some((16, &|w_: &mut _rt::Lower| {
            w_.f64(0, azimuth);
            w_.f64(8, m_per_px);
        })),
    );
    _rt::answer(r)
}

mod _rt {
    pub(super) use xpute_guest::link;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{answer, Handle, Lift, Lower, Method};
}
