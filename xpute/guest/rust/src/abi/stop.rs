// xpute-guest/abi/stop.rs
//
// GENERATED from wit/stop (xpute-link-gen wit.rs)

/// The interface's handle, named by `xpute:stop/stop` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x4eccaa;

/// `stopped`
pub const STOPPED: _rt::Method = 0;

pub fn stopped(errnum: i32, file: &str, line: u32, n1: u64, n2: u64) -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(
        HANDLE,
        STOPPED,
        Some((32, &|w_: &mut _rt::Lower| {
            w_.i32(0, errnum);
            w_.str(4, file);
            w_.u32(12, line);
            w_.u64(16, n1);
            w_.u64(24, n2);
        })),
    );
    _rt::answer(r)
}

mod _rt {
    pub use crate::link;
    pub use xpute_kit::status::errno::Errno;
    pub use xpute_link::{answer, Handle, Lower, Method};
}
