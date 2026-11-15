// pxl-gpu-host/abi/surface.rs
//
// GENERATED from spec/gpu/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:gpu/surface` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x76d49a;

/// `[method]surface.resize`
pub const SURFACE_RESIZE: _rt::Method = 0;

pub fn surface_resize(
    side_: &_rt::Side,
    surface_: _rt::Handle,
    width: u32,
    height: u32,
    dpr: f64,
) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(
        surface_,
        SURFACE_RESIZE,
        _rt::Packet::Args(Some((16, &|w_: &mut _rt::Lower| {
            w_.u32(0, width);
            w_.u32(4, height);
            w_.f64(8, dpr);
        }))),
    );
    _rt::answer(r)
}

mod _rt {
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lower, Method, Packet, Side, answer};
}
