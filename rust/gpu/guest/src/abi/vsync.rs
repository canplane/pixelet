// pxl-gpu-guest/abi/vsync.rs
//
// GENERATED from spec/gpu/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:gpu/vsync` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x51cb2a;

/// `next`
pub const NEXT: _rt::Method = 0;

pub fn next() -> impl Future<Output = Result<u64, _rt::Errno>> + use<> {
    let sent = _rt::wait::wait(HANDLE, NEXT, None, ());
    async move {
        let (r, data, _) = sent.await;
        _rt::answer(r).and_then(|_| {
            let r_ = _rt::Lift::new(data.as_deref().ok_or(_rt::Errno::EBADMSG)?);
            r_.u64(0)
        })
    }
}

mod _rt {
    pub(super) use xpute_guest::wait;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Method, answer};
}
