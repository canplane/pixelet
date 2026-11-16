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
        formats: Vec<u32>,
        shards: Vec<String>,
    ) -> Result<(), _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Guest>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: _rt::Lift, _: &mut _rt::Lower) -> i32 {
        let run = || -> Result<i32, _rt::Errno> {
            Ok(match m_ {
                DECLARE => {
                    let min_zoom = args_.u32(0)?;
                    let formats = args_.arr::<u32>(4)?;
                    let shards = args_.list(12, 8, 4).and_then(|(b, n)| {
                        (0..n)
                            .map(|i| args_.str(b + i * 8).map(str::to_owned))
                            .collect::<Result<Vec<_>, _rt::Errno>>()
                    })?;
                    match self.0.declare(min_zoom, formats, shards) {
                        Ok(()) => 0,
                        Err(e) => e as i32,
                    }
                }
                _ => _rt::Errno::ENOSYS as i32,
            })
        };
        run().unwrap_or_else(|e| e as i32)
    }
}

/// Mounts `implementation` under the interface's handle.
pub fn mount(implementation: _rt::Rc<dyn Guest>) {
    _rt::link::mount_named(HANDLE, _rt::Rc::new(_Object(implementation)));
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_guest::link;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Tag};
}
