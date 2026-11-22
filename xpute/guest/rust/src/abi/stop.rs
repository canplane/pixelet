// xpute-guest/abi/stop.rs
//
// GENERATED from wit/stop (xpute-link-gen wit.rs)

/// The interface's handle, named by `xpute:stop/stop` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x4eccaa;

/// `stopped`
pub const STOPPED: _rt::Method = 0;

/// The size of the fixed part of `stopped`'s args.
pub const STOPPED_ARGS_SZ: usize = 48;

pub fn stopped(
    errnum: i32,
    file: &str,
    line: u32,
    n1: u64,
    n2: u64,
    msg: Option<&str>,
) -> Result<(), _rt::XputeError> {
    let (r, data) = _rt::link::call(
        HANDLE,
        STOPPED,
        Some((48, &|w_: &mut _rt::Lower| {
            w_.i32(0, errnum);
            w_.str(4, file);
            w_.u32(12, line);
            w_.u64(16, n1);
            w_.u64(24, n2);
            if let Some(v) = &msg {
                w_.u8(32, 1);
                w_.str(36, v);
            }
        })),
    );
    _rt::answer(r, data)
}

mod _rt {
    pub(super) use crate::link;
    pub(super) use xpute_kit::status::error::XputeError;
    pub(super) use xpute_link::{Handle, Lower, Method, answer};
}
