// pxl-core/abi/camera.rs
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
use xpute_guest::wait::wait;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `pxl:core/camera` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x5a7108;

/// The interface's qualified name: its package and its name.
pub const NAME: [&str; 2] = ["pxl:core", "camera"];

/// `track`
pub const TRACK: Method = 0;
/// `north-up`
pub const NORTH_UP: Method = 1;
/// `goto`
pub const GOTO: Method = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Tracking {
    Recenter = 0,
    Follow = 1,
    Unfollow = 2,
}

impl Tracking {
    /// Every member, in its source's own order.
    pub const ALL: [Tracking; 3] = [Tracking::Recenter, Tracking::Follow, Tracking::Unfollow];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<Tracking> {
        match val {
            0 => Some(Tracking::Recenter),
            1 => Some(Tracking::Follow),
            2 => Some(Tracking::Unfollow),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Tracking::Recenter => "Recenter",
            Tracking::Follow => "Follow",
            Tracking::Unfollow => "Unfollow",
        }
    }
}

pub trait Camera {
    fn track(&self, tracking: Tracking) -> Result<(), Errno>;
    fn north_up(&self) -> Result<(), Errno>;
    fn goto(
        &self,
        lon: f64,
        lat: f64,
        dist: Option<f64>,
        azimuth: Option<f64>,
        polar: Option<f64>,
    ) -> Result<(), Errno>;
}

struct CameraObject(Rc<dyn Camera>);

impl Object for CameraObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                TRACK => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let tracking = b.at(0).and_then(|c| {
                        c.get_number().and_then(|v| {
                            Tracking::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                        })
                    })?;
                    match self.0.track(tracking) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                NORTH_UP => match self.0.north_up() {
                    Ok(v) => Reply::from(0),
                    Err(e) => Reply::from(e as i32),
                },
                GOTO => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let lon = b.at(0).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let lat = b.at(1).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let dist = b.at(2).and_then(|c| {
                        c.opt()
                            .map(|c| c.get_number().map(|v| v as f64))
                            .transpose()
                    })?;
                    let azimuth = b.at(3).and_then(|c| {
                        c.opt()
                            .map(|c| c.get_number().map(|v| v as f64))
                            .transpose()
                    })?;
                    let polar = b.at(4).and_then(|c| {
                        c.opt()
                            .map(|c| c.get_number().map(|v| v as f64))
                            .transpose()
                    })?;
                    match self.0.goto(lon, lat, dist, azimuth, polar) {
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

/// Serves `imp` under the interface's handle.
pub fn mount(imp: Rc<dyn Camera>) {
    link::mount_named(HANDLE, Rc::new(CameraObject(imp)));
}

pub fn track(tracking: Tracking) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        TRACK,
        Some((1, &|w: &mut PacketWriter| {
            w.u32(Some(tracking as u32));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}

pub fn north_up() -> Result<(), Errno> {
    let (r, data) = link::call(HANDLE, NORTH_UP, None);
    xpute_link::answer(r).map(|_| ())
}

pub fn goto(
    lon: f64,
    lat: f64,
    dist: Option<f64>,
    azimuth: Option<f64>,
    polar: Option<f64>,
) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        GOTO,
        Some((5, &|w: &mut PacketWriter| {
            w.f64(Some(lon as f64));
            w.f64(Some(lat as f64));
            match &dist {
                Some(v) => {
                    w.f64(Some(*v as f64));
                }
                None => {
                    w.nil();
                }
            }
            match &azimuth {
                Some(v) => {
                    w.f64(Some(*v as f64));
                }
                None => {
                    w.nil();
                }
            }
            match &polar {
                Some(v) => {
                    w.f64(Some(*v as f64));
                }
                None => {
                    w.nil();
                }
            }
        })),
    );
    xpute_link::answer(r).map(|_| ())
}
