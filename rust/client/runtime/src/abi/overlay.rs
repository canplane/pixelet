// pxl-runtime/abi/overlay.rs
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
    pub over_marker: bool,
    pub settings: Vec<u32>,
    pub division: Vec<DivisionPart>,
    pub character_surfaces: Vec<u32>,
    pub stall: Option<Message>,
    pub renderer_progress: f64,
    pub area_progress: f64,
}

impl Snapshot {
    pub const SZ: usize = 88;
    pub const ALIGN: usize = 8;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        self.pivot.store(w, at);
        w.bool(at + 40, self.companion_ready);
        w.bool(at + 41, self.drafting);
        w.bool(at + 42, self.is_follow_mode);
        w.bool(at + 43, self.over_marker);
        w.arr(at + 44, &self.settings);
        if let Some(b) = w.list(
            at + 52,
            self.division.len(),
            DivisionPart::SZ,
            DivisionPart::ALIGN,
        ) {
            for (i, x) in self.division.iter().enumerate() {
                (x).store(w, b + i * DivisionPart::SZ);
            }
        }
        w.arr(at + 60, &self.character_surfaces);
        if let Some(v) = &self.stall {
            w.u8(at + 68, 1);
            w.u8(at + 68 + 1, *v as u8);
        }
        w.f64(at + 72, self.renderer_progress);
        w.f64(at + 80, self.area_progress);
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<Snapshot, _rt::Errno> {
        Ok(Snapshot {
            pivot: PivotState::load(r, at)?,
            companion_ready: r.bool(at + 40)?,
            drafting: r.bool(at + 41)?,
            is_follow_mode: r.bool(at + 42)?,
            over_marker: r.bool(at + 43)?,
            settings: r.arr::<u32>(at + 44)?,
            division: r
                .list(at + 52, DivisionPart::SZ, DivisionPart::ALIGN)
                .and_then(|(b, n)| {
                    (0..n)
                        .map(|i| DivisionPart::load(r, b + i * DivisionPart::SZ))
                        .collect::<Result<Vec<_>, _rt::Errno>>()
                })?,
            character_surfaces: r.arr::<u32>(at + 60)?,
            stall: r.case(at + 68, 2).and_then(|d| {
                if d == 0 {
                    Ok(None)
                } else {
                    r.case(at + 68 + 1, 2)
                        .and_then(|v| Message::of(v).ok_or(_rt::Errno::EBADMSG))
                        .map(Some)
                }
            })?,
            renderer_progress: r.f64(at + 72)?,
            area_progress: r.f64(at + 80)?,
        })
    }
}

pub trait Host {
    fn show(&self, snapshot: Snapshot, task_return: _rt::Return<()>);
    fn draft(&self, text: String, task_return: _rt::Return<()>);
    fn view(&self, azimuth: f64, m_per_px: f64, task_return: _rt::Return<()>);
}

struct _Object(&'static _rt::Side, _rt::Rc<dyn Host>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, tag_: _rt::Tag, args_: _rt::Lift, _: &mut _rt::Lower) -> i32 {
        let run = || -> Result<i32, _rt::Errno> {
            Ok(match m_ {
                SHOW => {
                    let snapshot = Snapshot::load(args_, 0)?;
                    let side_ = self.0;
                    self.1.show(
                        snapshot,
                        _rt::Return::new(
                            Box::new(move |stop| side_.on_cancel(tag_, stop)),
                            Box::new(move |got| match got {
                                Ok(()) => {
                                    side_.task_return(tag_, 0, _rt::Packet::Args(None));
                                }
                                Err(e) => {
                                    side_.task_return(tag_, e as i32, _rt::Packet::Args(None));
                                }
                            }),
                        ),
                    );
                    _rt::Errno::EINPROGRESS as i32
                }
                DRAFT => {
                    let text = args_.str(0).map(str::to_owned)?;
                    let side_ = self.0;
                    self.1.draft(
                        text,
                        _rt::Return::new(
                            Box::new(move |stop| side_.on_cancel(tag_, stop)),
                            Box::new(move |got| match got {
                                Ok(()) => {
                                    side_.task_return(tag_, 0, _rt::Packet::Args(None));
                                }
                                Err(e) => {
                                    side_.task_return(tag_, e as i32, _rt::Packet::Args(None));
                                }
                            }),
                        ),
                    );
                    _rt::Errno::EINPROGRESS as i32
                }
                VIEW => {
                    let azimuth = args_.f64(0)?;
                    let m_per_px = args_.f64(8)?;
                    let side_ = self.0;
                    self.1.view(
                        azimuth,
                        m_per_px,
                        _rt::Return::new(
                            Box::new(move |stop| side_.on_cancel(tag_, stop)),
                            Box::new(move |got| match got {
                                Ok(()) => {
                                    side_.task_return(tag_, 0, _rt::Packet::Args(None));
                                }
                                Err(e) => {
                                    side_.task_return(tag_, e as i32, _rt::Packet::Args(None));
                                }
                            }),
                        ),
                    );
                    _rt::Errno::EINPROGRESS as i32
                }
                _ => _rt::Errno::ENOSYS as i32,
            })
        };
        run().unwrap_or_else(|e| e as i32)
    }
}

/// Mounts `implementation` under the interface's handle on `side_`.
pub fn mount(side_: &'static _rt::Side, implementation: _rt::Rc<dyn Host>) {
    side_.mount_named(HANDLE, _rt::Rc::new(_Object(side_, implementation)));
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Packet, Return, Side, Tag};
}
