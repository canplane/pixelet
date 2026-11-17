// pxl-core/abi/object.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/object` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x4edab0;

/// `apply`
pub const APPLY: _rt::Method = 0;

pub fn apply<L: _rt::Lend>(
    batch: L,
) -> impl Future<Output = (L, Result<Result<(), _rt::ErrorCode>, _rt::Errno>)> + use<L> {
    let mut batch = batch;
    let batch_lent_ = _rt::Lend::lent(&mut batch);
    let sent = _rt::wait::wait(
        HANDLE,
        APPLY,
        Some((16, &|w_: &mut _rt::Lower| {
            batch_lent_.store(w_, 0);
        })),
        batch,
    );
    async move {
        let (r, data, lent) = sent.await;
        (
            lent,
            _rt::answer(r).and_then(|_| {
                let r_ = _rt::Lift::new(data.as_deref().ok_or(_rt::Errno::EBADMSG)?);
                r_.case(0, 2).and_then(|d| {
                    if d == 0 {
                        Ok(Ok(()))
                    } else {
                        r_.case(1, 7)
                            .and_then(|v| _rt::ErrorCode::of(v).ok_or(_rt::Errno::EBADMSG))
                            .map(Err)
                    }
                })
            }),
        )
    }
}

mod _rt {
    pub(super) use xpute_guest::abi::fs::types::ErrorCode;
    pub(super) use xpute_guest::wait;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lend, Lift, Lower, Method, answer};
}
