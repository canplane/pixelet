// xpute-gpu-guest/abi/vsync.rs
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `xpute:gpu/vsync` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x42eda6;

/// `next`
pub const NEXT: _rt::Method = 0;

pub fn next() -> impl _rt::Future<Output = Result<(), _rt::Errno>> {
    let sent = _rt::wait::wait(HANDLE, NEXT, None, ());
    async move {
        let (r, _, _) = sent.await;
        _rt::answer(r)
    }
}

mod _rt {
    pub(super) use core::future::Future;
    pub(super) use xpute_guest::wait;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{answer, Handle, Method};
}
