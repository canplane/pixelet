// pxl-gpu-guest/abi/vsync.rs
//
// GENERATED from spec/gpu/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:gpu/vsync` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x51cb2a;

/// `next`
pub const NEXT: _rt::Method = 0;

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub at: u64,
    pub gap: u64,
}

impl Frame {
    pub const SZ: usize = 16;
    pub const ALIGN: usize = 8;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.u64(at, self.at);
        w.u64(at + 8, self.gap);
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<Frame, _rt::Errno> {
        Ok(Frame {
            at: r.u64(at)?,
            gap: r.u64(at + 8)?,
        })
    }
}

pub fn next(after: u64) -> impl Future<Output = Result<Frame, _rt::Errno>> + use<> {
    let sent = _rt::wait::wait(
        HANDLE,
        NEXT,
        Some((8, &|w_: &mut _rt::Lower| {
            w_.u64(0, after);
        })),
        (),
    );
    async move {
        let (r, data, _) = sent.await;
        _rt::answer(r).and_then(|_| {
            let r_ = _rt::Lift::new(data.as_deref().ok_or(_rt::Errno::EBADMSG)?);
            Frame::load(r_, 0)
        })
    }
}

mod _rt {
    pub(super) use xpute_guest::wait;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, answer};
}
