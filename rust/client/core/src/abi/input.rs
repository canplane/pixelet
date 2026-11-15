// pxl-core/abi/input.rs
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

pub trait Guest {
    fn pointer(
        &self,
        phase: PointerPhase,
        touch: bool,
        id: i32,
        drag: Drag,
        x: f64,
        y: f64,
    ) -> Result<(), _rt::Errno>;
    fn wheel(&self, delta_y: f64) -> Result<(), _rt::Errno>;
    fn pinch(&self, scale: f64) -> Result<(), _rt::Errno>;
    fn r#move(&self, x: i32, y: i32) -> Result<(), _rt::Errno>;
    #[expect(
        clippy::too_many_arguments,
        reason = "the WIT function's parameters, each its own"
    )]
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
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                POINTER => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let phase = r_
                        .case(0, 5)
                        .and_then(|v| PointerPhase::of(v).ok_or(_rt::Errno::EBADMSG))?;
                    let touch = r_.bool(1)?;
                    let id = r_.i32(4)?;
                    let drag = r_
                        .case(8, 3)
                        .and_then(|v| Drag::of(v).ok_or(_rt::Errno::EBADMSG))?;
                    let x = r_.f64(16)?;
                    let y = r_.f64(24)?;
                    match self.0.pointer(phase, touch, id, drag, x, y) {
                        Ok(()) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                WHEEL => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let delta_y = r_.f64(0)?;
                    match self.0.wheel(delta_y) {
                        Ok(()) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                PINCH => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let scale = r_.f64(0)?;
                    match self.0.pinch(scale) {
                        Ok(()) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                MOVE => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let x = r_.i32(0)?;
                    let y = r_.i32(4)?;
                    match self.0.r#move(x, y) {
                        Ok(()) => _rt::Reply::from(0),
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
                        Ok(()) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                XR_POSE_END => match self.0.xr_pose_end() {
                    Ok(()) => _rt::Reply::from(0),
                    Err(e) => _rt::Reply::from(e as i32),
                },
                _ => _rt::Reply::from(_rt::Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| _rt::Reply::from(e as i32))
    }
}

/// Serves `implementation` under the interface's handle.
pub fn mount(implementation: _rt::Rc<dyn Guest>) {
    _rt::link::mount_named(HANDLE, _rt::Rc::new(_Object(implementation)));
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_guest::link;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Method, Object, Reply, Tag};
}
