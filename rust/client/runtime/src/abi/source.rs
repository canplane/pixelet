// pxl-runtime/abi/source.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/source` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x6a88bf;

/// `declare`
pub const DECLARE: _rt::Method = 0;

pub fn declare(decl: &super::types::Declaration) -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(
        HANDLE,
        DECLARE,
        Some((28, &|w_: &mut _rt::Lower| {
            (decl).store(w_, 0);
        })),
    );
    _rt::answer(r)
}

mod _rt {
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{answer, Handle, Lower, Method};
    pub(super) use xpute_native_runtime::link;
}
