// pxl-runtime/abi/source.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/source` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x6a88bf;

/// `declare`
pub const DECLARE: _rt::Method = 0;

pub fn declare(min_zoom: u32, format: &[u32], shard: &[String]) -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(
        HANDLE,
        DECLARE,
        Some((20, &|w_: &mut _rt::Lower| {
            w_.u32(0, min_zoom);
            w_.arr(4, format);
            if let Some(b) = w_.list(12, (shard).len(), 8, 4) {
                for (i, x) in (shard).iter().enumerate() {
                    w_.str(b + i * 8, x);
                }
            }
        })),
    );
    _rt::answer(r)
}

mod _rt {
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{answer, Handle, Lower, Method};
    pub(super) use xpute_native_runtime::link;
}
