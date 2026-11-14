// pxl-core/abi/scene.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/scene` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x7c289a;

/// `draw`
pub const DRAW: _rt::Method = 0;
/// `chat`
pub const CHAT: _rt::Method = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum SceneContent {
    World = 0,
    Character = 1,
}

impl SceneContent {
    /// Every member, in its source's own order.
    pub const ALL: [SceneContent; 2] = [SceneContent::World, SceneContent::Character];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<SceneContent> {
        match val {
            0 => Some(SceneContent::World),
            1 => Some(SceneContent::Character),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            SceneContent::World => "World",
            SceneContent::Character => "Character",
        }
    }
}

pub trait Guest {
    fn draw(
        &self,
        surface: u32,
        content: SceneContent,
        group: Option<u8>,
        bg: Option<u32>,
    ) -> Result<Result<(), super::types::ErrorCode>, _rt::Errno>;
    fn chat(&self, text: String) -> Result<(), _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Guest>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                DRAW => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let surface = r_.u32(0)?;
                    let content = r_
                        .case(4, 2)
                        .and_then(|v| SceneContent::of(v).ok_or(_rt::Errno::EBADMSG))?;
                    let group = r_.case(5, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            r_.u8(6).map(Some)
                        }
                    })?;
                    let bg = r_.case(8, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            r_.u32(12).map(Some)
                        }
                    })?;
                    match self.0.draw(surface, content, group, bg) {
                        Ok(v) => _rt::Reply {
                            result: 0,
                            data: Some((
                                2,
                                Box::new(move |w: &mut _rt::Lower| match &v {
                                    Ok(_) => {}
                                    Err(v) => {
                                        w.u8(0, 1);
                                        w.u8(1, *v as u8);
                                    }
                                }),
                            )),
                        },
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                CHAT => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let text = r_.str(0).map(str::to_owned)?;
                    match self.0.chat(text) {
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
    pub use std::rc::Rc;
    pub use xpute_guest::link;
    pub use xpute_kit::status::errno::Errno;
    pub use xpute_link::{Handle, Lift, Lower, Method, Object, Reply, Tag};
}
