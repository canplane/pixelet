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
    ) -> Result<(), _rt::Errno>;
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
                    let image = _rt::Lent::load(r_, 24)?;
                    let width = r_.u32(40)?;
                    let height = r_.u32(44)?;
                    let x = r_.f64(48)?;
                    let text = _rt::Lent::load(r_, 56)?;
                    match self
                        .0
                        .raster(weight, px, color, image, width, height, x, text)
                    {
                        Ok(()) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                _ => _rt::Reply::from(_rt::Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| _rt::Reply::from(e as i32))
    }
}

/// Serves `implementation` under the interface's handle on `side_`.
pub fn mount(side_: &'static _rt::Side, implementation: _rt::Rc<dyn Host>) {
    side_.mount_named(HANDLE, _rt::Rc::new(_Object(implementation)));
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::abi::types::Lent;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Reply, Side, Tag};
}
