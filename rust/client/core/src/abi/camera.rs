// pxl-core/abi/camera.rs
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
pub const URI: [&str; 2] = ["pxl:core", "camera"];

/// `track`
pub const TRACK: Method = 0;
/// `north-up`
pub const NORTH_UP: Method = 1;
/// `goto`
pub const GOTO: Method = 2;

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tracking {
    #[default]
    Recenter = 0,
    Follow = 1,
    Unfollow = 2,
}

impl Tracking {
    pub const ALL: [Tracking; 3] = [Tracking::Recenter, Tracking::Follow, Tracking::Unfollow];

    pub fn of(v: u32) -> Option<Tracking> {
        Tracking::ALL.get(v as usize).copied()
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

/// Mounts `imp` at the URI, one object for every open.
pub fn mount(imp: Rc<dyn Camera>) {
    let o: Rc<dyn Object> = Rc::new(CameraObject(imp));
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

pub fn track(tracking: Tracking) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        TRACK,
        Some((1, &|w: &mut PacketWriter| {
            w.u32(Some(tracking as u32));
        })),
    );
    link::answer(r).map(|_| ())
}

pub fn north_up() -> Result<(), Errno> {
    let (r, data) = link::call(handle(), NORTH_UP, None);
    link::answer(r).map(|_| ())
}

pub fn goto(
    lon: f64,
    lat: f64,
    dist: Option<f64>,
    azimuth: Option<f64>,
    polar: Option<f64>,
) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        GOTO,
        Some((5, &|w: &mut PacketWriter| {
            w.f64(Some(lon as f64));
            w.f64(Some(lat as f64));
            match &dist {
                Some(v) => {
                    w.f64(Some((*v) as f64));
                }
                None => {
                    w.nil();
                }
            }
            match &azimuth {
                Some(v) => {
                    w.f64(Some((*v) as f64));
                }
                None => {
                    w.nil();
                }
            }
            match &polar {
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
