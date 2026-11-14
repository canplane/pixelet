// xpute-gpu-guest/abi/vsync.rs
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

/// The interface's handle, named by `xpute:gpu/vsync` (frame.json `NAMED`).
pub const HANDLE: _rt::Handle = 0x42eda6;

/// `next`
pub const NEXT: _rt::Method = 0;

pub fn next() -> impl _rt::Future<Output = Result<(), _rt::Errno>> {
    let sent = _rt::wait::wait(HANDLE, NEXT, None, ());
    async move {
        let (r, data, lent) = sent.await;
        _rt::answer(r)
    }
}

mod _rt {
    pub use core::future::Future;
    pub use xpute_guest::wait;
    pub use xpute_kit::status::errno::Errno;
    pub use xpute_link::{answer, Handle, Method};
}
