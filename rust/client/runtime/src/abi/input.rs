// pxl-runtime/abi/input.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/input` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x5ee0f8;

/// `pointer`
pub const POINTER: _rt::Method = 0;
/// `wheel`
pub const WHEEL: _rt::Method = 1;
/// `pinch`
pub const PINCH: _rt::Method = 2;
/// `move`
pub const MOVE: _rt::Method = 3;
/// `xr-pose`
pub const XR_POSE: _rt::Method = 4;
/// `xr-pose-end`
pub const XR_POSE_END: _rt::Method = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum PointerPhase {
    Down = 0,
    Move = 1,
    Up = 2,
    Out = 3,
    /// The pointer's gesture ends with no tap: the platform took it, or the UI
    /// spent it on something of its own.
    Cancel = 4,
}

impl PointerPhase {
    /// Every member, in its source's own order.
    pub const ALL: [PointerPhase; 5] = [
        PointerPhase::Down,
        PointerPhase::Move,
        PointerPhase::Up,
        PointerPhase::Out,
        PointerPhase::Cancel,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<PointerPhase> {
        match val {
            0 => Some(PointerPhase::Down),
            1 => Some(PointerPhase::Move),
            2 => Some(PointerPhase::Up),
            3 => Some(PointerPhase::Out),
            4 => Some(PointerPhase::Cancel),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            PointerPhase::Down => "Down",
            PointerPhase::Move => "Move",
            PointerPhase::Up => "Up",
            PointerPhase::Out => "Out",
            PointerPhase::Cancel => "Cancel",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Drag {
    None = 0,
    Pan = 1,
    Rotate = 2,
}

impl Drag {
    /// Every member, in its source's own order.
    pub const ALL: [Drag; 3] = [Drag::None, Drag::Pan, Drag::Rotate];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<Drag> {
        match val {
            0 => Some(Drag::None),
            1 => Some(Drag::Pan),
            2 => Some(Drag::Rotate),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Drag::None => "None",
            Drag::Pan => "Pan",
            Drag::Rotate => "Rotate",
        }
    }
}

pub fn pointer(
    side_: &_rt::Side,
    phase: PointerPhase,
    touch: bool,
    id: i32,
    drag: Drag,
    x: f64,
    y: f64,
) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(
        HANDLE,
        POINTER,
        _rt::Packet::Args(Some((32, &|w_: &mut _rt::Lower| {
            w_.u8(0, phase as u8);
            w_.bool(1, touch);
            w_.i32(4, id);
            w_.u8(8, drag as u8);
            w_.f64(16, x);
            w_.f64(24, y);
        }))),
    );
    _rt::answer(r)
}

pub fn wheel(side_: &_rt::Side, delta_y: f64) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(
        HANDLE,
        WHEEL,
        _rt::Packet::Args(Some((8, &|w_: &mut _rt::Lower| {
            w_.f64(0, delta_y);
        }))),
    );
    _rt::answer(r)
}

pub fn pinch(side_: &_rt::Side, scale: f64) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(
        HANDLE,
        PINCH,
        _rt::Packet::Args(Some((8, &|w_: &mut _rt::Lower| {
            w_.f64(0, scale);
        }))),
    );
    _rt::answer(r)
}

pub fn r#move(side_: &_rt::Side, x: i32, y: i32) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(
        HANDLE,
        MOVE,
        _rt::Packet::Args(Some((8, &|w_: &mut _rt::Lower| {
            w_.i32(0, x);
            w_.i32(4, y);
        }))),
    );
    _rt::answer(r)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the WIT function's parameters, each its own"
)]
pub fn xr_pose(
    side_: &_rt::Side,
    x: f64,
    y: f64,
    z: f64,
    qx: f64,
    qy: f64,
    qz: f64,
    qw: f64,
) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(
        HANDLE,
        XR_POSE,
        _rt::Packet::Args(Some((56, &|w_: &mut _rt::Lower| {
            w_.f64(0, x);
            w_.f64(8, y);
            w_.f64(16, z);
            w_.f64(24, qx);
            w_.f64(32, qy);
            w_.f64(40, qz);
            w_.f64(48, qw);
        }))),
    );
    _rt::answer(r)
}

pub fn xr_pose_end(side_: &_rt::Side) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(HANDLE, XR_POSE_END, _rt::Packet::Args(None));
    _rt::answer(r)
}

mod _rt {
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lower, Method, Packet, Side, answer};
}
