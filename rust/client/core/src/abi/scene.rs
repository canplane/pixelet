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
    fn chat(&self, text: String) -> Result<Result<(), super::types::ErrorCode>, _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Guest>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: _rt::Lift, ret_: &mut _rt::Lower) -> i32 {
        let mut run = || -> Result<i32, _rt::Errno> {
            Ok(match m_ {
                DRAW => {
                    let surface = args_.u32(0)?;
                    let content = args_
                        .case(4, 2)
                        .and_then(|v| SceneContent::of(v).ok_or(_rt::Errno::EBADMSG))?;
                    let group = args_.case(5, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            args_.u8(6).map(Some)
                        }
                    })?;
                    let bg = args_.case(8, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            args_.u32(12).map(Some)
                        }
                    })?;
                    match self.0.draw(surface, content, group, bg) {
                        Ok(v) => {
                            ret_.start(2);
                            match &v {
                                Ok(_) => {}
                                Err(v) => {
                                    ret_.u8(0, 1);
                                    ret_.u8(1, *v as u8);
                                }
                            }
                            0
                        }
                        Err(e) => e as i32,
                    }
                }
                CHAT => {
                    let text = args_.str(0).map(str::to_owned)?;
                    match self.0.chat(text) {
                        Ok(v) => {
                            ret_.start(2);
                            match &v {
                                Ok(_) => {}
                                Err(v) => {
                                    ret_.u8(0, 1);
                                    ret_.u8(1, *v as u8);
                                }
                            }
                            0
                        }
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
