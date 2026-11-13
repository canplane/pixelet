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
use xpute_guest::wait::wait;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `pxl:core/input` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x5ee0f8;

/// The interface's qualified name: its package and its name.
pub const NAME: [&str; 2] = ["pxl:core", "input"];

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
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                POINTER => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let phase = b.at(0).and_then(|c| {
                        c.get_number().and_then(|v| {
                            PointerPhase::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                        })
                    })?;
                    let pointer_type = b.at(1).and_then(|c| {
                        c.get_number().and_then(|v| {
                            PointerType::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                        })
                    })?;
                    let id = b.at(2).and_then(|c| c.get_number().map(|v| v as i32))?;
                    let button = b.at(3).and_then(|c| c.get_number().map(|v| v as i32))?;
                    let x = b.at(4).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let y = b.at(5).and_then(|c| c.get_number().map(|v| v as f64))?;
                    match self.0.pointer(phase, pointer_type, id, button, x, y) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                TYPING => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let typing = b.at(0).and_then(|c| c.get_bool())?;
                    match self.0.typing(typing) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                WHEEL => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let delta_y = b.at(0).and_then(|c| c.get_number().map(|v| v as f64))?;
                    match self.0.wheel(delta_y) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                PINCH => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let scale = b.at(0).and_then(|c| c.get_number().map(|v| v as f64))?;
                    match self.0.pinch(scale) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                MOVE => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let x = b.at(0).and_then(|c| c.get_number().map(|v| v as i32))?;
                    let y = b.at(1).and_then(|c| c.get_number().map(|v| v as i32))?;
                    match self.0.r#move(x, y) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                XR_POSE => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let x = b.at(0).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let y = b.at(1).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let z = b.at(2).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let qx = b.at(3).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let qy = b.at(4).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let qz = b.at(5).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let qw = b.at(6).and_then(|c| c.get_number().map(|v| v as f64))?;
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

pub fn pointer(
    phase: PointerPhase,
    pointer_type: PointerType,
    id: i32,
    button: i32,
    x: f64,
    y: f64,
) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        POINTER,
        Some((6, &|w: &mut PacketWriter| {
            w.u32(Some(phase as u32));
            w.u32(Some(pointer_type as u32));
            w.i32(Some(id as i32));
            w.i32(Some(button as i32));
            w.f64(Some(x as f64));
            w.f64(Some(y as f64));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}

pub fn typing(typing: bool) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        TYPING,
        Some((1, &|w: &mut PacketWriter| {
            w.bool(Some(typing));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}

pub fn wheel(delta_y: f64) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        WHEEL,
        Some((1, &|w: &mut PacketWriter| {
            w.f64(Some(delta_y as f64));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}

pub fn pinch(scale: f64) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        PINCH,
        Some((1, &|w: &mut PacketWriter| {
            w.f64(Some(scale as f64));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}

pub fn r#move(x: i32, y: i32) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        MOVE,
        Some((2, &|w: &mut PacketWriter| {
            w.i32(Some(x as i32));
            w.i32(Some(y as i32));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}

pub fn xr_pose(x: f64, y: f64, z: f64, qx: f64, qy: f64, qz: f64, qw: f64) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        XR_POSE,
        Some((7, &|w: &mut PacketWriter| {
            w.f64(Some(x as f64));
            w.f64(Some(y as f64));
            w.f64(Some(z as f64));
            w.f64(Some(qx as f64));
            w.f64(Some(qy as f64));
            w.f64(Some(qz as f64));
            w.f64(Some(qw as f64));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}

pub fn xr_pose_end() -> Result<(), Errno> {
    let (r, data) = link::call(HANDLE, XR_POSE_END, None);
    xpute_link::answer(r).map(|_| ())
}
