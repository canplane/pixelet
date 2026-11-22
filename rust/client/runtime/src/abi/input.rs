// pxl-runtime/abi/input.rs
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

/// The size of the fixed part of `pan`'s args.
pub const PAN_ARGS_SZ: usize = 16;

pub fn pan(side_: &_rt::Side, dx: f64, dy: f64) -> Result<(), _rt::XputeError> {
    let (r, data) = side_.call(
        HANDLE,
        PAN,
        _rt::Packet::Args(Some((16, &|w_: &mut _rt::Lower| {
            w_.f64(0, dx);
            w_.f64(8, dy);
        }))),
    );
    _rt::answer(r, data)
}

/// The size of the fixed part of `rotate`'s args.
pub const ROTATE_ARGS_SZ: usize = 8;

pub fn rotate(side_: &_rt::Side, rad: f64) -> Result<(), _rt::XputeError> {
    let (r, data) = side_.call(
        HANDLE,
        ROTATE,
        _rt::Packet::Args(Some((8, &|w_: &mut _rt::Lower| {
            w_.f64(0, rad);
        }))),
    );
    _rt::answer(r, data)
}

/// The size of the fixed part of `tilt`'s args.
pub const TILT_ARGS_SZ: usize = 8;

pub fn tilt(side_: &_rt::Side, rad: f64) -> Result<(), _rt::XputeError> {
    let (r, data) = side_.call(
        HANDLE,
        TILT,
        _rt::Packet::Args(Some((8, &|w_: &mut _rt::Lower| {
            w_.f64(0, rad);
        }))),
    );
    _rt::answer(r, data)
}

/// The size of the fixed part of `zoom`'s args.
pub const ZOOM_ARGS_SZ: usize = 8;

pub fn zoom(side_: &_rt::Side, scale: f64) -> Result<(), _rt::XputeError> {
    let (r, data) = side_.call(
        HANDLE,
        ZOOM,
        _rt::Packet::Args(Some((8, &|w_: &mut _rt::Lower| {
            w_.f64(0, scale);
        }))),
    );
    _rt::answer(r, data)
}

/// The size of the fixed part of `drag`'s args.
pub const DRAG_ARGS_SZ: usize = 1;

pub fn drag(side_: &_rt::Side, dragging: bool) -> Result<(), _rt::XputeError> {
    let (r, data) = side_.call(
        HANDLE,
        DRAG,
        _rt::Packet::Args(Some((1, &|w_: &mut _rt::Lower| {
            w_.bool(0, dragging);
        }))),
    );
    _rt::answer(r, data)
}

/// The size of the fixed part of `tap`'s args.
pub const TAP_ARGS_SZ: usize = 16;

pub fn tap(side_: &_rt::Side, x: f64, y: f64) -> Result<(), _rt::XputeError> {
    let (r, data) = side_.call(
        HANDLE,
        TAP,
        _rt::Packet::Args(Some((16, &|w_: &mut _rt::Lower| {
            w_.f64(0, x);
            w_.f64(8, y);
        }))),
    );
    _rt::answer(r, data)
}

/// The size of the fixed part of `hover`'s args.
pub const HOVER_ARGS_SZ: usize = 16;

pub fn hover(side_: &_rt::Side, x: f64, y: f64) -> Result<(), _rt::XputeError> {
    let (r, data) = side_.call(
        HANDLE,
        HOVER,
        _rt::Packet::Args(Some((16, &|w_: &mut _rt::Lower| {
            w_.f64(0, x);
            w_.f64(8, y);
        }))),
    );
    _rt::answer(r, data)
}

pub fn out(side_: &_rt::Side) -> Result<(), _rt::XputeError> {
    let (r, data) = side_.call(HANDLE, OUT, _rt::Packet::Args(None));
    _rt::answer(r, data)
}

/// The size of the fixed part of `move`'s args.
pub const MOVE_ARGS_SZ: usize = 8;

pub fn r#move(side_: &_rt::Side, x: i32, y: i32) -> Result<(), _rt::XputeError> {
    let (r, data) = side_.call(
        HANDLE,
        MOVE,
        _rt::Packet::Args(Some((8, &|w_: &mut _rt::Lower| {
            w_.i32(0, x);
            w_.i32(4, y);
        }))),
    );
    _rt::answer(r, data)
}

mod _rt {
    pub(super) use xpute_kit::status::error::XputeError;
    pub(super) use xpute_link::{Handle, Lower, Method, Packet, Side, answer};
}
