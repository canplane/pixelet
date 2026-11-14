// xpute-guest/abi/fs/file.rs
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

/// `[method]file.size`
pub const FILE_SIZE: _rt::Method = 0;
/// `[method]file.read`
pub const FILE_READ: _rt::Method = 1;
/// `[method]file.write`
pub const FILE_WRITE: _rt::Method = 2;
/// `[method]file.changed`
pub const FILE_CHANGED: _rt::Method = 3;
/// `[method]file.close`
pub const FILE_CLOSE: _rt::Method = 4;

pub fn file_size(file_: _rt::Handle) -> impl _rt::Future<Output = Result<u32, _rt::Errno>> {
    let sent = _rt::wait::wait(file_, FILE_SIZE, None, ());
    async move {
        let (r, data, lent) = sent.await;
        _rt::answer(r).and_then(|_| {
            let r_ = _rt::Lift::new(data.as_deref().ok_or(_rt::Errno::EBADMSG)?);
            r_.u32(0)
        })
    }
}

pub fn file_read<L: _rt::Lend>(
    file_: _rt::Handle,
    buf: L,
) -> impl _rt::Future<Output = (L, Result<u32, _rt::Errno>)> {
    let mut buf = buf;
    let (buf_at_, buf_len_) = _rt::Lend::span(&mut buf);
    let sent = _rt::wait::wait(
        file_,
        FILE_READ,
        Some((8, &|w_: &mut _rt::Lower| {
            _rt::Lent {
                at: buf_at_,
                len: buf_len_,
            }
            .store(w_, 0);
        })),
        buf,
    );
    async move {
        let (r, data, lent) = sent.await;
        (
            lent,
            _rt::answer(r).and_then(|_| {
                let r_ = _rt::Lift::new(data.as_deref().ok_or(_rt::Errno::EBADMSG)?);
                r_.u32(0)
            }),
        )
    }
}

pub fn file_write<L: _rt::Lend>(
    file_: _rt::Handle,
    buf: L,
) -> impl _rt::Future<Output = (L, Result<(), _rt::Errno>)> {
    let mut buf = buf;
    let (buf_at_, buf_len_) = _rt::Lend::span(&mut buf);
    let sent = _rt::wait::wait(
        file_,
        FILE_WRITE,
        Some((8, &|w_: &mut _rt::Lower| {
            _rt::Lent {
                at: buf_at_,
                len: buf_len_,
            }
            .store(w_, 0);
        })),
        buf,
    );
    async move {
        let (r, data, lent) = sent.await;
        (lent, _rt::answer(r))
    }
}

pub fn file_changed(file_: _rt::Handle) -> impl _rt::Future<Output = Result<(), _rt::Errno>> {
    let sent = _rt::wait::wait(file_, FILE_CHANGED, None, ());
    async move {
        let (r, data, lent) = sent.await;
        _rt::answer(r)
    }
}

pub fn file_close(file_: _rt::Handle) -> impl _rt::Future<Output = Result<(), _rt::Errno>> {
    let sent = _rt::wait::wait(file_, FILE_CLOSE, None, ());
    async move {
        let (r, data, lent) = sent.await;
        _rt::answer(r)
    }
}

mod _rt {
    pub use crate::wait;
    pub use core::future::Future;
    pub use xpute_kit::status::errno::Errno;
    pub use xpute_link::abi::types::Lent;
    pub use xpute_link::{answer, Handle, Lend, Lift, Lower, Method};
}
