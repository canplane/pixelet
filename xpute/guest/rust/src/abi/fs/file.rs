// xpute-guest/abi/fs/file.rs
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

/// The interface's handle, named by `xpute:fs/file` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x5b6743;

/// `[method]file.size`
pub const FILE_SIZE: _rt::Method = 0;
/// `[method]file.read`
pub const FILE_READ: _rt::Method = 1;
/// `[method]file.write`
pub const FILE_WRITE: _rt::Method = 2;
/// `[method]file.set-aside`
pub const FILE_SET_ASIDE: _rt::Method = 3;
/// `[method]file.close`
pub const FILE_CLOSE: _rt::Method = 4;

pub fn file_size(
    file_: _rt::Handle,
) -> impl _rt::Future<Output = Result<Result<u32, super::types::ErrorCode>, _rt::Errno>> {
    let sent = _rt::wait::wait(file_, FILE_SIZE, None, ());
    async move {
        let (r, data, _) = sent.await;
        _rt::answer(r).and_then(|_| {
            let r_ = _rt::Lift::new(data.as_deref().ok_or(_rt::Errno::EBADMSG)?);
            r_.case(0, 2).and_then(|d| {
                if d == 0 {
                    r_.u32(4).map(Ok)
                } else {
                    r_.case(4, 7)
                        .and_then(|v| super::types::ErrorCode::of(v).ok_or(_rt::Errno::EBADMSG))
                        .map(Err)
                }
            })
        })
    }
}

pub fn file_read<L: _rt::Lend>(
    file_: _rt::Handle,
    buf: L,
) -> impl _rt::Future<Output = (L, Result<Result<u32, super::types::ErrorCode>, _rt::Errno>)> {
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
                r_.case(0, 2).and_then(|d| {
                    if d == 0 {
                        r_.u32(4).map(Ok)
                    } else {
                        r_.case(4, 7)
                            .and_then(|v| super::types::ErrorCode::of(v).ok_or(_rt::Errno::EBADMSG))
                            .map(Err)
                    }
                })
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
        let (r, _, lent) = sent.await;
        (lent, _rt::answer(r))
    }
}

pub fn file_set_aside(
    file_: _rt::Handle,
) -> impl _rt::Future<Output = Result<Result<(), super::types::ErrorCode>, _rt::Errno>> {
    let sent = _rt::wait::wait(file_, FILE_SET_ASIDE, None, ());
    async move {
        let (r, data, _) = sent.await;
        _rt::answer(r).and_then(|_| {
            let r_ = _rt::Lift::new(data.as_deref().ok_or(_rt::Errno::EBADMSG)?);
            r_.case(0, 2).and_then(|d| {
                if d == 0 {
                    Ok(Ok(()))
                } else {
                    r_.case(1, 7)
                        .and_then(|v| super::types::ErrorCode::of(v).ok_or(_rt::Errno::EBADMSG))
                        .map(Err)
                }
            })
        })
    }
}

pub fn file_close(
    file_: _rt::Handle,
) -> impl _rt::Future<Output = Result<Result<(), super::types::ErrorCode>, _rt::Errno>> {
    let sent = _rt::wait::wait(file_, FILE_CLOSE, None, ());
    async move {
        let (r, data, _) = sent.await;
        _rt::answer(r).and_then(|_| {
            let r_ = _rt::Lift::new(data.as_deref().ok_or(_rt::Errno::EBADMSG)?);
            r_.case(0, 2).and_then(|d| {
                if d == 0 {
                    Ok(Ok(()))
                } else {
                    r_.case(1, 7)
                        .and_then(|v| super::types::ErrorCode::of(v).ok_or(_rt::Errno::EBADMSG))
                        .map(Err)
                }
            })
        })
    }
}

mod _rt {
    pub(super) use crate::wait;
    pub(super) use core::future::Future;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::abi::types::Lent;
    pub(super) use xpute_link::{answer, Handle, Lend, Lift, Lower, Method};
}
