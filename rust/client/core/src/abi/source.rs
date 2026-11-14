// pxl-core/abi/source.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/source` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x6a88bf;

/// `declare`
pub const DECLARE: _rt::Method = 0;

pub trait Guest {
    fn declare(
        &self,
        min_zoom: u32,
        format: Vec<u32>,
        shard: Vec<String>,
    ) -> Result<(), _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Guest>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                DECLARE => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let min_zoom = r_.u32(0)?;
                    let format = r_.arr::<u32>(4)?;
                    let shard = r_.list(12, 8, 4).and_then(|(b, n)| {
                        (0..n)
                            .map(|i| r_.str(b + i * 8).map(str::to_owned))
                            .collect::<Result<Vec<_>, _rt::Errno>>()
                    })?;
                    match self.0.declare(min_zoom, format, shard) {
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

/// Serves `implementation` under the interface's handle.
pub fn mount(implementation: _rt::Rc<dyn Guest>) {
    _rt::link::mount_named(HANDLE, _rt::Rc::new(_Object(implementation)));
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_guest::link;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Method, Object, Reply, Tag};
}
