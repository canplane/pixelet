// xpute-gpu-native-host/abi/surface.rs
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

/// `[method]surface.resize`
pub const SURFACE_RESIZE: _rt::Method = 0;

pub fn surface_resize(
    surface_: _rt::Handle,
    width: u32,
    height: u32,
    dpr: f64,
) -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(
        surface_,
        SURFACE_RESIZE,
        Some((16, &|w_: &mut _rt::Lower| {
            w_.u32(0, width);
            w_.u32(4, height);
            w_.f64(8, dpr);
        })),
    );
    _rt::answer(r)
}

mod _rt {
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{answer, Handle, Lower, Method};
    pub(super) use xpute_native_runtime::link;
}
