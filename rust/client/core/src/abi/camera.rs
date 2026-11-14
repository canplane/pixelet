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

/// The interface's handle, named by `pxl:core/camera` (frame.json `NAMED`).
pub const HANDLE: _rt::Handle = 0x5a7108;

/// `track`
pub const TRACK: _rt::Method = 0;
/// `north-up`
pub const NORTH_UP: _rt::Method = 1;
/// `goto`
pub const GOTO: _rt::Method = 2;

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

pub trait Guest {
    fn track(&self, tracking: Tracking) -> Result<(), _rt::Errno>;
    fn north_up(&self) -> Result<(), _rt::Errno>;
    fn goto(
        &self,
        lon: f64,
        lat: f64,
        dist: Option<f64>,
        azimuth: Option<f64>,
        polar: Option<f64>,
    ) -> Result<(), _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Guest>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, tag_: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                TRACK => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let tracking = r_
                        .case(0, 3)
                        .and_then(|v| Tracking::of(v).ok_or(_rt::Errno::EBADMSG))?;
                    match self.0.track(tracking) {
                        Ok(v) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                NORTH_UP => match self.0.north_up() {
                    Ok(v) => _rt::Reply::from(0),
                    Err(e) => _rt::Reply::from(e as i32),
                },
                GOTO => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let lon = r_.f64(0)?;
                    let lat = r_.f64(8)?;
                    let dist = r_.case(16, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            r_.f64(16 + 8).map(Some)
                        }
                    })?;
                    let azimuth = r_.case(32, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            r_.f64(32 + 8).map(Some)
                        }
                    })?;
                    let polar = r_.case(48, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            r_.f64(48 + 8).map(Some)
                        }
                    })?;
                    match self.0.goto(lon, lat, dist, azimuth, polar) {
                        Ok(v) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                _ => _rt::Reply::from(_rt::Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| _rt::Reply::from(e as i32))
    }
}

/// Serves `imp` under the interface's handle.
pub fn mount(imp: _rt::Rc<dyn Guest>) {
    _rt::link::mount_named(HANDLE, _rt::Rc::new(_Object(imp)));
}

mod _rt {
    pub use std::rc::Rc;
    pub use xpute_guest::link;
    pub use xpute_kit::status::errno::Errno;
    pub use xpute_link::{Handle, Lift, Method, Object, Reply, Tag};
}
