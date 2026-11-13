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

use std::rc::Rc;
use xpute_guest::link;
use xpute_guest::wait;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `pxl:core/input` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x5ee0f8;

/// `pointer`
pub const POINTER: Method = 0;
/// `typing`
pub const TYPING: Method = 1;
/// `wheel`
pub const WHEEL: Method = 2;
/// `pinch`
pub const PINCH: Method = 3;
/// `move`
pub const MOVE: Method = 4;
/// `xr-pose`
pub const XR_POSE: Method = 5;
/// `xr-pose-end`
pub const XR_POSE_END: Method = 6;

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

pub trait Input {
    fn pointer(
        &self,
        phase: PointerPhase,
        pointer_type: PointerType,
        id: i32,
        button: i32,
        x: f64,
        y: f64,
    ) -> Result<(), Errno>;
    fn typing(&self, typing: bool) -> Result<(), Errno>;
    fn wheel(&self, delta_y: f64) -> Result<(), Errno>;
    fn pinch(&self, scale: f64) -> Result<(), Errno>;
    fn r#move(&self, x: i32, y: i32) -> Result<(), Errno>;
    fn xr_pose(
        &self,
        x: f64,
        y: f64,
        z: f64,
        qx: f64,
        qy: f64,
        qz: f64,
        qw: f64,
    ) -> Result<(), Errno>;
    fn xr_pose_end(&self) -> Result<(), Errno>;
}

struct InputObject(Rc<dyn Input>);

impl Object for InputObject {
    fn call(&self, m_: Method, tag_: Tag, args_: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m_ {
                POINTER => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let phase = b_.at(0).and_then(|c| {
                        c.get_number().and_then(|v| {
                            PointerPhase::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                        })
                    })?;
                    let pointer_type = b_.at(1).and_then(|c| {
                        c.get_number().and_then(|v| {
                            PointerType::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                        })
                    })?;
                    let id = b_.at(2).and_then(|c| c.get_number().map(|v| v as i32))?;
                    let button = b_.at(3).and_then(|c| c.get_number().map(|v| v as i32))?;
                    let x = b_.at(4).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let y = b_.at(5).and_then(|c| c.get_number().map(|v| v as f64))?;
                    match self.0.pointer(phase, pointer_type, id, button, x, y) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                TYPING => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let typing = b_.at(0).and_then(|c| c.get_bool())?;
                    match self.0.typing(typing) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                WHEEL => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let delta_y = b_.at(0).and_then(|c| c.get_number().map(|v| v as f64))?;
                    match self.0.wheel(delta_y) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                PINCH => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let scale = b_.at(0).and_then(|c| c.get_number().map(|v| v as f64))?;
                    match self.0.pinch(scale) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                MOVE => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let x = b_.at(0).and_then(|c| c.get_number().map(|v| v as i32))?;
                    let y = b_.at(1).and_then(|c| c.get_number().map(|v| v as i32))?;
                    match self.0.r#move(x, y) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                XR_POSE => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let x = b_.at(0).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let y = b_.at(1).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let z = b_.at(2).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let qx = b_.at(3).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let qy = b_.at(4).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let qz = b_.at(5).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let qw = b_.at(6).and_then(|c| c.get_number().map(|v| v as f64))?;
                    match self.0.xr_pose(x, y, z, qx, qy, qz, qw) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                XR_POSE_END => match self.0.xr_pose_end() {
                    Ok(v) => Reply::from(0),
                    Err(e) => Reply::from(e as i32),
                },
                _ => Reply::from(Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
    }
}

/// Serves `imp` under the interface's handle.
pub fn mount(imp: Rc<dyn Input>) {
    link::mount_named(HANDLE, Rc::new(InputObject(imp)));
}
