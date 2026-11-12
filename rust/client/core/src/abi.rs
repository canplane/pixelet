// pxl-core/abi.rs
//
// GENERATED from spec/client/core/wit (xpute-gen spec/wit.rs)

pub mod camera {
    #![allow(
        unused_imports,
        unused_variables,
        dead_code,
        clippy::all,
        reason = "generated from WIT"
    )]
    use std::cell::Cell;
    use std::rc::Rc;
    use xpute_core::link::{self, Done, Handle, Method, Object, Reply, Tag};
    use xpute_core::wait::wait;
    use xpute_kit::status::bug::OrBug;
    use xpute_kit::status::errno::Errno;
    use xpute_kit::status::error::MarshalError;
    use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

    /// The singleton's URI.
    pub const URI: [&str; 2] = ["pxl:core", "camera"];

    /// `track`
    pub const TRACK: Method = 0;
    /// `north-up`
    pub const NORTH_UP: Method = 1;
    /// `goto`
    pub const GOTO: Method = 2;

    #[repr(u32)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Tracking {
        Recenter = 0,
        Follow = 1,
        Unfollow = 2,
    }

    impl Tracking {
        pub const ALL: [Tracking; 3] = [Tracking::Recenter, Tracking::Follow, Tracking::Unfollow];

        pub fn of(v: u32) -> Option<Tracking> {
            Tracking::ALL.get(v as usize).copied()
        }
    }

    pub trait Camera {
        fn track(&self, tracking: Tracking) -> Result<(), Errno>;
        fn north_up(&self) -> Result<(), Errno>;
        fn goto(
            &self,
            lon: f64,
            lat: f64,
            dist: Option<f64>,
            azimuth: Option<f64>,
            polar: Option<f64>,
        ) -> Result<(), Errno>;
    }

    struct CameraObject(Rc<dyn Camera>);

    impl Object for CameraObject {
        fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
            let run = || -> Result<Reply, MarshalError> {
                Ok(match m {
                    TRACK => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let tracking = b.at(0).and_then(|c| {
                            c.get_number().and_then(|v| {
                                Tracking::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                            })
                        })?;
                        match self.0.track(tracking) {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    NORTH_UP => match self.0.north_up() {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    },
                    GOTO => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let lon = b.at(0).and_then(|c| c.get_number().map(|v| v as f64))?;
                        let lat = b.at(1).and_then(|c| c.get_number().map(|v| v as f64))?;
                        let dist = b.at(2).and_then(|c| {
                            c.opt()
                                .map(|c| c.get_number().map(|v| v as f64))
                                .transpose()
                        })?;
                        let azimuth = b.at(3).and_then(|c| {
                            c.opt()
                                .map(|c| c.get_number().map(|v| v as f64))
                                .transpose()
                        })?;
                        let polar = b.at(4).and_then(|c| {
                            c.opt()
                                .map(|c| c.get_number().map(|v| v as f64))
                                .transpose()
                        })?;
                        match self.0.goto(lon, lat, dist, azimuth, polar) {
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
    pub fn mount(imp: Rc<dyn Camera>) {
        let o: Rc<dyn Object> = Rc::new(CameraObject(imp));
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

    pub fn track(tracking: Tracking) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            TRACK,
            Some((1, &|w: &mut PacketWriter| {
                w.u32(Some(tracking as u32));
            })),
        );
        link::answer(r).map(|_| ())
    }

    pub fn north_up() -> Result<(), Errno> {
        let (r, data) = link::call(handle(), NORTH_UP, None);
        link::answer(r).map(|_| ())
    }

    pub fn goto(
        lon: f64,
        lat: f64,
        dist: Option<f64>,
        azimuth: Option<f64>,
        polar: Option<f64>,
    ) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            GOTO,
            Some((5, &|w: &mut PacketWriter| {
                w.f64(Some(lon as f64));
                w.f64(Some(lat as f64));
                match &dist {
                    Some(v) => {
                        w.f64(Some((*v) as f64));
                    }
                    None => {
                        w.nil();
                    }
                }
                match &azimuth {
                    Some(v) => {
                        w.f64(Some((*v) as f64));
                    }
                    None => {
                        w.nil();
                    }
                }
                match &polar {
                    Some(v) => {
                        w.f64(Some((*v) as f64));
                    }
                    None => {
                        w.nil();
                    }
                }
            })),
        );
        link::answer(r).map(|_| ())
    }
}

pub mod edit {
    #![allow(
        unused_imports,
        unused_variables,
        dead_code,
        clippy::all,
        reason = "generated from WIT"
    )]
    use std::cell::Cell;
    use std::rc::Rc;
    use xpute_core::link::{self, Done, Handle, Method, Object, Reply, Tag};
    use xpute_core::wait::wait;
    use xpute_kit::status::bug::OrBug;
    use xpute_kit::status::errno::Errno;
    use xpute_kit::status::error::MarshalError;
    use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

    /// The singleton's URI.
    pub const URI: [&str; 2] = ["pxl:core", "edit"];

    /// `hold`
    pub const HOLD: Method = 0;
    /// `marker-text`
    pub const MARKER_TEXT: Method = 1;
    /// `marker-cancel`
    pub const MARKER_CANCEL: Method = 2;
    /// `marker-commit`
    pub const MARKER_COMMIT: Method = 3;

    #[repr(u32)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Tool {
        Pointer = 0,
        Item = 1,
        Eraser = 2,
    }

    impl Tool {
        pub const ALL: [Tool; 3] = [Tool::Pointer, Tool::Item, Tool::Eraser];

        pub fn of(v: u32) -> Option<Tool> {
            Tool::ALL.get(v as usize).copied()
        }
    }

    pub trait Edit {
        fn hold(&self, tool: Tool, val: u32, scale_steps: i32) -> Result<(), Errno>;
        fn marker_text(&self, text: String) -> Result<(), Errno>;
        fn marker_cancel(&self) -> Result<(), Errno>;
        fn marker_commit(&self) -> Result<(), Errno>;
    }

    struct EditObject(Rc<dyn Edit>);

    impl Object for EditObject {
        fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
            let run = || -> Result<Reply, MarshalError> {
                Ok(match m {
                    HOLD => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let tool = b.at(0).and_then(|c| {
                            c.get_number().and_then(|v| {
                                Tool::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                            })
                        })?;
                        let val = b.at(1).and_then(|c| c.get_number().map(|v| v as u32))?;
                        let scale_steps = b.at(2).and_then(|c| c.get_number().map(|v| v as i32))?;
                        match self.0.hold(tool, val, scale_steps) {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    MARKER_TEXT => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let text = b.at(0).and_then(|c| c.get_str_ref().map(str::to_owned))?;
                        match self.0.marker_text(text) {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    MARKER_CANCEL => match self.0.marker_cancel() {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    },
                    MARKER_COMMIT => match self.0.marker_commit() {
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
    pub fn mount(imp: Rc<dyn Edit>) {
        let o: Rc<dyn Object> = Rc::new(EditObject(imp));
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

    pub fn hold(tool: Tool, val: u32, scale_steps: i32) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            HOLD,
            Some((3, &|w: &mut PacketWriter| {
                w.u32(Some(tool as u32));
                w.u32(Some(val as u32));
                w.i32(Some(scale_steps as i32));
            })),
        );
        link::answer(r).map(|_| ())
    }

    pub fn marker_text(text: &str) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            MARKER_TEXT,
            Some((1, &|w: &mut PacketWriter| {
                w.str(Some(&*text));
            })),
        );
        link::answer(r).map(|_| ())
    }

    pub fn marker_cancel() -> Result<(), Errno> {
        let (r, data) = link::call(handle(), MARKER_CANCEL, None);
        link::answer(r).map(|_| ())
    }

    pub fn marker_commit() -> Result<(), Errno> {
        let (r, data) = link::call(handle(), MARKER_COMMIT, None);
        link::answer(r).map(|_| ())
    }
}

pub mod input {
    #![allow(
        unused_imports,
        unused_variables,
        dead_code,
        clippy::all,
        reason = "generated from WIT"
    )]
    use std::cell::Cell;
    use std::rc::Rc;
    use xpute_core::link::{self, Done, Handle, Method, Object, Reply, Tag};
    use xpute_core::wait::wait;
    use xpute_kit::status::bug::OrBug;
    use xpute_kit::status::errno::Errno;
    use xpute_kit::status::error::MarshalError;
    use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

    /// The singleton's URI.
    pub const URI: [&str; 2] = ["pxl:core", "input"];

    /// `pointer`
    pub const POINTER: Method = 0;
    /// `typing`
    pub const TYPING: Method = 1;
    /// `wheel`
    pub const WHEEL: Method = 2;
    /// `pinch`
    pub const PINCH: Method = 3;
    /// `move`
    pub const MOVE: Method = 4;
    /// `xr-pose`
    pub const XR_POSE: Method = 5;
    /// `xr-pose-end`
    pub const XR_POSE_END: Method = 6;

    #[repr(u32)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum PointerPhase {
        Down = 0,
        Move = 1,
        Up = 2,
        Out = 3,
        Cancel = 4,
    }

    impl PointerPhase {
        pub const ALL: [PointerPhase; 5] = [
            PointerPhase::Down,
            PointerPhase::Move,
            PointerPhase::Up,
            PointerPhase::Out,
            PointerPhase::Cancel,
        ];

        pub fn of(v: u32) -> Option<PointerPhase> {
            PointerPhase::ALL.get(v as usize).copied()
        }
    }

    #[repr(u32)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum PointerType {
        Mouse = 0,
        Touch = 1,
        Pen = 2,
    }

    impl PointerType {
        pub const ALL: [PointerType; 3] =
            [PointerType::Mouse, PointerType::Touch, PointerType::Pen];

        pub fn of(v: u32) -> Option<PointerType> {
            PointerType::ALL.get(v as usize).copied()
        }
    }

    pub trait Input {
        fn pointer(
            &self,
            phase: PointerPhase,
            pointer_type: PointerType,
            id: i32,
            button: i32,
            x: f64,
            y: f64,
        ) -> Result<(), Errno>;
        fn typing(&self, typing: bool) -> Result<(), Errno>;
        fn wheel(&self, delta_y: f64) -> Result<(), Errno>;
        fn pinch(&self, scale: f64) -> Result<(), Errno>;
        fn r#move(&self, x: i32, y: i32) -> Result<(), Errno>;
        fn xr_pose(
            &self,
            x: f64,
            y: f64,
            z: f64,
            qx: f64,
            qy: f64,
            qz: f64,
            qw: f64,
        ) -> Result<(), Errno>;
        fn xr_pose_end(&self) -> Result<(), Errno>;
    }

    struct InputObject(Rc<dyn Input>);

    impl Object for InputObject {
        fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
            let run = || -> Result<Reply, MarshalError> {
                Ok(match m {
                    POINTER => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let phase = b.at(0).and_then(|c| {
                            c.get_number().and_then(|v| {
                                PointerPhase::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                            })
                        })?;
                        let pointer_type = b.at(1).and_then(|c| {
                            c.get_number().and_then(|v| {
                                PointerType::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                            })
                        })?;
                        let id = b.at(2).and_then(|c| c.get_number().map(|v| v as i32))?;
                        let button = b.at(3).and_then(|c| c.get_number().map(|v| v as i32))?;
                        let x = b.at(4).and_then(|c| c.get_number().map(|v| v as f64))?;
                        let y = b.at(5).and_then(|c| c.get_number().map(|v| v as f64))?;
                        match self.0.pointer(phase, pointer_type, id, button, x, y) {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    TYPING => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let typing = b.at(0).and_then(|c| c.get_bool())?;
                        match self.0.typing(typing) {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    WHEEL => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let delta_y = b.at(0).and_then(|c| c.get_number().map(|v| v as f64))?;
                        match self.0.wheel(delta_y) {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    PINCH => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let scale = b.at(0).and_then(|c| c.get_number().map(|v| v as f64))?;
                        match self.0.pinch(scale) {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    MOVE => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let x = b.at(0).and_then(|c| c.get_number().map(|v| v as i32))?;
                        let y = b.at(1).and_then(|c| c.get_number().map(|v| v as i32))?;
                        match self.0.r#move(x, y) {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    XR_POSE => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let x = b.at(0).and_then(|c| c.get_number().map(|v| v as f64))?;
                        let y = b.at(1).and_then(|c| c.get_number().map(|v| v as f64))?;
                        let z = b.at(2).and_then(|c| c.get_number().map(|v| v as f64))?;
                        let qx = b.at(3).and_then(|c| c.get_number().map(|v| v as f64))?;
                        let qy = b.at(4).and_then(|c| c.get_number().map(|v| v as f64))?;
                        let qz = b.at(5).and_then(|c| c.get_number().map(|v| v as f64))?;
                        let qw = b.at(6).and_then(|c| c.get_number().map(|v| v as f64))?;
                        match self.0.xr_pose(x, y, z, qx, qy, qz, qw) {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    XR_POSE_END => match self.0.xr_pose_end() {
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
    pub fn mount(imp: Rc<dyn Input>) {
        let o: Rc<dyn Object> = Rc::new(InputObject(imp));
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

    pub fn pointer(
        phase: PointerPhase,
        pointer_type: PointerType,
        id: i32,
        button: i32,
        x: f64,
        y: f64,
    ) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            POINTER,
            Some((6, &|w: &mut PacketWriter| {
                w.u32(Some(phase as u32));
                w.u32(Some(pointer_type as u32));
                w.i32(Some(id as i32));
                w.i32(Some(button as i32));
                w.f64(Some(x as f64));
                w.f64(Some(y as f64));
            })),
        );
        link::answer(r).map(|_| ())
    }

    pub fn typing(typing: bool) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            TYPING,
            Some((1, &|w: &mut PacketWriter| {
                w.bool(Some(typing));
            })),
        );
        link::answer(r).map(|_| ())
    }

    pub fn wheel(delta_y: f64) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            WHEEL,
            Some((1, &|w: &mut PacketWriter| {
                w.f64(Some(delta_y as f64));
            })),
        );
        link::answer(r).map(|_| ())
    }

    pub fn pinch(scale: f64) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            PINCH,
            Some((1, &|w: &mut PacketWriter| {
                w.f64(Some(scale as f64));
            })),
        );
        link::answer(r).map(|_| ())
    }

    pub fn r#move(x: i32, y: i32) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            MOVE,
            Some((2, &|w: &mut PacketWriter| {
                w.i32(Some(x as i32));
                w.i32(Some(y as i32));
            })),
        );
        link::answer(r).map(|_| ())
    }

    pub fn xr_pose(
        x: f64,
        y: f64,
        z: f64,
        qx: f64,
        qy: f64,
        qz: f64,
        qw: f64,
    ) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            XR_POSE,
            Some((7, &|w: &mut PacketWriter| {
                w.f64(Some(x as f64));
                w.f64(Some(y as f64));
                w.f64(Some(z as f64));
                w.f64(Some(qx as f64));
                w.f64(Some(qy as f64));
                w.f64(Some(qz as f64));
                w.f64(Some(qw as f64));
            })),
        );
        link::answer(r).map(|_| ())
    }

    pub fn xr_pose_end() -> Result<(), Errno> {
        let (r, data) = link::call(handle(), XR_POSE_END, None);
        link::answer(r).map(|_| ())
    }
}

pub mod perf {
    #![allow(
        unused_imports,
        unused_variables,
        dead_code,
        clippy::all,
        reason = "generated from WIT"
    )]
    use std::cell::Cell;
    use std::rc::Rc;
    use xpute_core::link::{self, Done, Handle, Method, Object, Reply, Tag};
    use xpute_core::wait::wait;
    use xpute_kit::status::bug::OrBug;
    use xpute_kit::status::errno::Errno;
    use xpute_kit::status::error::MarshalError;
    use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

    /// The singleton's URI.
    pub const URI: [&str; 2] = ["pxl:core", "perf"];

    /// `start`
    pub const START: Method = 0;
    /// `stop`
    pub const STOP: Method = 1;
    /// `stats`
    pub const STATS: Method = 2;
    /// `rings`
    pub const RINGS: Method = 3;
    /// `strtab`
    pub const STRTAB: Method = 4;

    pub trait Perf {
        fn start(&self) -> Result<(), Errno>;
        fn stop(&self) -> Result<(), Errno>;
        fn stats(&self, window_ms: Option<f64>) -> Result<Vec<u8>, Errno>;
        fn rings(&self, ring: u32) -> Result<Vec<u8>, Errno>;
        fn strtab(&self) -> Result<Vec<u8>, Errno>;
    }

    struct PerfObject(Rc<dyn Perf>);

    impl Object for PerfObject {
        fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
            let run = || -> Result<Reply, MarshalError> {
                Ok(match m {
                    START => match self.0.start() {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    },
                    STOP => match self.0.stop() {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    },
                    STATS => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let window_ms = b.at(0).and_then(|c| {
                            c.opt()
                                .map(|c| c.get_number().map(|v| v as f64))
                                .transpose()
                        })?;
                        match self.0.stats(window_ms) {
                            Ok(v) => Reply {
                                result: 0,
                                data: Some((
                                    1,
                                    Box::new(move |w: &mut PacketWriter| {
                                        w.array::<u8>(Some(&*v));
                                    }),
                                )),
                            },
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    RINGS => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let ring = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                        match self.0.rings(ring) {
                            Ok(v) => Reply {
                                result: 0,
                                data: Some((
                                    1,
                                    Box::new(move |w: &mut PacketWriter| {
                                        w.array::<u8>(Some(&*v));
                                    }),
                                )),
                            },
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    STRTAB => match self.0.strtab() {
                        Ok(v) => Reply {
                            result: 0,
                            data: Some((
                                1,
                                Box::new(move |w: &mut PacketWriter| {
                                    w.array::<u8>(Some(&*v));
                                }),
                            )),
                        },
                        Err(e) => Reply::from(e as i32),
                    },
                    _ => Reply::from(Errno::ENOSYS as i32),
                })
            };
            run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
        }
    }

    /// Mounts `imp` at the URI, one object for every open.
    pub fn mount(imp: Rc<dyn Perf>) {
        let o: Rc<dyn Object> = Rc::new(PerfObject(imp));
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

    pub fn start() -> Result<(), Errno> {
        let (r, data) = link::call(handle(), START, None);
        link::answer(r).map(|_| ())
    }

    pub fn stop() -> Result<(), Errno> {
        let (r, data) = link::call(handle(), STOP, None);
        link::answer(r).map(|_| ())
    }

    pub fn stats(window_ms: Option<f64>) -> Result<Vec<u8>, Errno> {
        let (r, data) = link::call(
            handle(),
            STATS,
            Some((1, &|w: &mut PacketWriter| match &window_ms {
                Some(v) => {
                    w.f64(Some((*v) as f64));
                }
                None => {
                    w.nil();
                }
            })),
        );
        link::answer(r).and_then(|_| {
            let p = data.as_deref().ok_or(Errno::EBADMSG)?;
            TreeReader::new(p)
                .and_then(|t| t.read_branch())
                .and_then(|b| b.at(0))
                .and_then(|c| c.get_array::<u8>().map(<[u8]>::to_vec))
                .map_err(|e| e.errnum)
        })
    }

    pub fn rings(ring: u32) -> Result<Vec<u8>, Errno> {
        let (r, data) = link::call(
            handle(),
            RINGS,
            Some((1, &|w: &mut PacketWriter| {
                w.u32(Some(ring as u32));
            })),
        );
        link::answer(r).and_then(|_| {
            let p = data.as_deref().ok_or(Errno::EBADMSG)?;
            TreeReader::new(p)
                .and_then(|t| t.read_branch())
                .and_then(|b| b.at(0))
                .and_then(|c| c.get_array::<u8>().map(<[u8]>::to_vec))
                .map_err(|e| e.errnum)
        })
    }

    pub fn strtab() -> Result<Vec<u8>, Errno> {
        let (r, data) = link::call(handle(), STRTAB, None);
        link::answer(r).and_then(|_| {
            let p = data.as_deref().ok_or(Errno::EBADMSG)?;
            TreeReader::new(p)
                .and_then(|t| t.read_branch())
                .and_then(|b| b.at(0))
                .and_then(|c| c.get_array::<u8>().map(<[u8]>::to_vec))
                .map_err(|e| e.errnum)
        })
    }
}

pub mod scene {
    #![allow(
        unused_imports,
        unused_variables,
        dead_code,
        clippy::all,
        reason = "generated from WIT"
    )]
    use std::cell::Cell;
    use std::rc::Rc;
    use xpute_core::link::{self, Done, Handle, Method, Object, Reply, Tag};
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
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum SceneContent {
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
}

pub mod setting {
    #![allow(
        unused_imports,
        unused_variables,
        dead_code,
        clippy::all,
        reason = "generated from WIT"
    )]
    use std::cell::Cell;
    use std::rc::Rc;
    use xpute_core::link::{self, Done, Handle, Method, Object, Reply, Tag};
    use xpute_core::wait::wait;
    use xpute_kit::status::bug::OrBug;
    use xpute_kit::status::errno::Errno;
    use xpute_kit::status::error::MarshalError;
    use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

    /// The singleton's URI.
    pub const URI: [&str; 2] = ["pxl:core", "setting"];

    /// `dev`
    pub const DEV: Method = 0;
    /// `set`
    pub const SET: Method = 1;

    pub trait Setting {
        fn dev(&self, dev: bool) -> Result<(), Errno>;
        fn set(&self, key: u32, val: u32) -> Result<(), Errno>;
    }

    struct SettingObject(Rc<dyn Setting>);

    impl Object for SettingObject {
        fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
            let run = || -> Result<Reply, MarshalError> {
                Ok(match m {
                    DEV => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let dev = b.at(0).and_then(|c| c.get_bool())?;
                        match self.0.dev(dev) {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    SET => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let key = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                        let val = b.at(1).and_then(|c| c.get_number().map(|v| v as u32))?;
                        match self.0.set(key, val) {
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
    pub fn mount(imp: Rc<dyn Setting>) {
        let o: Rc<dyn Object> = Rc::new(SettingObject(imp));
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

    pub fn dev(dev: bool) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            DEV,
            Some((1, &|w: &mut PacketWriter| {
                w.bool(Some(dev));
            })),
        );
        link::answer(r).map(|_| ())
    }

    pub fn set(key: u32, val: u32) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            SET,
            Some((2, &|w: &mut PacketWriter| {
                w.u32(Some(key as u32));
                w.u32(Some(val as u32));
            })),
        );
        link::answer(r).map(|_| ())
    }
}

pub mod source {
    #![allow(
        unused_imports,
        unused_variables,
        dead_code,
        clippy::all,
        reason = "generated from WIT"
    )]
    use std::cell::Cell;
    use std::rc::Rc;
    use xpute_core::link::{self, Done, Handle, Method, Object, Reply, Tag};
    use xpute_core::wait::wait;
    use xpute_kit::status::bug::OrBug;
    use xpute_kit::status::errno::Errno;
    use xpute_kit::status::error::MarshalError;
    use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

    /// The singleton's URI.
    pub const URI: [&str; 2] = ["pxl:core", "source"];

    /// `declare`
    pub const DECLARE: Method = 0;

    pub trait Source {
        fn declare(&self, min_zoom: u32, format: Vec<u32>, shard: Vec<String>)
            -> Result<(), Errno>;
    }

    struct SourceObject(Rc<dyn Source>);

    impl Object for SourceObject {
        fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
            let run = || -> Result<Reply, MarshalError> {
                Ok(match m {
                    DECLARE => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let min_zoom = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                        let format = b.at(1).and_then(|c| {
                            c.as_branch().and_then(|b| {
                                (0..b.len)
                                    .map(|i| b.at(i).and_then(|c| c.get_number().map(|v| v as u32)))
                                    .collect::<Result<Vec<_>, MarshalError>>()
                            })
                        })?;
                        let shard = b.at(2).and_then(|c| {
                            c.as_branch().and_then(|b| {
                                (0..b.len)
                                    .map(|i| {
                                        b.at(i).and_then(|c| c.get_str_ref().map(str::to_owned))
                                    })
                                    .collect::<Result<Vec<_>, MarshalError>>()
                            })
                        })?;
                        match self.0.declare(min_zoom, format, shard) {
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
    pub fn mount(imp: Rc<dyn Source>) {
        let o: Rc<dyn Object> = Rc::new(SourceObject(imp));
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

    pub fn declare(min_zoom: u32, format: &[u32], shard: &[String]) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            DECLARE,
            Some((3, &|w: &mut PacketWriter| {
                w.u32(Some(min_zoom as u32));
                w.branch(format.len() as u32, |w| {
                    for x in format.iter() {
                        w.u32(Some((*x) as u32));
                    }
                });
                w.branch(shard.len() as u32, |w| {
                    for x in shard.iter() {
                        w.str(Some(&*(*x)));
                    }
                });
            })),
        );
        link::answer(r).map(|_| ())
    }
}

pub mod text {
    #![allow(
        unused_imports,
        unused_variables,
        dead_code,
        clippy::all,
        reason = "generated from WIT"
    )]
    use std::cell::Cell;
    use std::rc::Rc;
    use xpute_core::link::{self, Done, Handle, Method, Object, Reply, Tag};
    use xpute_core::wait::wait;
    use xpute_kit::status::bug::OrBug;
    use xpute_kit::status::errno::Errno;
    use xpute_kit::status::error::MarshalError;
    use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

    /// The singleton's URI.
    pub const URI: [&str; 2] = ["pxl:core", "text"];

    /// `layout`
    pub const LAYOUT: Method = 0;
    /// `raster`
    pub const RASTER: Method = 1;

    pub trait Text {
        fn layout(
            &self,
            weight: u32,
            px: f64,
            max_width: f64,
            at: u32,
            lines: u32,
            text_at: u32,
            text_len: u32,
        ) -> Result<u32, Errno>;
        fn raster(
            &self,
            weight: u32,
            px: f64,
            color: u32,
            at: u32,
            width: u32,
            height: u32,
            x: f64,
            text_at: u32,
            text_len: u32,
        ) -> Result<u32, Errno>;
    }

    struct TextObject(Rc<dyn Text>);

    impl Object for TextObject {
        fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
            let run = || -> Result<Reply, MarshalError> {
                Ok(match m {
                    LAYOUT => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let weight = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                        let px = b.at(1).and_then(|c| c.get_number().map(|v| v as f64))?;
                        let max_width = b.at(2).and_then(|c| c.get_number().map(|v| v as f64))?;
                        let at = b.at(3).and_then(|c| c.get_number().map(|v| v as u32))?;
                        let lines = b.at(4).and_then(|c| c.get_number().map(|v| v as u32))?;
                        let text_at = b.at(5).and_then(|c| c.get_number().map(|v| v as u32))?;
                        let text_len = b.at(6).and_then(|c| c.get_number().map(|v| v as u32))?;
                        match self
                            .0
                            .layout(weight, px, max_width, at, lines, text_at, text_len)
                        {
                            Ok(v) => Reply::from(v as i32),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    RASTER => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let weight = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                        let px = b.at(1).and_then(|c| c.get_number().map(|v| v as f64))?;
                        let color = b.at(2).and_then(|c| c.get_number().map(|v| v as u32))?;
                        let at = b.at(3).and_then(|c| c.get_number().map(|v| v as u32))?;
                        let width = b.at(4).and_then(|c| c.get_number().map(|v| v as u32))?;
                        let height = b.at(5).and_then(|c| c.get_number().map(|v| v as u32))?;
                        let x = b.at(6).and_then(|c| c.get_number().map(|v| v as f64))?;
                        let text_at = b.at(7).and_then(|c| c.get_number().map(|v| v as u32))?;
                        let text_len = b.at(8).and_then(|c| c.get_number().map(|v| v as u32))?;
                        match self
                            .0
                            .raster(weight, px, color, at, width, height, x, text_at, text_len)
                        {
                            Ok(v) => Reply::from(v as i32),
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
    pub fn mount(imp: Rc<dyn Text>) {
        let o: Rc<dyn Object> = Rc::new(TextObject(imp));
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

    pub fn layout(
        weight: u32,
        px: f64,
        max_width: f64,
        at: u32,
        lines: u32,
        text_at: u32,
        text_len: u32,
    ) -> Result<u32, Errno> {
        let (r, data) = link::call(
            handle(),
            LAYOUT,
            Some((7, &|w: &mut PacketWriter| {
                w.u32(Some(weight as u32));
                w.f64(Some(px as f64));
                w.f64(Some(max_width as f64));
                w.u32(Some(at as u32));
                w.u32(Some(lines as u32));
                w.u32(Some(text_at as u32));
                w.u32(Some(text_len as u32));
            })),
        );
        link::answer(r).map(|v| v as u32)
    }

    pub fn raster(
        weight: u32,
        px: f64,
        color: u32,
        at: u32,
        width: u32,
        height: u32,
        x: f64,
        text_at: u32,
        text_len: u32,
    ) -> Result<u32, Errno> {
        let (r, data) = link::call(
            handle(),
            RASTER,
            Some((9, &|w: &mut PacketWriter| {
                w.u32(Some(weight as u32));
                w.f64(Some(px as f64));
                w.u32(Some(color as u32));
                w.u32(Some(at as u32));
                w.u32(Some(width as u32));
                w.u32(Some(height as u32));
                w.f64(Some(x as f64));
                w.u32(Some(text_at as u32));
                w.u32(Some(text_len as u32));
            })),
        );
        link::answer(r).map(|v| v as u32)
    }
}

pub mod types {
    #![allow(
        unused_imports,
        unused_variables,
        dead_code,
        clippy::all,
        reason = "generated from WIT"
    )]
    use std::cell::Cell;
    use std::rc::Rc;
    use xpute_core::link::{self, Done, Handle, Method, Object, Reply, Tag};
    use xpute_core::wait::wait;
    use xpute_kit::status::bug::OrBug;
    use xpute_kit::status::errno::Errno;
    use xpute_kit::status::error::MarshalError;
    use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

    #[repr(u32)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Sensor {
        Geolocation = 0,
        Orientation = 1,
    }

    impl Sensor {
        pub const ALL: [Sensor; 2] = [Sensor::Geolocation, Sensor::Orientation];

        pub fn of(v: u32) -> Option<Sensor> {
            Sensor::ALL.get(v as usize).copied()
        }
    }
}

pub mod sensor {
    #![allow(
        unused_imports,
        unused_variables,
        dead_code,
        clippy::all,
        reason = "generated from WIT"
    )]
    use std::cell::Cell;
    use std::rc::Rc;
    use xpute_core::link::{self, Done, Handle, Method, Object, Reply, Tag};
    use xpute_core::wait::wait;
    use xpute_kit::status::bug::OrBug;
    use xpute_kit::status::errno::Errno;
    use xpute_kit::status::error::MarshalError;
    use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

    /// The singleton's URI.
    pub const URI: [&str; 2] = ["pxl:core", "sensor"];

    /// `pos`
    pub const POS: Method = 0;
    /// `orient`
    pub const ORIENT: Method = 1;
    /// `err`
    pub const ERR: Method = 2;
    /// `permission`
    pub const PERMISSION: Method = 3;
    /// `platform`
    pub const PLATFORM: Method = 4;
    /// `locate`
    pub const LOCATE: Method = 5;

    #[repr(u32)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum GeolocationErrorCode {
        Other = 0,
        PermissionDenied = 1,
        PositionUnavailable = 2,
        Timeout = 3,
    }

    impl GeolocationErrorCode {
        pub const ALL: [GeolocationErrorCode; 4] = [
            GeolocationErrorCode::Other,
            GeolocationErrorCode::PermissionDenied,
            GeolocationErrorCode::PositionUnavailable,
            GeolocationErrorCode::Timeout,
        ];

        pub fn of(v: u32) -> Option<GeolocationErrorCode> {
            GeolocationErrorCode::ALL.get(v as usize).copied()
        }
    }

    #[repr(u32)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum PermissionAnswer {
        Asking = 0,
        Granted = 1,
        Denied = 2,
        Failed = 3,
    }

    impl PermissionAnswer {
        pub const ALL: [PermissionAnswer; 4] = [
            PermissionAnswer::Asking,
            PermissionAnswer::Granted,
            PermissionAnswer::Denied,
            PermissionAnswer::Failed,
        ];

        pub fn of(v: u32) -> Option<PermissionAnswer> {
            PermissionAnswer::ALL.get(v as usize).copied()
        }
    }

    pub trait Sensor {
        fn pos(
            &self,
            lon: f64,
            lat: f64,
            alt: Option<f64>,
            accuracy: Option<f64>,
            course: Option<f64>,
        ) -> Result<(), Errno>;
        fn orient(
            &self,
            webkit_compass_heading: Option<f64>,
            alpha: Option<f64>,
            beta: Option<f64>,
            gamma: Option<f64>,
        ) -> Result<(), Errno>;
        fn err(
            &self,
            sensor: super::types::Sensor,
            code: Option<GeolocationErrorCode>,
            watching: bool,
        ) -> Result<(), Errno>;
        fn permission(
            &self,
            sensor: super::types::Sensor,
            answer: PermissionAnswer,
        ) -> Result<(), Errno>;
        fn platform(
            &self,
            geolocation: bool,
            orient: bool,
            orient_permission: bool,
            orient_silence_is_loss: bool,
        ) -> Result<(), Errno>;
        fn locate(&self) -> Result<(), Errno>;
    }

    struct SensorObject(Rc<dyn Sensor>);

    impl Object for SensorObject {
        fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
            let run = || -> Result<Reply, MarshalError> {
                Ok(match m {
                    POS => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let lon = b.at(0).and_then(|c| c.get_number().map(|v| v as f64))?;
                        let lat = b.at(1).and_then(|c| c.get_number().map(|v| v as f64))?;
                        let alt = b.at(2).and_then(|c| {
                            c.opt()
                                .map(|c| c.get_number().map(|v| v as f64))
                                .transpose()
                        })?;
                        let accuracy = b.at(3).and_then(|c| {
                            c.opt()
                                .map(|c| c.get_number().map(|v| v as f64))
                                .transpose()
                        })?;
                        let course = b.at(4).and_then(|c| {
                            c.opt()
                                .map(|c| c.get_number().map(|v| v as f64))
                                .transpose()
                        })?;
                        match self.0.pos(lon, lat, alt, accuracy, course) {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    ORIENT => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let webkit_compass_heading = b.at(0).and_then(|c| {
                            c.opt()
                                .map(|c| c.get_number().map(|v| v as f64))
                                .transpose()
                        })?;
                        let alpha = b.at(1).and_then(|c| {
                            c.opt()
                                .map(|c| c.get_number().map(|v| v as f64))
                                .transpose()
                        })?;
                        let beta = b.at(2).and_then(|c| {
                            c.opt()
                                .map(|c| c.get_number().map(|v| v as f64))
                                .transpose()
                        })?;
                        let gamma = b.at(3).and_then(|c| {
                            c.opt()
                                .map(|c| c.get_number().map(|v| v as f64))
                                .transpose()
                        })?;
                        match self.0.orient(webkit_compass_heading, alpha, beta, gamma) {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    ERR => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let sensor = b.at(0).and_then(|c| {
                            c.get_number().and_then(|v| {
                                super::types::Sensor::of(v as u32)
                                    .ok_or(MarshalError::new(Errno::EBADMSG))
                            })
                        })?;
                        let code = b.at(1).and_then(|c| {
                            c.opt()
                                .map(|c| {
                                    c.get_number().and_then(|v| {
                                        GeolocationErrorCode::of(v as u32)
                                            .ok_or(MarshalError::new(Errno::EBADMSG))
                                    })
                                })
                                .transpose()
                        })?;
                        let watching = b.at(2).and_then(|c| c.get_bool())?;
                        match self.0.err(sensor, code, watching) {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    PERMISSION => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let sensor = b.at(0).and_then(|c| {
                            c.get_number().and_then(|v| {
                                super::types::Sensor::of(v as u32)
                                    .ok_or(MarshalError::new(Errno::EBADMSG))
                            })
                        })?;
                        let answer = b.at(1).and_then(|c| {
                            c.get_number().and_then(|v| {
                                PermissionAnswer::of(v as u32)
                                    .ok_or(MarshalError::new(Errno::EBADMSG))
                            })
                        })?;
                        match self.0.permission(sensor, answer) {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    PLATFORM => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let geolocation = b.at(0).and_then(|c| c.get_bool())?;
                        let orient = b.at(1).and_then(|c| c.get_bool())?;
                        let orient_permission = b.at(2).and_then(|c| c.get_bool())?;
                        let orient_silence_is_loss = b.at(3).and_then(|c| c.get_bool())?;
                        match self.0.platform(
                            geolocation,
                            orient,
                            orient_permission,
                            orient_silence_is_loss,
                        ) {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    LOCATE => match self.0.locate() {
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
    pub fn mount(imp: Rc<dyn Sensor>) {
        let o: Rc<dyn Object> = Rc::new(SensorObject(imp));
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

    pub fn pos(
        lon: f64,
        lat: f64,
        alt: Option<f64>,
        accuracy: Option<f64>,
        course: Option<f64>,
    ) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            POS,
            Some((5, &|w: &mut PacketWriter| {
                w.f64(Some(lon as f64));
                w.f64(Some(lat as f64));
                match &alt {
                    Some(v) => {
                        w.f64(Some((*v) as f64));
                    }
                    None => {
                        w.nil();
                    }
                }
                match &accuracy {
                    Some(v) => {
                        w.f64(Some((*v) as f64));
                    }
                    None => {
                        w.nil();
                    }
                }
                match &course {
                    Some(v) => {
                        w.f64(Some((*v) as f64));
                    }
                    None => {
                        w.nil();
                    }
                }
            })),
        );
        link::answer(r).map(|_| ())
    }

    pub fn orient(
        webkit_compass_heading: Option<f64>,
        alpha: Option<f64>,
        beta: Option<f64>,
        gamma: Option<f64>,
    ) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            ORIENT,
            Some((4, &|w: &mut PacketWriter| {
                match &webkit_compass_heading {
                    Some(v) => {
                        w.f64(Some((*v) as f64));
                    }
                    None => {
                        w.nil();
                    }
                }
                match &alpha {
                    Some(v) => {
                        w.f64(Some((*v) as f64));
                    }
                    None => {
                        w.nil();
                    }
                }
                match &beta {
                    Some(v) => {
                        w.f64(Some((*v) as f64));
                    }
                    None => {
                        w.nil();
                    }
                }
                match &gamma {
                    Some(v) => {
                        w.f64(Some((*v) as f64));
                    }
                    None => {
                        w.nil();
                    }
                }
            })),
        );
        link::answer(r).map(|_| ())
    }

    pub fn err(
        sensor: super::types::Sensor,
        code: Option<GeolocationErrorCode>,
        watching: bool,
    ) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            ERR,
            Some((3, &|w: &mut PacketWriter| {
                w.u32(Some(sensor as u32));
                match &code {
                    Some(v) => {
                        w.u32(Some((*v) as u32));
                    }
                    None => {
                        w.nil();
                    }
                }
                w.bool(Some(watching));
            })),
        );
        link::answer(r).map(|_| ())
    }

    pub fn permission(sensor: super::types::Sensor, answer: PermissionAnswer) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            PERMISSION,
            Some((2, &|w: &mut PacketWriter| {
                w.u32(Some(sensor as u32));
                w.u32(Some(answer as u32));
            })),
        );
        link::answer(r).map(|_| ())
    }

    pub fn platform(
        geolocation: bool,
        orient: bool,
        orient_permission: bool,
        orient_silence_is_loss: bool,
    ) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            PLATFORM,
            Some((4, &|w: &mut PacketWriter| {
                w.bool(Some(geolocation));
                w.bool(Some(orient));
                w.bool(Some(orient_permission));
                w.bool(Some(orient_silence_is_loss));
            })),
        );
        link::answer(r).map(|_| ())
    }

    pub fn locate() -> Result<(), Errno> {
        let (r, data) = link::call(handle(), LOCATE, None);
        link::answer(r).map(|_| ())
    }
}

pub mod ui {
    #![allow(
        unused_imports,
        unused_variables,
        dead_code,
        clippy::all,
        reason = "generated from WIT"
    )]
    use std::cell::Cell;
    use std::rc::Rc;
    use xpute_core::link::{self, Done, Handle, Method, Object, Reply, Tag};
    use xpute_core::wait::wait;
    use xpute_kit::status::bug::OrBug;
    use xpute_kit::status::errno::Errno;
    use xpute_kit::status::error::MarshalError;
    use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

    /// The singleton's URI.
    pub const URI: [&str; 2] = ["pxl:core", "ui"];

    /// `sensor-start`
    pub const SENSOR_START: Method = 0;
    /// `sensor-stop`
    pub const SENSOR_STOP: Method = 1;
    /// `show`
    pub const SHOW: Method = 2;
    /// `view`
    pub const VIEW: Method = 3;

    #[repr(u32)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum GroundStatus {
        Idle = 0,
        Loading = 1,
        Success = 2,
    }

    impl GroundStatus {
        pub const ALL: [GroundStatus; 3] = [
            GroundStatus::Idle,
            GroundStatus::Loading,
            GroundStatus::Success,
        ];

        pub fn of(v: u32) -> Option<GroundStatus> {
            GroundStatus::ALL.get(v as usize).copied()
        }
    }

    #[repr(u32)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum SensorStatus {
        Idle = 0,
        Loading = 1,
        Ready = 2,
        Error = 3,
        Unavailable = 4,
    }

    impl SensorStatus {
        pub const ALL: [SensorStatus; 5] = [
            SensorStatus::Idle,
            SensorStatus::Loading,
            SensorStatus::Ready,
            SensorStatus::Error,
            SensorStatus::Unavailable,
        ];

        pub fn of(v: u32) -> Option<SensorStatus> {
            SensorStatus::ALL.get(v as usize).copied()
        }
    }

    #[repr(u32)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Message {
        GeolocationUnsupported = 0,
        GeolocationPermissionDenied = 1,
        GeolocationPositionUnavailable = 2,
        GeolocationTimeout = 3,
        GeolocationGenericError = 4,
        WorldReadFailing = 5,
        WorldUnavailable = 6,
    }

    impl Message {
        pub const ALL: [Message; 7] = [
            Message::GeolocationUnsupported,
            Message::GeolocationPermissionDenied,
            Message::GeolocationPositionUnavailable,
            Message::GeolocationTimeout,
            Message::GeolocationGenericError,
            Message::WorldReadFailing,
            Message::WorldUnavailable,
        ];

        pub fn of(v: u32) -> Option<Message> {
            Message::ALL.get(v as usize).copied()
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct LonLat {
        pub lon: f64,
        pub lat: f64,
    }

    impl LonLat {
        pub fn write(&self, w: &mut PacketWriter) {
            w.branch(2, |w| {
                w.f64(Some(self.lon as f64));
                w.f64(Some(self.lat as f64));
            });
        }

        pub fn read(c: Cursor<'_>) -> Result<LonLat, MarshalError> {
            let b = c.as_branch()?;
            Ok(LonLat {
                lon: b.at(0).and_then(|c| c.get_number().map(|v| v as f64))?,
                lat: b.at(1).and_then(|c| c.get_number().map(|v| v as f64))?,
            })
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct PivotState {
        pub status: GroundStatus,
        pub is_off_center: bool,
        pub lon_lat: Option<LonLat>,
        pub outside: bool,
    }

    impl PivotState {
        pub fn write(&self, w: &mut PacketWriter) {
            w.branch(4, |w| {
                w.u32(Some(self.status as u32));
                w.bool(Some(self.is_off_center));
                match &self.lon_lat {
                    Some(v) => {
                        (*v).write(w);
                    }
                    None => {
                        w.nil();
                    }
                }
                w.bool(Some(self.outside));
            });
        }

        pub fn read(c: Cursor<'_>) -> Result<PivotState, MarshalError> {
            let b = c.as_branch()?;
            Ok(PivotState {
                status: b.at(0).and_then(|c| {
                    c.get_number().and_then(|v| {
                        GroundStatus::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                    })
                })?,
                is_off_center: b.at(1).and_then(|c| c.get_bool())?,
                lon_lat: b
                    .at(2)
                    .and_then(|c| c.opt().map(|c| LonLat::read(c)).transpose())?,
                outside: b.at(3).and_then(|c| c.get_bool())?,
            })
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct LocationState {
        pub status: SensorStatus,
        pub has_location: bool,
        pub watching: bool,
        pub err: Option<Message>,
    }

    impl LocationState {
        pub fn write(&self, w: &mut PacketWriter) {
            w.branch(4, |w| {
                w.u32(Some(self.status as u32));
                w.bool(Some(self.has_location));
                w.bool(Some(self.watching));
                match &self.err {
                    Some(v) => {
                        w.u32(Some((*v) as u32));
                    }
                    None => {
                        w.nil();
                    }
                }
            });
        }

        pub fn read(c: Cursor<'_>) -> Result<LocationState, MarshalError> {
            let b = c.as_branch()?;
            Ok(LocationState {
                status: b.at(0).and_then(|c| {
                    c.get_number().and_then(|v| {
                        SensorStatus::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                    })
                })?,
                has_location: b.at(1).and_then(|c| c.get_bool())?,
                watching: b.at(2).and_then(|c| c.get_bool())?,
                err: b.at(3).and_then(|c| {
                    c.opt()
                        .map(|c| {
                            c.get_number().and_then(|v| {
                                Message::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                            })
                        })
                        .transpose()
                })?,
            })
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct CompanionState {
        pub person_ready: bool,
        pub pet_ready: bool,
    }

    impl CompanionState {
        pub fn write(&self, w: &mut PacketWriter) {
            w.branch(2, |w| {
                w.bool(Some(self.person_ready));
                w.bool(Some(self.pet_ready));
            });
        }

        pub fn read(c: Cursor<'_>) -> Result<CompanionState, MarshalError> {
            let b = c.as_branch()?;
            Ok(CompanionState {
                person_ready: b.at(0).and_then(|c| c.get_bool())?,
                pet_ready: b.at(1).and_then(|c| c.get_bool())?,
            })
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct OrientationState {
        pub status: SensorStatus,
    }

    impl OrientationState {
        pub fn write(&self, w: &mut PacketWriter) {
            w.branch(1, |w| {
                w.u32(Some(self.status as u32));
            });
        }

        pub fn read(c: Cursor<'_>) -> Result<OrientationState, MarshalError> {
            let b = c.as_branch()?;
            Ok(OrientationState {
                status: b.at(0).and_then(|c| {
                    c.get_number().and_then(|v| {
                        SensorStatus::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                    })
                })?,
            })
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct DivisionPart {
        pub subtype: u8,
        pub name: String,
        pub shown: bool,
        pub area: bool,
    }

    impl DivisionPart {
        pub fn write(&self, w: &mut PacketWriter) {
            w.branch(4, |w| {
                w.u8(Some(self.subtype));
                w.str(Some(&*self.name));
                w.bool(Some(self.shown));
                w.bool(Some(self.area));
            });
        }

        pub fn read(c: Cursor<'_>) -> Result<DivisionPart, MarshalError> {
            let b = c.as_branch()?;
            Ok(DivisionPart {
                subtype: b.at(0).and_then(|c| c.get_number().map(|v| v as u8))?,
                name: b.at(1).and_then(|c| c.get_str_ref().map(str::to_owned))?,
                shown: b.at(2).and_then(|c| c.get_bool())?,
                area: b.at(3).and_then(|c| c.get_bool())?,
            })
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct RefusedFile {
        pub path: Vec<xpute_core::abi::link::Token>,
    }

    impl RefusedFile {
        pub fn write(&self, w: &mut PacketWriter) {
            w.branch(1, |w| {
                w.branch(self.path.len() as u32, |w| {
                    for x in self.path.iter() {
                        (*x).write(w);
                    }
                });
            });
        }

        pub fn read(c: Cursor<'_>) -> Result<RefusedFile, MarshalError> {
            let b = c.as_branch()?;
            Ok(RefusedFile {
                path: b.at(0).and_then(|c| {
                    c.as_branch().and_then(|b| {
                        (0..b.len)
                            .map(|i| b.at(i).and_then(|c| xpute_core::abi::link::Token::read(c)))
                            .collect::<Result<Vec<_>, MarshalError>>()
                    })
                })?,
            })
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct Snapshot {
        pub pivot: PivotState,
        pub location: LocationState,
        pub companion: CompanionState,
        pub draft: Option<String>,
        pub orient: OrientationState,
        pub is_follow_mode: bool,
        pub camera_busy: bool,
        pub zooming: bool,
        pub over_marker: bool,
        pub settings: Vec<u32>,
        pub division: Vec<DivisionPart>,
        pub character_surfaces: Vec<u32>,
        pub refused_files: Vec<RefusedFile>,
        pub stall: Option<Message>,
        pub renderer_progress: f64,
        pub area_progress: f64,
    }

    impl Snapshot {
        pub fn write(&self, w: &mut PacketWriter) {
            w.branch(16, |w| {
                self.pivot.write(w);
                self.location.write(w);
                self.companion.write(w);
                match &self.draft {
                    Some(v) => {
                        w.str(Some(&*(*v)));
                    }
                    None => {
                        w.nil();
                    }
                }
                self.orient.write(w);
                w.bool(Some(self.is_follow_mode));
                w.bool(Some(self.camera_busy));
                w.bool(Some(self.zooming));
                w.bool(Some(self.over_marker));
                w.branch(self.settings.len() as u32, |w| {
                    for x in self.settings.iter() {
                        w.u32(Some((*x) as u32));
                    }
                });
                w.branch(self.division.len() as u32, |w| {
                    for x in self.division.iter() {
                        (*x).write(w);
                    }
                });
                w.branch(self.character_surfaces.len() as u32, |w| {
                    for x in self.character_surfaces.iter() {
                        w.u32(Some((*x) as u32));
                    }
                });
                w.branch(self.refused_files.len() as u32, |w| {
                    for x in self.refused_files.iter() {
                        (*x).write(w);
                    }
                });
                match &self.stall {
                    Some(v) => {
                        w.u32(Some((*v) as u32));
                    }
                    None => {
                        w.nil();
                    }
                }
                w.f64(Some(self.renderer_progress as f64));
                w.f64(Some(self.area_progress as f64));
            });
        }

        pub fn read(c: Cursor<'_>) -> Result<Snapshot, MarshalError> {
            let b = c.as_branch()?;
            Ok(Snapshot {
                pivot: b.at(0).and_then(|c| PivotState::read(c))?,
                location: b.at(1).and_then(|c| LocationState::read(c))?,
                companion: b.at(2).and_then(|c| CompanionState::read(c))?,
                draft: b.at(3).and_then(|c| {
                    c.opt()
                        .map(|c| c.get_str_ref().map(str::to_owned))
                        .transpose()
                })?,
                orient: b.at(4).and_then(|c| OrientationState::read(c))?,
                is_follow_mode: b.at(5).and_then(|c| c.get_bool())?,
                camera_busy: b.at(6).and_then(|c| c.get_bool())?,
                zooming: b.at(7).and_then(|c| c.get_bool())?,
                over_marker: b.at(8).and_then(|c| c.get_bool())?,
                settings: b.at(9).and_then(|c| {
                    c.as_branch().and_then(|b| {
                        (0..b.len)
                            .map(|i| b.at(i).and_then(|c| c.get_number().map(|v| v as u32)))
                            .collect::<Result<Vec<_>, MarshalError>>()
                    })
                })?,
                division: b.at(10).and_then(|c| {
                    c.as_branch().and_then(|b| {
                        (0..b.len)
                            .map(|i| b.at(i).and_then(|c| DivisionPart::read(c)))
                            .collect::<Result<Vec<_>, MarshalError>>()
                    })
                })?,
                character_surfaces: b.at(11).and_then(|c| {
                    c.as_branch().and_then(|b| {
                        (0..b.len)
                            .map(|i| b.at(i).and_then(|c| c.get_number().map(|v| v as u32)))
                            .collect::<Result<Vec<_>, MarshalError>>()
                    })
                })?,
                refused_files: b.at(12).and_then(|c| {
                    c.as_branch().and_then(|b| {
                        (0..b.len)
                            .map(|i| b.at(i).and_then(|c| RefusedFile::read(c)))
                            .collect::<Result<Vec<_>, MarshalError>>()
                    })
                })?,
                stall: b.at(13).and_then(|c| {
                    c.opt()
                        .map(|c| {
                            c.get_number().and_then(|v| {
                                Message::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG))
                            })
                        })
                        .transpose()
                })?,
                renderer_progress: b.at(14).and_then(|c| c.get_number().map(|v| v as f64))?,
                area_progress: b.at(15).and_then(|c| c.get_number().map(|v| v as f64))?,
            })
        }
    }

    pub trait Ui {
        fn sensor_start(&self, sensor: super::types::Sensor) -> Result<(), Errno>;
        fn sensor_stop(&self, sensor: super::types::Sensor) -> Result<(), Errno>;
        fn show(&self, snapshot: Snapshot) -> Result<(), Errno>;
        fn view(&self, azimuth: f64, m_per_px: f64) -> Result<(), Errno>;
    }

    struct UiObject(Rc<dyn Ui>);

    impl Object for UiObject {
        fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
            let run = || -> Result<Reply, MarshalError> {
                Ok(match m {
                    SENSOR_START => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let sensor = b.at(0).and_then(|c| {
                            c.get_number().and_then(|v| {
                                super::types::Sensor::of(v as u32)
                                    .ok_or(MarshalError::new(Errno::EBADMSG))
                            })
                        })?;
                        match self.0.sensor_start(sensor) {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    SENSOR_STOP => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let sensor = b.at(0).and_then(|c| {
                            c.get_number().and_then(|v| {
                                super::types::Sensor::of(v as u32)
                                    .ok_or(MarshalError::new(Errno::EBADMSG))
                            })
                        })?;
                        match self.0.sensor_stop(sensor) {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    SHOW => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let snapshot = b.at(0).and_then(|c| Snapshot::read(c))?;
                        match self.0.show(snapshot) {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        }
                    }
                    VIEW => {
                        let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                        let azimuth = b.at(0).and_then(|c| c.get_number().map(|v| v as f64))?;
                        let m_per_px = b.at(1).and_then(|c| c.get_number().map(|v| v as f64))?;
                        match self.0.view(azimuth, m_per_px) {
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
    pub fn mount(imp: Rc<dyn Ui>) {
        let o: Rc<dyn Object> = Rc::new(UiObject(imp));
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

    pub fn sensor_start(sensor: super::types::Sensor) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            SENSOR_START,
            Some((1, &|w: &mut PacketWriter| {
                w.u32(Some(sensor as u32));
            })),
        );
        link::answer(r).map(|_| ())
    }

    pub fn sensor_stop(sensor: super::types::Sensor) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            SENSOR_STOP,
            Some((1, &|w: &mut PacketWriter| {
                w.u32(Some(sensor as u32));
            })),
        );
        link::answer(r).map(|_| ())
    }

    pub fn show(snapshot: &Snapshot) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            SHOW,
            Some((1, &|w: &mut PacketWriter| {
                snapshot.write(w);
            })),
        );
        link::answer(r).map(|_| ())
    }

    pub fn view(azimuth: f64, m_per_px: f64) -> Result<(), Errno> {
        let (r, data) = link::call(
            handle(),
            VIEW,
            Some((2, &|w: &mut PacketWriter| {
                w.f64(Some(azimuth as f64));
                w.f64(Some(m_per_px as f64));
            })),
        );
        link::answer(r).map(|_| ())
    }
}
