// xpute-core/abi.rs
//
// GENERATED from wit (xpute-gen spec/wit.rs)

pub mod clock {
    #![allow(
        unused_imports,
        unused_variables,
        dead_code,
        clippy::all,
        reason = "generated from WIT"
    )]
    use crate::link::{self, Done, Handle, Method, Object, Reply, Tag};
    use crate::wait::wait;
    use std::cell::Cell;
    use std::rc::Rc;
    use xpute_kit::status::bug::OrBug;
    use xpute_kit::status::errno::Errno;
    use xpute_kit::status::error::MarshalError;
    use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

    /// The singleton's URI.
    pub const URI: [&str; 2] = ["xpute:core", "clock"];

    /// `now`
    pub const NOW: Method = 0;

    pub trait Clock {
        fn now(&self, at: u32) -> Result<(), Errno>;
    }

    struct ClockObject(Rc<dyn Clock>);

    impl Object for ClockObject {
        fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
            let run = || -> Result<Reply, MarshalError> {
                Ok(match m {
                    NOW => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let at = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                        match self.0.now(at) {
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
    pub fn mount(imp: Rc<dyn Clock>) {
        let o: Rc<dyn Object> = Rc::new(ClockObject(imp));
        link::mount(link::uri(&URI), move |_| Ok(o.clone()));
    }

    thread_local! {
        static HANDLE: Cell<Option<Handle>> = const { Cell::new(None) };
    }

    /// Opens the singleton on the other side, once, at boot.
    pub fn open() -> Result<(), Errno> {
        HANDLE.set(Some(link::open(&link::uri(&URI))?));
        Ok(())
    }

    fn handle() -> Handle {
        HANDLE.get().or_bug(Errno::EBADF)
    }

    pub fn now(at: u32) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            NOW,
            Some((1, &|w: &mut PacketWriter| {
                w.u32(Some(at as u32));
            })),
        );
        link::answer(r).map(|_| ())
    }
}

pub mod fs {
    #![allow(
        unused_imports,
        unused_variables,
        dead_code,
        clippy::all,
        reason = "generated from WIT"
    )]
    use crate::link::{self, Done, Handle, Method, Object, Reply, Tag};
    use crate::wait::wait;
    use std::cell::Cell;
    use std::rc::Rc;
    use xpute_kit::status::bug::OrBug;
    use xpute_kit::status::errno::Errno;
    use xpute_kit::status::error::MarshalError;
    use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

    /// `[method]file.read`
    pub const FILE_READ: Method = 0;
    /// `[method]file.write`
    pub const FILE_WRITE: Method = 1;
    /// `[method]file.changed`
    pub const FILE_CHANGED: Method = 2;
    /// `[method]file.close`
    pub const FILE_CLOSE: Method = 3;

    pub trait File {
        fn read(&self, at: u32, len: u32, done: Done<u32>);
        fn write(&self, at: u32, len: u32, done: Done<()>);
        fn changed(&self, done: Done<()>);
        fn close(&self, done: Done<()>);
    }

    struct FileObject(Rc<dyn File>);

    impl Object for FileObject {
        fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
            let run = || -> Result<Reply, MarshalError> {
                Ok(match m {
                    FILE_READ => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let at = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                        let len = b.at(1).and_then(|c| c.get_number().map(|v| v as u32))?;
                        self.0.read(
                            at,
                            len,
                            Box::new(move |got| {
                                let reply = match got {
                                    Ok(v) => Reply::from(v as i32),
                                    Err(e) => Reply::from(e as i32),
                                };
                                link::complete(
                                    tag,
                                    reply.result,
                                    reply
                                        .data
                                        .as_ref()
                                        .map(|(n, w)| (*n, &**w as &dyn Fn(&mut PacketWriter))),
                                )
                            }),
                        );
                        Reply::from(Errno::EINPROGRESS as i32)
                    }
                    FILE_WRITE => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let at = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                        let len = b.at(1).and_then(|c| c.get_number().map(|v| v as u32))?;
                        self.0.write(
                            at,
                            len,
                            Box::new(move |got| {
                                let reply = match got {
                                    Ok(v) => Reply::from(0),
                                    Err(e) => Reply::from(e as i32),
                                };
                                link::complete(
                                    tag,
                                    reply.result,
                                    reply
                                        .data
                                        .as_ref()
                                        .map(|(n, w)| (*n, &**w as &dyn Fn(&mut PacketWriter))),
                                )
                            }),
                        );
                        Reply::from(Errno::EINPROGRESS as i32)
                    }
                    FILE_CHANGED => {
                        self.0.changed(Box::new(move |got| {
                            let reply = match got {
                                Ok(v) => Reply::from(0),
                                Err(e) => Reply::from(e as i32),
                            };
                            link::complete(
                                tag,
                                reply.result,
                                reply
                                    .data
                                    .as_ref()
                                    .map(|(n, w)| (*n, &**w as &dyn Fn(&mut PacketWriter))),
                            )
                        }));
                        Reply::from(Errno::EINPROGRESS as i32)
                    }
                    FILE_CLOSE => {
                        self.0.close(Box::new(move |got| {
                            let reply = match got {
                                Ok(v) => Reply::from(0),
                                Err(e) => Reply::from(e as i32),
                            };
                            link::complete(
                                tag,
                                reply.result,
                                reply
                                    .data
                                    .as_ref()
                                    .map(|(n, w)| (*n, &**w as &dyn Fn(&mut PacketWriter))),
                            )
                        }));
                        Reply::from(Errno::EINPROGRESS as i32)
                    }
                    _ => Reply::from(Errno::ENOSYS as i32),
                })
            };
            run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
        }
    }

    /// The object over `imp`, for an opener to answer.
    pub fn file(imp: Rc<dyn File>) -> Rc<dyn Object> {
        Rc::new(FileObject(imp))
    }

    pub fn file_read(
        file: Handle,
        at: u32,
        len: u32,
    ) -> impl core::future::Future<Output = Result<u32, Errno>> {
        let sent = wait(
            file,
            FILE_READ,
            Some((
                2,
                Box::new(move |w: &mut PacketWriter| {
                    w.u32(Some(at as u32));
                    w.u32(Some(len as u32));
                }),
            )),
            (),
        );
        async move {
            let (r, data) = sent.await;
            link::answer(r).map(|v| v as u32)
        }
    }

    pub fn file_write(
        file: Handle,
        at: u32,
        len: u32,
    ) -> impl core::future::Future<Output = Result<(), Errno>> {
        let sent = wait(
            file,
            FILE_WRITE,
            Some((
                2,
                Box::new(move |w: &mut PacketWriter| {
                    w.u32(Some(at as u32));
                    w.u32(Some(len as u32));
                }),
            )),
            (),
        );
        async move {
            let (r, data) = sent.await;
            link::answer(r).map(|_| ())
        }
    }

    pub fn file_changed(file: Handle) -> impl core::future::Future<Output = Result<(), Errno>> {
        let sent = wait(file, FILE_CHANGED, None, ());
        async move {
            let (r, data) = sent.await;
            link::answer(r).map(|_| ())
        }
    }

    pub fn file_close(file: Handle) -> impl core::future::Future<Output = Result<(), Errno>> {
        let sent = wait(file, FILE_CLOSE, None, ());
        async move {
            let (r, data) = sent.await;
            link::answer(r).map(|_| ())
        }
    }
}

pub mod life {
    #![allow(
        unused_imports,
        unused_variables,
        dead_code,
        clippy::all,
        reason = "generated from WIT"
    )]
    use crate::link::{self, Done, Handle, Method, Object, Reply, Tag};
    use crate::wait::wait;
    use std::cell::Cell;
    use std::rc::Rc;
    use xpute_kit::status::bug::OrBug;
    use xpute_kit::status::errno::Errno;
    use xpute_kit::status::error::MarshalError;
    use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

    /// The singleton's URI.
    pub const URI: [&str; 2] = ["xpute:core", "life"];

    /// `hidden`
    pub const HIDDEN: Method = 0;
    /// `shown`
    pub const SHOWN: Method = 1;

    pub trait Life {
        fn hidden(&self) -> Result<(), Errno>;
        fn shown(&self) -> Result<(), Errno>;
    }

    struct LifeObject(Rc<dyn Life>);

    impl Object for LifeObject {
        fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
            let run = || -> Result<Reply, MarshalError> {
                Ok(match m {
                    HIDDEN => match self.0.hidden() {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    },
                    SHOWN => match self.0.shown() {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    },
                    _ => Reply::from(Errno::ENOSYS as i32),
                })
            };
            run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
        }
    }

    /// Mounts `imp` at the URI, one object for every open.
    pub fn mount(imp: Rc<dyn Life>) {
        let o: Rc<dyn Object> = Rc::new(LifeObject(imp));
        link::mount(link::uri(&URI), move |_| Ok(o.clone()));
    }

    thread_local! {
        static HANDLE: Cell<Option<Handle>> = const { Cell::new(None) };
    }

    /// Opens the singleton on the other side, once, at boot.
    pub fn open() -> Result<(), Errno> {
        HANDLE.set(Some(link::open(&link::uri(&URI))?));
        Ok(())
    }

    fn handle() -> Handle {
        HANDLE.get().or_bug(Errno::EBADF)
    }

    pub fn hidden() -> Result<(), Errno> {
        let (r, data) = link::call(handle(), HIDDEN, None);
        link::answer(r).map(|_| ())
    }

    pub fn shown() -> Result<(), Errno> {
        let (r, data) = link::call(handle(), SHOWN, None);
        link::answer(r).map(|_| ())
    }
}

pub mod link {
    #![allow(
        unused_imports,
        unused_variables,
        dead_code,
        clippy::all,
        reason = "generated from WIT"
    )]
    use crate::link::{self, Done, Handle, Method, Object, Reply, Tag};
    use crate::wait::wait;
    use std::cell::Cell;
    use std::rc::Rc;
    use xpute_kit::status::bug::OrBug;
    use xpute_kit::status::errno::Errno;
    use xpute_kit::status::error::MarshalError;
    use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

    #[derive(Clone, Debug, PartialEq)]
    pub enum Token {
        Int(i32),
        Str(String),
    }

    impl Token {
        /// Writes its payload alone, whose node's type says its case.
        pub fn write(&self, w: &mut PacketWriter) {
            match self {
                Token::Int(v) => {
                    w.i32(Some((*v) as i32));
                }
                Token::Str(v) => {
                    w.str(Some(&*(*v)));
                }
            }
        }

        pub fn read(c: Cursor<'_>) -> Result<Token, MarshalError> {
            if let Ok(v) = c.get_number().map(|v| v as i32) {
                return Ok(Token::Int(v));
            }
            if let Ok(v) = c.get_str_ref().map(str::to_owned) {
                return Ok(Token::Str(v));
            }
            Err(MarshalError::new(Errno::EBADMSG))
        }
    }
}

pub mod timer {
    #![allow(
        unused_imports,
        unused_variables,
        dead_code,
        clippy::all,
        reason = "generated from WIT"
    )]
    use crate::link::{self, Done, Handle, Method, Object, Reply, Tag};
    use crate::wait::wait;
    use std::cell::Cell;
    use std::rc::Rc;
    use xpute_kit::status::bug::OrBug;
    use xpute_kit::status::errno::Errno;
    use xpute_kit::status::error::MarshalError;
    use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

    /// The singleton's URI.
    pub const URI: [&str; 2] = ["xpute:core", "timer"];

    /// `sleep`
    pub const SLEEP: Method = 0;

    pub trait Timer {
        fn sleep(&self, ms: u32, done: Done<()>);
    }

    struct TimerObject(Rc<dyn Timer>);

    impl Object for TimerObject {
        fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
            let run = || -> Result<Reply, MarshalError> {
                Ok(match m {
                    SLEEP => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let ms = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                        self.0.sleep(
                            ms,
                            Box::new(move |got| {
                                let reply = match got {
                                    Ok(v) => Reply::from(0),
                                    Err(e) => Reply::from(e as i32),
                                };
                                link::complete(
                                    tag,
                                    reply.result,
                                    reply
                                        .data
                                        .as_ref()
                                        .map(|(n, w)| (*n, &**w as &dyn Fn(&mut PacketWriter))),
                                )
                            }),
                        );
                        Reply::from(Errno::EINPROGRESS as i32)
                    }
                    _ => Reply::from(Errno::ENOSYS as i32),
                })
            };
            run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
        }
    }

    /// Mounts `imp` at the URI, one object for every open.
    pub fn mount(imp: Rc<dyn Timer>) {
        let o: Rc<dyn Object> = Rc::new(TimerObject(imp));
        link::mount(link::uri(&URI), move |_| Ok(o.clone()));
    }

    thread_local! {
        static HANDLE: Cell<Option<Handle>> = const { Cell::new(None) };
    }

    /// Opens the singleton on the other side, once, at boot.
    pub fn open() -> Result<(), Errno> {
        HANDLE.set(Some(link::open(&link::uri(&URI))?));
        Ok(())
    }

    fn handle() -> Handle {
        HANDLE.get().or_bug(Errno::EBADF)
    }

    pub fn sleep(ms: u32) -> impl core::future::Future<Output = Result<(), Errno>> {
        let sent = wait(
            handle(),
            SLEEP,
            Some((
                1,
                Box::new(move |w: &mut PacketWriter| {
                    w.u32(Some(ms as u32));
                }),
            )),
            (),
        );
        async move {
            let (r, data) = sent.await;
            link::answer(r).map(|_| ())
        }
    }
}
