// pxl-runtime/abi/camera.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/camera` (frame.json5 `NAMED`).
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

pub fn track(side_: &_rt::Side, tracking: Tracking) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(
        HANDLE,
        TRACK,
        _rt::Packet::Args(Some((1, &|w_: &mut _rt::Lower| {
            w_.u8(0, tracking as u8);
        }))),
    );
    _rt::answer(r)
}

pub fn north_up(side_: &_rt::Side) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(HANDLE, NORTH_UP, _rt::Packet::Args(None));
    _rt::answer(r)
}

pub fn goto(side_: &_rt::Side, lon: f64, lat: f64) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(
        HANDLE,
        GOTO,
        _rt::Packet::Args(Some((16, &|w_: &mut _rt::Lower| {
            w_.f64(0, lon);
            w_.f64(8, lat);
        }))),
    );
    _rt::answer(r)
}

mod _rt {
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{answer, Handle, Lower, Method, Packet, Side};
}
