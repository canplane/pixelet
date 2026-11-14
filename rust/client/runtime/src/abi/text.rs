// pxl-runtime/abi/text.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/text` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x61d944;

/// `width`
pub const WIDTH: _rt::Method = 0;
/// `raster`
pub const RASTER: _rt::Method = 1;

pub trait Host {
    fn width(&self, weight: u32, px: f64, text: _rt::Lent) -> Result<f64, _rt::Errno>;
    #[expect(
        clippy::too_many_arguments,
        reason = "the WIT function's parameters, each its own"
    )]
    fn raster(
        &self,
        weight: u32,
        px: f64,
        color: u32,
        image: _rt::Lent,
        width: u32,
        height: u32,
        x: f64,
        text: _rt::Lent,
    ) -> Result<u32, _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Host>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                WIDTH => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let weight = r_.u32(0)?;
                    let px = r_.f64(8)?;
                    let text = _rt::Lent::load(r_, 16)?;
                    match self.0.width(weight, px, text) {
                        Ok(v) => _rt::Reply {
                            result: 0,
                            data: Some((
                                8,
                                Box::new(move |w: &mut _rt::Lower| {
                                    w.f64(0, v);
                                }),
                            )),
                        },
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                RASTER => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let weight = r_.u32(0)?;
                    let px = r_.f64(8)?;
                    let color = r_.u32(16)?;
                    let image = _rt::Lent::load(r_, 20)?;
                    let width = r_.u32(28)?;
                    let height = r_.u32(32)?;
                    let x = r_.f64(40)?;
                    let text = _rt::Lent::load(r_, 48)?;
                    match self
                        .0
                        .raster(weight, px, color, image, width, height, x, text)
                    {
                        Ok(v) => _rt::Reply {
                            result: 0,
                            data: Some((
                                4,
                                Box::new(move |w: &mut _rt::Lower| {
                                    w.u32(0, v);
                                }),
                            )),
                        },
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                _ => _rt::Reply::from(_rt::Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| _rt::Reply::from(e as i32))
    }
}

/// Serves `implementation` under the interface's handle.
pub fn mount(implementation: _rt::Rc<dyn Host>) {
    _rt::link::mount_named(HANDLE, _rt::Rc::new(_Object(implementation)));
}

mod _rt {
    pub use std::rc::Rc;
    pub use xpute_kit::status::errno::Errno;
    pub use xpute_link::abi::types::Lent;
    pub use xpute_link::{Handle, Lift, Lower, Method, Object, Reply, Tag};
    pub use xpute_native_runtime::link;
}
