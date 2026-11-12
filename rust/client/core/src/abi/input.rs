// pxl-core/abi/input.rs
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
pub const URI: [&str; 2] = ["pxl:core", "input"];

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

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PointerPhase {
    #[default]
    Down = 0,
    Move = 1,
    Up = 2,
    Out = 3,
    Cancel = 4,
}

impl PointerPhase {
    pub const ALL: [PointerPhase; 5] = [
        PointerPhase::Down,
        PointerPhase::Move,
        PointerPhase::Up,
        PointerPhase::Out,
        PointerPhase::Cancel,
    ];

    pub fn of(v: u32) -> Option<PointerPhase> {
        PointerPhase::ALL.get(v as usize).copied()
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PointerType {
    #[default]
    Mouse = 0,
    Touch = 1,
    Pen = 2,
}

impl PointerType {
    pub const ALL: [PointerType; 3] = [PointerType::Mouse, PointerType::Touch, PointerType::Pen];

    pub fn of(v: u32) -> Option<PointerType> {
        PointerType::ALL.get(v as usize).copied()
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

/// Mounts `imp` at the URI, one object for every open.
pub fn mount(imp: Rc<dyn Input>) {
    let o: Rc<dyn Object> = Rc::new(InputObject(imp));
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

pub fn pointer(
    phase: PointerPhase,
    pointer_type: PointerType,
    id: i32,
    button: i32,
    x: f64,
    y: f64,
) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
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
    link::answer(r).map(|_| ())
}

pub fn typing(typing: bool) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        TYPING,
        Some((1, &|w: &mut PacketWriter| {
            w.bool(Some(typing));
        })),
    );
    link::answer(r).map(|_| ())
}

pub fn wheel(delta_y: f64) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        WHEEL,
        Some((1, &|w: &mut PacketWriter| {
            w.f64(Some(delta_y as f64));
        })),
    );
    link::answer(r).map(|_| ())
}

pub fn pinch(scale: f64) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        PINCH,
        Some((1, &|w: &mut PacketWriter| {
            w.f64(Some(scale as f64));
        })),
    );
    link::answer(r).map(|_| ())
}

pub fn r#move(x: i32, y: i32) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        MOVE,
        Some((2, &|w: &mut PacketWriter| {
            w.i32(Some(x as i32));
            w.i32(Some(y as i32));
        })),
    );
    link::answer(r).map(|_| ())
}

pub fn xr_pose(x: f64, y: f64, z: f64, qx: f64, qy: f64, qz: f64, qw: f64) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
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
    link::answer(r).map(|_| ())
}

pub fn xr_pose_end() -> Result<(), Errno> {
    let (r, data) = link::call(handle(), XR_POSE_END, None);
    link::answer(r).map(|_| ())
}
