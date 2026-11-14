// pxl-core/abi/input.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

/// The interface's handle, named by `pxl:core/input` (frame.json `NAMED`).
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

pub trait Guest {
    fn pointer(
        &self,
        phase: PointerPhase,
        pointer_type: PointerType,
        id: i32,
        button: i32,
        x: f64,
        y: f64,
    ) -> Result<(), _rt::Errno>;
    fn typing(&self, typing: bool) -> Result<(), _rt::Errno>;
    fn wheel(&self, delta_y: f64) -> Result<(), _rt::Errno>;
    fn pinch(&self, scale: f64) -> Result<(), _rt::Errno>;
    fn r#move(&self, x: i32, y: i32) -> Result<(), _rt::Errno>;
    fn xr_pose(
        &self,
        x: f64,
        y: f64,
        z: f64,
        qx: f64,
        qy: f64,
        qz: f64,
        qw: f64,
    ) -> Result<(), _rt::Errno>;
    fn xr_pose_end(&self) -> Result<(), _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Guest>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, tag_: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                POINTER => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let phase = r_
                        .case(0, 5)
                        .and_then(|v| PointerPhase::of(v).ok_or(_rt::Errno::EBADMSG))?;
                    let pointer_type = r_
                        .case(1, 3)
                        .and_then(|v| PointerType::of(v).ok_or(_rt::Errno::EBADMSG))?;
                    let id = r_.i32(4)?;
                    let button = r_.i32(8)?;
                    let x = r_.f64(16)?;
                    let y = r_.f64(24)?;
                    match self.0.pointer(phase, pointer_type, id, button, x, y) {
                        Ok(v) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                TYPING => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let typing = r_.bool(0)?;
                    match self.0.typing(typing) {
                        Ok(v) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                WHEEL => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let delta_y = r_.f64(0)?;
                    match self.0.wheel(delta_y) {
                        Ok(v) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                PINCH => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let scale = r_.f64(0)?;
                    match self.0.pinch(scale) {
                        Ok(v) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                MOVE => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let x = r_.i32(0)?;
                    let y = r_.i32(4)?;
                    match self.0.r#move(x, y) {
                        Ok(v) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                XR_POSE => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let x = r_.f64(0)?;
                    let y = r_.f64(8)?;
                    let z = r_.f64(16)?;
                    let qx = r_.f64(24)?;
                    let qy = r_.f64(32)?;
                    let qz = r_.f64(40)?;
                    let qw = r_.f64(48)?;
                    match self.0.xr_pose(x, y, z, qx, qy, qz, qw) {
                        Ok(v) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                XR_POSE_END => match self.0.xr_pose_end() {
                    Ok(v) => _rt::Reply::from(0),
                    Err(e) => _rt::Reply::from(e as i32),
                },
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
