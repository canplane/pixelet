// pxl-core/abi/input.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/input` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x5ee0f8;

/// `pan`
pub const PAN: _rt::Method = 0;
/// `turn`
pub const TURN: _rt::Method = 1;
/// `tilt`
pub const TILT: _rt::Method = 2;
/// `zoom`
pub const ZOOM: _rt::Method = 3;
/// `drag`
pub const DRAG: _rt::Method = 4;
/// `tap`
pub const TAP: _rt::Method = 5;
/// `hover`
pub const HOVER: _rt::Method = 6;
/// `out`
pub const OUT: _rt::Method = 7;
/// `move`
pub const MOVE: _rt::Method = 8;
/// `xr-pose`
pub const XR_POSE: _rt::Method = 9;
/// `xr-pose-end`
pub const XR_POSE_END: _rt::Method = 10;

pub trait Guest {
    fn pan(&self, dx: f64, dy: f64) -> Result<(), _rt::Errno>;
    fn turn(&self, rad: f64) -> Result<(), _rt::Errno>;
    fn tilt(&self, rad: f64) -> Result<(), _rt::Errno>;
    fn zoom(&self, scale: f64) -> Result<(), _rt::Errno>;
    fn drag(&self, dragging: bool) -> Result<(), _rt::Errno>;
    fn tap(&self, x: f64, y: f64) -> Result<(), _rt::Errno>;
    fn hover(&self, x: f64, y: f64) -> Result<(), _rt::Errno>;
    fn out(&self) -> Result<(), _rt::Errno>;
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
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: _rt::Lift, _: &mut _rt::Lower) -> i32 {
        let run = || -> Result<i32, _rt::Errno> {
            Ok(match m_ {
                PAN => {
                    let dx = args_.f64(0)?;
                    let dy = args_.f64(8)?;
                    match self.0.pan(dx, dy) {
                        Ok(()) => 0,
                        Err(e) => e as i32,
                    }
                }
                TURN => {
                    let rad = args_.f64(0)?;
                    match self.0.turn(rad) {
                        Ok(()) => 0,
                        Err(e) => e as i32,
                    }
                }
                TILT => {
                    let rad = args_.f64(0)?;
                    match self.0.tilt(rad) {
                        Ok(()) => 0,
                        Err(e) => e as i32,
                    }
                }
                ZOOM => {
                    let scale = args_.f64(0)?;
                    match self.0.zoom(scale) {
                        Ok(()) => 0,
                        Err(e) => e as i32,
                    }
                }
                DRAG => {
                    let dragging = args_.bool(0)?;
                    match self.0.drag(dragging) {
                        Ok(()) => 0,
                        Err(e) => e as i32,
                    }
                }
                TAP => {
                    let x = args_.f64(0)?;
                    let y = args_.f64(8)?;
                    match self.0.tap(x, y) {
                        Ok(()) => 0,
                        Err(e) => e as i32,
                    }
                }
                HOVER => {
                    let x = args_.f64(0)?;
                    let y = args_.f64(8)?;
                    match self.0.hover(x, y) {
                        Ok(()) => 0,
                        Err(e) => e as i32,
                    }
                }
                OUT => match self.0.out() {
                    Ok(()) => 0,
                    Err(e) => e as i32,
                },
                MOVE => {
                    let x = args_.i32(0)?;
                    let y = args_.i32(4)?;
                    match self.0.r#move(x, y) {
                        Ok(()) => 0,
                        Err(e) => e as i32,
                    }
                }
                XR_POSE => {
                    let x = args_.f64(0)?;
                    let y = args_.f64(8)?;
                    let z = args_.f64(16)?;
                    let qx = args_.f64(24)?;
                    let qy = args_.f64(32)?;
                    let qz = args_.f64(40)?;
                    let qw = args_.f64(48)?;
                    match self.0.xr_pose(x, y, z, qx, qy, qz, qw) {
                        Ok(()) => 0,
                        Err(e) => e as i32,
                    }
                }
                XR_POSE_END => match self.0.xr_pose_end() {
                    Ok(()) => 0,
                    Err(e) => e as i32,
                },
                _ => _rt::Errno::ENOSYS as i32,
            })
        };
        run().unwrap_or_else(|e| e as i32)
    }
}

/// Mounts `implementation` under the interface's handle.
pub fn mount(implementation: _rt::Rc<dyn Guest>) {
    _rt::link::mount_named(HANDLE, _rt::Rc::new(_Object(implementation)));
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_guest::link;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Tag};
}
