// pxl-core/abi/input.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/input` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x5ee0f8;

/// `pan`
pub const PAN: _rt::Method = 0;
/// `rotate`
pub const ROTATE: _rt::Method = 1;
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

pub trait Guest {
    fn pan(&self, dx: f64, dy: f64) -> Result<(), _rt::XputeError>;
    fn rotate(&self, rad: f64) -> Result<(), _rt::XputeError>;
    fn tilt(&self, rad: f64) -> Result<(), _rt::XputeError>;
    fn zoom(&self, scale: f64) -> Result<(), _rt::XputeError>;
    fn drag(&self, dragging: bool) -> Result<(), _rt::XputeError>;
    fn tap(&self, x: f64, y: f64) -> Result<(), _rt::XputeError>;
    fn hover(&self, x: f64, y: f64) -> Result<(), _rt::XputeError>;
    fn out(&self) -> Result<(), _rt::XputeError>;
    fn r#move(&self, x: i32, y: i32) -> Result<(), _rt::XputeError>;
}

struct _Object(_rt::Rc<dyn Guest>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: _rt::Lift, ret_: &mut _rt::Lower) -> i32 {
        let run = || -> Result<i32, _rt::XputeError> {
            Ok(match m_ {
                PAN => {
                    let dx = args_.f64(0)?;
                    let dy = args_.f64(8)?;
                    let () = self.0.pan(dx, dy)?;
                    0
                }
                ROTATE => {
                    let rad = args_.f64(0)?;
                    let () = self.0.rotate(rad)?;
                    0
                }
                TILT => {
                    let rad = args_.f64(0)?;
                    let () = self.0.tilt(rad)?;
                    0
                }
                ZOOM => {
                    let scale = args_.f64(0)?;
                    let () = self.0.zoom(scale)?;
                    0
                }
                DRAG => {
                    let dragging = args_.bool(0)?;
                    let () = self.0.drag(dragging)?;
                    0
                }
                TAP => {
                    let x = args_.f64(0)?;
                    let y = args_.f64(8)?;
                    let () = self.0.tap(x, y)?;
                    0
                }
                HOVER => {
                    let x = args_.f64(0)?;
                    let y = args_.f64(8)?;
                    let () = self.0.hover(x, y)?;
                    0
                }
                OUT => {
                    let () = self.0.out()?;
                    0
                }
                MOVE => {
                    let x = args_.i32(0)?;
                    let y = args_.i32(4)?;
                    let () = self.0.r#move(x, y)?;
                    0
                }
                _ => return Err(_rt::Errno::ENOSYS.into()),
            })
        };
        run().unwrap_or_else(|e| _rt::store_err(ret_, &e))
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
    pub(super) use xpute_kit::status::error::XputeError;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Tag, store_err};
}
