// pxl-runtime/abi/input.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/input` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x5ee0f8;

/// `pointer`
pub const POINTER: _rt::Method = 0;
/// `typing`
pub const TYPING: _rt::Method = 1;
/// `wheel`
pub const WHEEL: _rt::Method = 2;
/// `pinch`
pub const PINCH: _rt::Method = 3;
/// `move`
pub const MOVE: _rt::Method = 4;
/// `xr-pose`
pub const XR_POSE: _rt::Method = 5;
/// `xr-pose-end`
pub const XR_POSE_END: _rt::Method = 6;

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
pub enum PointerType {
    Mouse = 0,
    Touch = 1,
    Pen = 2,
}

impl PointerType {
    /// Every member, in its source's own order.
    pub const ALL: [PointerType; 3] = [PointerType::Mouse, PointerType::Touch, PointerType::Pen];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<PointerType> {
        match val {
            0 => Some(PointerType::Mouse),
            1 => Some(PointerType::Touch),
            2 => Some(PointerType::Pen),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            PointerType::Mouse => "Mouse",
            PointerType::Touch => "Touch",
            PointerType::Pen => "Pen",
        }
    }
}

pub fn pointer(
    phase: PointerPhase,
    pointer_type: PointerType,
    id: i32,
    button: i32,
    x: f64,
    y: f64,
) -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(
        HANDLE,
        POINTER,
        Some((32, &|w_: &mut _rt::Lower| {
            w_.u8(0, phase as u8);
            w_.u8(1, pointer_type as u8);
            w_.i32(4, id);
            w_.i32(8, button);
            w_.f64(16, x);
            w_.f64(24, y);
        })),
    );
    _rt::answer(r)
}

pub fn typing(typing: bool) -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(
        HANDLE,
        TYPING,
        Some((1, &|w_: &mut _rt::Lower| {
            w_.bool(0, typing);
        })),
    );
    _rt::answer(r)
}

pub fn wheel(delta_y: f64) -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(
        HANDLE,
        WHEEL,
        Some((8, &|w_: &mut _rt::Lower| {
            w_.f64(0, delta_y);
        })),
    );
    _rt::answer(r)
}

pub fn pinch(scale: f64) -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(
        HANDLE,
        PINCH,
        Some((8, &|w_: &mut _rt::Lower| {
            w_.f64(0, scale);
        })),
    );
    _rt::answer(r)
}

pub fn r#move(x: i32, y: i32) -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(
        HANDLE,
        MOVE,
        Some((8, &|w_: &mut _rt::Lower| {
            w_.i32(0, x);
            w_.i32(4, y);
        })),
    );
    _rt::answer(r)
}

pub fn xr_pose(
    x: f64,
    y: f64,
    z: f64,
    qx: f64,
    qy: f64,
    qz: f64,
    qw: f64,
) -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(
        HANDLE,
        XR_POSE,
        Some((56, &|w_: &mut _rt::Lower| {
            w_.f64(0, x);
            w_.f64(8, y);
            w_.f64(16, z);
            w_.f64(24, qx);
            w_.f64(32, qy);
            w_.f64(40, qz);
            w_.f64(48, qw);
        })),
    );
    _rt::answer(r)
}

pub fn xr_pose_end() -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(HANDLE, XR_POSE_END, None);
    _rt::answer(r)
}

mod _rt {
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{answer, Handle, Lower, Method};
    pub(super) use xpute_native_runtime::link;
}
