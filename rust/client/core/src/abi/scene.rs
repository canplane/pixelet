// pxl-core/abi/scene.rs
//
// GENERATED from spec/client/core/wit (xpute-gen spec/wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use std::cell::Cell;
use std::rc::Rc;
use xpute_core::link::{self, Handle, Method, Object, Reply, Return, Tag};
use xpute_core::wait::wait;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

/// The singleton's URI.
pub const URI: [&str; 2] = ["pxl:core", "scene"];

/// `draw`
pub const DRAW: Method = 0;
/// `chat`
pub const CHAT: Method = 1;

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SceneContent {
    #[default]
    World = 0,
    Character = 1,
}

impl SceneContent {
    pub const ALL: [SceneContent; 2] = [SceneContent::World, SceneContent::Character];

    pub fn of(v: u32) -> Option<SceneContent> {
        SceneContent::ALL.get(v as usize).copied()
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
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                DRAW => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let surface = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let content = b.at(1).and_then(|c| {
                        c.get_number().and_then(|v| {
                            SceneContent::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                        })
                    })?;
                    let group = b.at(2).and_then(|c| {
                        c.opt().map(|c| c.get_number().map(|v| v as u8)).transpose()
                    })?;
                    let bg = b.at(3).and_then(|c| {
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
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let text = b.at(0).and_then(|c| c.get_str_ref().map(str::to_owned))?;
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

/// Mounts `imp` at the URI, one object for every open.
pub fn mount(imp: Rc<dyn Scene>) {
    let o: Rc<dyn Object> = Rc::new(SceneObject(imp));
    link::mount(link::uri(&URI), move |_, _| Ok(o.clone()));
}

thread_local! {
    static HANDLE: Cell<Option<Handle>> = const { Cell::new(None) };
}

/// Opens the singleton on the other side, once, at boot.
pub fn open() -> Result<(), Errno> {
    HANDLE.set(Some(link::open(&link::uri(&URI))?));
    Ok(())
}

/// The singleton's handle, which `open` opened.
pub fn handle() -> Handle {
    HANDLE.get().or_bug(Errno::EBADF)
}

pub fn draw(
    surface: u32,
    content: SceneContent,
    group: Option<u8>,
    bg: Option<u32>,
) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        DRAW,
        Some((4, &|w: &mut PacketWriter| {
            w.u32(Some(surface as u32));
            w.u32(Some(content as u32));
            match &group {
                Some(v) => {
                    w.u8(Some((*v)));
                }
                None => {
                    w.nil();
                }
            }
            match &bg {
                Some(v) => {
                    w.u32(Some((*v) as u32));
                }
                None => {
                    w.nil();
                }
            }
        })),
    );
    link::answer(r).map(|_| ())
}

pub fn chat(text: &str) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        CHAT,
        Some((1, &|w: &mut PacketWriter| {
            w.str(Some(&*text));
        })),
    );
    link::answer(r).map(|_| ())
}
