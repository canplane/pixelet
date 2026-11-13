// pxl-core/abi/scene.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use std::rc::Rc;
use xpute_guest::link;
use xpute_guest::wait;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `pxl:core/scene` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x7c289a;

/// `draw`
pub const DRAW: Method = 0;
/// `chat`
pub const CHAT: Method = 1;

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

pub trait Scene {
    fn draw(
        &self,
        surface: u32,
        content: SceneContent,
        group: Option<u8>,
        bg: Option<u32>,
    ) -> Result<(), Errno>;
    fn chat(&self, text: String) -> Result<(), Errno>;
}

struct SceneObject(Rc<dyn Scene>);

impl Object for SceneObject {
    fn call(&self, m_: Method, tag_: Tag, args_: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m_ {
                DRAW => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let surface = b_.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let content = b_.at(1).and_then(|c| {
                        c.get_number().and_then(|v| {
                            SceneContent::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                        })
                    })?;
                    let group = b_.at(2).and_then(|c| {
                        c.opt().map(|c| c.get_number().map(|v| v as u8)).transpose()
                    })?;
                    let bg = b_.at(3).and_then(|c| {
                        c.opt()
                            .map(|c| c.get_number().map(|v| v as u32))
                            .transpose()
                    })?;
                    match self.0.draw(surface, content, group, bg) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                CHAT => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let text = b_.at(0).and_then(|c| c.get_str_ref().map(str::to_owned))?;
                    match self.0.chat(text) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                _ => Reply::from(Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
    }
}

/// Serves `imp` under the interface's handle.
pub fn mount(imp: Rc<dyn Scene>) {
    link::mount_named(HANDLE, Rc::new(SceneObject(imp)));
}
