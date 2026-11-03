// pxl-kernel/domain/control/cmd.spec.rs
//
// GENERATED from spec/client/kernel/domain/control/cmd.json

//! The kernel's commands, by number (@xpute/kit/abi/cmd.ts: `major << 8 | minor`),
//! and the packet each carries — an XTP branch whose children are its fields, in
//! order. A command with no fields carries no packet. The kernel is the process; the
//! JavaScript around it is three things to it, its host, which holds the turns, its
//! drivers, which answer its devices, and its shell, the UI. A command's major is the module it is for, and
//! the sections say which of them it is between.
//!
//! The same number is a command on the submission ring and an event on the
//! completion ring (its `RES` clear). The direction is the ring: `pkt` is
//! what the shell submits, `event` what the kernel posts. Each is written and
//! read by what this file generates, on both sides, so no field is found by
//! its position anywhere else.
//!
//! A field is a number of a width, a `bool`, a `str`, a member of an enum
//! below or of one another spec declares (`extern`), or a record below. An
//! `optional` one may be absent, its kind kept; a `list` is a branch of them,
//! or a packed array where the item is a number. A record crosses as a branch
//! of its own fields, and is how a block past a slot is laid out. An enum's
//! `unknown` is the member a number it does not give out reads as; an enum
//! without one refuses the packet. The generator refuses a number given twice.

// A list a record carries is a function handing out its items, whose
// type is long to spell and is spelled once, here, for each. A table
// with no event or no enum read off the wire leaves a writer or a
// number's check unused.
#![allow(
    non_camel_case_types,
    clippy::upper_case_acronyms,
    clippy::type_complexity,
    clippy::absolute_paths,
    unused_imports,
    dead_code,
    reason = "generated from the spec"
)]

use core::fmt::Display;

use xpute_guest::abi::number::{finite, integer, unknown};
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{PacketWriter, Strs, TreeReader};

/// The module a command is for: the kernel's, or the device's that sends it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Major {
    SYS = 0x10,
    INPUT = 0x11,
    SENSOR = 0x12,
    FS = 0x13,
    GPU = 0x14,
    SURFACE = 0x15,
    SETTING = 0x16,
    SCENE = 0x17,
    CAMERA = 0x18,
    PERF = 0x19,
    EDIT = 0x1a,
    SHELL = 0x1b,
}

impl Major {
    pub const ALL: [Major; 12] = [
        Major::SYS,
        Major::INPUT,
        Major::SENSOR,
        Major::FS,
        Major::GPU,
        Major::SURFACE,
        Major::SETTING,
        Major::SCENE,
        Major::CAMERA,
        Major::PERF,
        Major::EDIT,
        Major::SHELL,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Major> {
        Major::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// The system's: the kernel's start, and the platform's sleep and wake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SysMinor {
    BOOT = 0x01,
    SLEEP = 0x02,
}

impl SysMinor {
    pub const ALL: [SysMinor; 2] = [SysMinor::BOOT, SysMinor::SLEEP];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<SysMinor> {
        SysMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// The input's: the pointer, the wheel, the held movement, a tracker's pose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum InputMinor {
    POINTER = 0x02,
    WHEEL = 0x03,
    MOVE = 0x06,
    XR_POSE = 0x09,
    XR_POSE_END = 0x0a,
}

impl InputMinor {
    pub const ALL: [InputMinor; 5] = [
        InputMinor::POINTER,
        InputMinor::WHEEL,
        InputMinor::MOVE,
        InputMinor::XR_POSE,
        InputMinor::XR_POSE_END,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<InputMinor> {
        InputMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// The device sensors'.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SensorMinor {
    START = 0x01,
    POS = 0x02,
    STOP = 0x03,
    ORIENT = 0x05,
    PERMISSION = 0x06,
    ERR = 0x0b,
    LOCATE = 0x0c,
}

impl SensorMinor {
    pub const ALL: [SensorMinor; 7] = [
        SensorMinor::START,
        SensorMinor::POS,
        SensorMinor::STOP,
        SensorMinor::ORIENT,
        SensorMinor::PERMISSION,
        SensorMinor::ERR,
        SensorMinor::LOCATE,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<SensorMinor> {
        SensorMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// The kernel's files: a root mounted, a storage file truncated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum FsMinor {
    MOUNT = 0x01,
    TRUNCATED = 0x02,
}

impl FsMinor {
    pub const ALL: [FsMinor; 2] = [FsMinor::MOUNT, FsMinor::TRUNCATED];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<FsMinor> {
        FsMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// The GPU's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum GpuMinor {
    READY = 0x01,
}

impl GpuMinor {
    pub const ALL: [GpuMinor; 1] = [GpuMinor::READY];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<GpuMinor> {
        GpuMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// A surface's: opened, resized, closed, and the frame drawn into the surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SurfaceMinor {
    OPEN = 0x01,
    RESIZE = 0x02,
    CLOSE = 0x03,
    FRAME = 0x04,
}

impl SurfaceMinor {
    pub const ALL: [SurfaceMinor; 4] = [
        SurfaceMinor::OPEN,
        SurfaceMinor::RESIZE,
        SurfaceMinor::CLOSE,
        SurfaceMinor::FRAME,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<SurfaceMinor> {
        SurfaceMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// A setting's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SettingMinor {
    SET = 0x01,
}

impl SettingMinor {
    pub const ALL: [SettingMinor; 1] = [SettingMinor::SET];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<SettingMinor> {
        SettingMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// The scene's: a character drawn alone into a surface, a line said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SceneMinor {
    PREVIEW_OPEN = 0x01,
    PREVIEW_CLOSE = 0x02,
    CHAT = 0x03,
}

impl SceneMinor {
    pub const ALL: [SceneMinor; 3] = [
        SceneMinor::PREVIEW_OPEN,
        SceneMinor::PREVIEW_CLOSE,
        SceneMinor::CHAT,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<SceneMinor> {
        SceneMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// The camera's: the tracking button, the compass, a search result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum CameraMinor {
    TRACKING = 0x01,
    NORTH_UP = 0x02,
    GOTO = 0x04,
}

impl CameraMinor {
    pub const ALL: [CameraMinor; 3] = [
        CameraMinor::TRACKING,
        CameraMinor::NORTH_UP,
        CameraMinor::GOTO,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<CameraMinor> {
        CameraMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// The profiler's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum PerfMinor {
    START = 0x01,
    STOP = 0x02,
    STATS = 0x03,
    RINGS = 0x04,
}

impl PerfMinor {
    pub const ALL: [PerfMinor; 4] = [
        PerfMinor::START,
        PerfMinor::STOP,
        PerfMinor::STATS,
        PerfMinor::RINGS,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<PerfMinor> {
        PerfMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// The edit's: the tool and what it takes, and a marker's draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum EditMinor {
    TOOL = 0x01,
    MARKER_TEXT = 0x02,
    MARKER_CANCEL = 0x03,
    MARKER_COMMIT = 0x04,
}

impl EditMinor {
    pub const ALL: [EditMinor; 4] = [
        EditMinor::TOOL,
        EditMinor::MARKER_TEXT,
        EditMinor::MARKER_CANCEL,
        EditMinor::MARKER_COMMIT,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<EditMinor> {
        EditMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// What the kernel shows the shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ShellMinor {
    SNAPSHOT = 0x01,
}

impl ShellMinor {
    pub const ALL: [ShellMinor; 1] = [ShellMinor::SNAPSHOT];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<ShellMinor> {
        ShellMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// Where a pointer event is in its gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum PointerPhase {
    Down = 0,
    Move = 1,
    Up = 2,
    Out = 3,
    /// The pointer's gesture ends with no tap: the platform took it, or the shell spent it on something of its own.
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

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<PointerPhase> {
        PointerPhase::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// What the pointer is, as the browser's `pointerType` says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum PointerType {
    Mouse = 0,
    Touch = 1,
    Pen = 2,
}

impl PointerType {
    pub const ALL: [PointerType; 3] = [PointerType::Mouse, PointerType::Touch, PointerType::Pen];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<PointerType> {
        PointerType::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// What the pointer holds: itself, the eraser, or an item to place, which its id says the kind of.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum Tool {
    #[default]
    Pointer = 0,
    Item = 1,
    Eraser = 2,
}

impl Tool {
    pub const ALL: [Tool; 3] = [Tool::Pointer, Tool::Item, Tool::Eraser];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Tool> {
        Tool::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// What the tracking button did, as the button showed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Tracking {
    /// Back to the device.
    Recenter = 0,
    /// Follow mode on, where the compass is listened to.
    Follow = 1,
    /// Follow mode off, north up again.
    Unfollow = 2,
}

impl Tracking {
    pub const ALL: [Tracking; 3] = [Tracking::Recenter, Tracking::Follow, Tracking::Unfollow];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Tracking> {
        Tracking::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// The browser's PositionError codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
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

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<GeolocationErrorCode> {
        GeolocationErrorCode::ALL
            .into_iter()
            .find(|m| *m as u32 == number)
    }
}

/// Which of the device's sensors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Sensor {
    Geolocation = 0,
    Orientation = 1,
}

impl Sensor {
    pub const ALL: [Sensor; 2] = [Sensor::Geolocation, Sensor::Orientation];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Sensor> {
        Sensor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// A sensor permission, as the shell's prompt or the browser's change reported it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum PermissionAnswer {
    /// The shell's prompt is up.
    Asking = 0,
    Granted = 1,
    Denied = 2,
    /// The prompt itself threw.
    Failed = 3,
}

impl PermissionAnswer {
    pub const ALL: [PermissionAnswer; 4] = [
        PermissionAnswer::Asking,
        PermissionAnswer::Granted,
        PermissionAnswer::Denied,
        PermissionAnswer::Failed,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<PermissionAnswer> {
        PermissionAnswer::ALL
            .into_iter()
            .find(|m| *m as u32 == number)
    }
}

/// Whether the screen shows the world: Success once a frame that drew ground went
/// to it, or one went over a view whose tiles all answered with nothing to draw;
/// Loading until then, on a first load, after a jump or a lost device; Idle before
/// there is a view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum GroundStatus {
    #[default]
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

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<GroundStatus> {
        GroundStatus::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// Where a sensor's reading is: none asked for, asked for, arriving, failed, or none to be had, the device having no such sensor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum SensorStatus {
    #[default]
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

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<SensorStatus> {
        SensorStatus::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// A message the kernel raises and the shell shows.
///
/// The kernel decides which message a state calls for and carries the number.
/// It never carries the words: a string that exists to be read has no reason
/// to sit in the kernel's memory, and the shell has the locale files already
/// (ts/client/src/locale) — the same split `strerror` is on the other side of.
///
/// The number is the name here, and a band is what a message is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Message {
    // ---- 0x02 where the device says it is ----
    GeolocationUnsupported = 0x0203,
    GeolocationPermissionDenied = 0x0204,
    GeolocationPositionUnavailable = 0x0205,
    GeolocationTimeout = 0x0206,
    GeolocationGenericError = 0x0207,

    // ---- 0x03 which way the device is facing ----
    OrientationPermissionError = 0x0302,
    OrientationPermissionDenied = 0x0303,
    OrientationSignalLost = 0x0304,

    // ---- 0x04 why the server does not answer the world's reads ----
    /// Reads reach the server and fail; they are tried again.
    WorldReadFailing = 0x0403,
    /// Reads failed past every retry; the world is not coming.
    WorldUnavailable = 0x0404,
}

impl Message {
    pub const ALL: [Message; 10] = [
        Message::GeolocationUnsupported,
        Message::GeolocationPermissionDenied,
        Message::GeolocationPositionUnavailable,
        Message::GeolocationTimeout,
        Message::GeolocationGenericError,
        Message::OrientationPermissionError,
        Message::OrientationPermissionDenied,
        Message::OrientationSignalLost,
        Message::WorldReadFailing,
        Message::WorldUnavailable,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Message> {
        Message::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Command {
    // ---- the process asks the host: its hostcalls, raised on the completion ring and run before the next turn (the rest are the host stream's records, file.json) ----
    /// Start watching.
    SENSOR_START = 0x1201,
    /// Stop watching.
    SENSOR_STOP = 0x1203,

    // ---- the host answers: an I/O the process started, done ----
    /// A GPS fix; a field is absent where the browser gave none.
    SENSOR_POS = 0x1202,
    /// An orientation event; a field is absent where the browser gave none.
    SENSOR_ORIENT = 0x1205,
    /// A watch that ended in failure and was stopped, refused from the start (PermissionDenied)
    /// or cut off; the compass's, where the platform says so (SYS_BOOT's
    /// orient_silence_is_loss).
    SENSOR_ERR = 0x120b,

    // ---- the host tells the process: its start, its turn, its window and its devices ----
    /// The kernel's start: what the platform is. Its storage files it opens itself, on the storage root.
    SYS_BOOT = 0x1001,
    /// The frame of the turn the doorbell's quota rang in: the effects' turn, then the tick's
    /// phases on what is left of the quota, the fetches within the credits. The turn leaves
    /// its events on the completion ring, drawn by the kernel. It carries the host's alone:
    /// the turn's pacing, the network's credits and the clock; the view it is drawn at is the
    /// world surface's (SURFACE_RESIZE).
    SURFACE_FRAME = 0x1504,
    /// The platform about to stop the process, or waking it: the page or the app hidden or
    /// shown. The host rings the doorbell for it at once, since a hidden page gets no frame and
    /// hiding is the save's last sure moment.
    SYS_SLEEP = 0x1002,
    /// What the tile source declares of itself, once the client has it: the tile root's
    /// `source`, the published one out of the bucket or the dev tile server's, read by the
    /// client (ts/client/src/driver/namespace/source.ts) and handed over as numbers and names.
    /// Until it comes nothing is observed: the floor is held at the zoom it names, and no
    /// tile is asked for that no shard it serves is over or under (pxl-kernel sys/io/source.rs).
    FS_MOUNT = 0x1301,
    /// A file the kernel refused (the snapshot's `refused_files`) is empty now, or its source's
    /// own again, what it held set aside by the driver: the kernel reads it again, and writes
    /// a storage file from then on.
    FS_TRUNCATED = 0x1302,
    /// A permission as the shell's prompt or the browser's change reported it.
    SENSOR_PERMISSION = 0x1206,
    /// A device answered: the kernel asks its block (xpute DEVICE) and makes the renderer on it once the block is in (rust/client/render/src/gpu/api.rs). Its loss is the block's to say (xpute STATUS.LOST, written by the host's GpuHost), read at the turn's edge.
    GPU_READY = 0x1401,

    // ---- the shell and the process: what the UI asks of it, the input it routes, and what it shows the UI ----
    /// A pointer event on the world's canvas, in CSS pixels from the top left of its layout box
    /// (transforms aside), as the world surface's size is (SURFACE_RESIZE).
    INPUT_POINTER = 0x1102,
    /// The tool and what it takes, the shell's to choose and keep: the kernel is told, and answers EINVAL for an item the catalog does not give out. It picks again under a hovering pointer.
    EDIT_TOOL = 0x1a01,
    /// The draft's text (Snapshot.draft), which the marker is drawn with and committed with. Text past MARKER_TEXT_CHARS (pxl-spec object/ext.json) is refused whole, never cut: EMSGSIZE, which the shell shows as what does not fit.
    EDIT_MARKER_TEXT = 0x1a02,
    /// The marker's draft dropped, nothing committed.
    EDIT_MARKER_CANCEL = 0x1a03,
    /// The marker's draft committed with its text, where its place is still free or still a marker, and closed.
    EDIT_MARKER_COMMIT = 0x1a04,
    INPUT_WHEEL = 0x1103,
    /// The screen direction the held movement keys point, sent when it changes: each axis -1, 0 or 1, x to the right and y down.
    INPUT_MOVE = 0x1106,
    /// A call, answered with the surface's handle, 0 refused: a canvas the shell hands over, drawn by the kernel alone from then on.
    SURFACE_OPEN = 0x1501,
    /// The canvas's layout box in device pixels, transforms aside; the buffer it is drawn into is the kernel's.
    SURFACE_RESIZE = 0x1502,
    /// A call: the canvas is going, and what it held on the device is let go before the call returns.
    SURFACE_CLOSE = 0x1503,
    /// A character drawn alone into a surface the shell handed over, as the picker shows it.
    SCENE_PREVIEW_OPEN = 0x1701,
    SCENE_PREVIEW_CLOSE = 0x1702,
    /// A setting set from the sheet or the character picker; EAGAIN until the kernel has read the
    /// kept settings, which would be read over it.
    SETTING_SET = 0x1601,
    /// A line the player said, shown over the character.
    SCENE_CHAT = 0x1703,
    /// The tracking button's press, as the button showed it.
    CAMERA_TRACKING = 0x1801,
    /// The compass's press: north up.
    CAMERA_NORTH_UP = 0x1802,
    /// The search panel's location card's press: the location asked for. The compass is
    /// asked for where it is used, entering follow mode (TRACKING), and never alone: nothing
    /// reads it without the location.
    SENSOR_LOCATE = 0x120c,
    /// A search result: the pivot moved there, and the view asked with it at once, as the
    /// controls ask it where the camera stands; a view field absent keeps what the view has.
    CAMERA_GOTO = 0x1804,
    /// A tracked pose: where a tracker has the camera, and its turn as a unit quaternion
    /// (x, y, z, w) from the camera's axes to the tracker's, in the world's units. The
    /// first of a run is taken to be where the camera already is, so nothing moves as
    /// it begins; the camera follows the tracker from there until POSE_END.
    /// Nothing sends it yet: it is where an immersive mode's head pose (a Quest's, a Vision Pro's) would drive
    /// the camera, sent by the device app's driver.
    INPUT_XR_POSE = 0x1109,
    /// The tracker let the camera go: back to the controller it had.
    INPUT_XR_POSE_END = 0x110a,
    /// A fresh recording window.
    PERF_START = 0x1901,
    /// The window frozen, so what is exported holds still.
    PERF_STOP = 0x1902,
    /// The profiler's statistics and what it holds now, where they lie: the reply is the address of STATS's words (perf.json), the statistics
    /// over the last `window_ms`, the whole recording where it is absent, good until the next turn. A
    /// number past the door is finite, so no window is infinite.
    PERF_STATS = 0x1903,
    /// The rings where they lie, nothing reduced: the reply is the address of RINGS's words (perf.json), good until the next turn.
    PERF_RINGS = 0x1904,
    /// The UI's snapshot, when it changed: a Snapshot packet far past a ring frame, so a
    /// block the kernel holds, valid until the next snapshot.
    SHELL_SNAPSHOT = 0x1b01,
}

impl Command {
    pub const ALL: [Command; 37] = [
        Command::SENSOR_START,
        Command::SENSOR_STOP,
        Command::SENSOR_POS,
        Command::SENSOR_ORIENT,
        Command::SENSOR_ERR,
        Command::SYS_BOOT,
        Command::SURFACE_FRAME,
        Command::SYS_SLEEP,
        Command::FS_MOUNT,
        Command::FS_TRUNCATED,
        Command::SENSOR_PERMISSION,
        Command::GPU_READY,
        Command::INPUT_POINTER,
        Command::EDIT_TOOL,
        Command::EDIT_MARKER_TEXT,
        Command::EDIT_MARKER_CANCEL,
        Command::EDIT_MARKER_COMMIT,
        Command::INPUT_WHEEL,
        Command::INPUT_MOVE,
        Command::SURFACE_OPEN,
        Command::SURFACE_RESIZE,
        Command::SURFACE_CLOSE,
        Command::SCENE_PREVIEW_OPEN,
        Command::SCENE_PREVIEW_CLOSE,
        Command::SETTING_SET,
        Command::SCENE_CHAT,
        Command::CAMERA_TRACKING,
        Command::CAMERA_NORTH_UP,
        Command::SENSOR_LOCATE,
        Command::CAMERA_GOTO,
        Command::INPUT_XR_POSE,
        Command::INPUT_XR_POSE_END,
        Command::PERF_START,
        Command::PERF_STOP,
        Command::PERF_STATS,
        Command::PERF_RINGS,
        Command::SHELL_SNAPSHOT,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Command> {
        Command::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// What the page's UI shows of the kernel (shell.rs): written after a turn that changed it, and replaced whole.
pub struct Snapshot<'a> {
    pub pivot: PivotState,
    pub location: LocationState,
    pub companion: CompanionState,
    /// A marker's draft, its text so far, absent where none is open: opened by the marker tool's tap, with the marker's own text where one stands there, empty where it is new, and closed by EDIT_MARKER_COMMIT or EDIT_MARKER_CANCEL.
    pub draft: Option<&'a str>,
    pub orient: OrientationState,
    /// The view's azimuth.
    pub azimuth: f64,
    pub is_follow_mode: bool,
    pub camera_busy: bool,
    /// A zoom is in hand: two fingers are down, or a wheel's zoom is still arriving. The scale is shown while it is.
    pub zooming: bool,
    /// The pointer is over a scene marker a tap would act on with the tool held: a place to go to, or a pin to erase or draft.
    pub over_marker: bool,
    /// Every setting's value by its SettingKey, as SETTING_SET carries one; none until the kernel
    /// has read its storage file, a few turns from boot.
    pub settings: &'a [u32],
    /// The meters a logical pixel spans at the pivot, at the camera's distance; 0 where the camera stands there. The scale bar is the shell's to make of it; which division names show, the kernel's (`DivisionPart.shown`).
    pub m_per_px: f64,
    /// The division the pivot is in and every one above it, broad to narrow; empty
    /// where the pivot is in none or the tile it is read out of has not arrived.
    pub division: &'a dyn Fn(&mut dyn FnMut(&DivisionPart<'_>)),
    /// The surfaces a character preview is drawn on: its model and skin have arrived. The shell shows a case loading until its surface is here.
    pub previews: &'a [u32],
    /// The files the kernel refused, not of their format, not decoding, or too large for any
    /// heap: a storage file (file.json's `storage`) or a placed objects' file (`object`). The
    /// kernel read none of it and writes nothing over it until FS_TRUNCATED says it is
    /// empty.
    pub refused_files: &'a dyn Fn(&mut dyn FnMut(&RefusedFile<'_>)),
    /// Why the server does not answer the world's reads now: failing since the last answered, or given up. Absent while it answers. The network and the GPU are the host's to say.
    pub stall: Option<Message>,
    /// How much of what the renderer makes before its first frame is made, 0 to 1 (pxl-render gpu/pipeline.rs `Pipelines::share`): its shader modules, by their bytes, the first half, its pipelines made the second; 1 once it draws, 0 again while a lost device's next renderer is made. The loading bar is the shell's to draw of it.
    pub renderer_progress: f64,
    /// How much of the view's ground shows, 0 to 1: of the tiles asked for, those with an answer or drawn coarser. 0 where none is asked yet, 1 once the area is shown; it goes back as more is asked for.
    pub area_progress: f64,
}

impl Snapshot<'_> {
    /// The children it is written as.
    pub const FIELDS: u32 = 18;

    pub fn write(&self, w: &mut PacketWriter) {
        w.branch(PivotState::FIELDS, |w| self.pivot.write(w));
        w.branch(LocationState::FIELDS, |w| self.location.write(w));
        w.branch(CompanionState::FIELDS, |w| self.companion.write(w));
        w.str(self.draft);
        w.branch(OrientationState::FIELDS, |w| self.orient.write(w));
        w.f64(Some(self.azimuth));
        w.bool(Some(self.is_follow_mode));
        w.bool(Some(self.camera_busy));
        w.bool(Some(self.zooming));
        w.bool(Some(self.over_marker));
        w.u32_array(Some(self.settings));
        w.f64(Some(self.m_per_px));
        let mut n = 0;
        (self.division)(&mut |_| n += 1);
        w.branch(n, |w| {
            (self.division)(&mut |item| {
                w.branch(DivisionPart::FIELDS, |w| item.write(w));
            });
        });
        w.u32_array(Some(self.previews));
        let mut n = 0;
        (self.refused_files)(&mut |_| n += 1);
        w.branch(n, |w| {
            (self.refused_files)(&mut |item| {
                w.branch(RefusedFile::FIELDS, |w| item.write(w));
            });
        });
        w.u32(self.stall.map(|m| m as u32));
        w.f64(Some(self.renderer_progress));
        w.f64(Some(self.area_progress));
    }
}

pub struct PivotState {
    pub status: GroundStatus,
    pub is_off_center: bool,
    /// The pivot in degrees: what the search measures distance from.
    pub lon_lat: Option<LonLat>,
    /// The pivot is past the service area: the source serves no shard over it (pxl-kernel sys/io/source.rs).
    pub outside: bool,
}

impl PivotState {
    /// The children it is written as.
    pub const FIELDS: u32 = 4;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.status as u32));
        w.bool(Some(self.is_off_center));
        match &self.lon_lat {
            Some(r) => w.branch(LonLat::FIELDS, |w| r.write(w)),
            None => w.no_branch(),
        };
        w.bool(Some(self.outside));
    }
}

pub struct LonLat {
    pub lon: f64,
    pub lat: f64,
}

impl LonLat {
    /// The children it is written as.
    pub const FIELDS: u32 = 2;

    pub fn write(&self, w: &mut PacketWriter) {
        w.f64(Some(self.lon));
        w.f64(Some(self.lat));
    }
}

pub struct LocationState {
    pub status: SensorStatus,
    pub has_location: bool,
    /// Present exactly when status is Error: the failure the kernel chose.
    pub err: Option<Message>,
}

impl LocationState {
    /// The children it is written as.
    pub const FIELDS: u32 = 3;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.status as u32));
        w.bool(Some(self.has_location));
        w.u32(self.err.map(|m| m as u32));
    }
}

pub struct CompanionState {
    pub person_ready: bool,
    pub pet_ready: bool,
}

impl CompanionState {
    /// The children it is written as.
    pub const FIELDS: u32 = 2;

    pub fn write(&self, w: &mut PacketWriter) {
        w.bool(Some(self.person_ready));
        w.bool(Some(self.pet_ready));
    }
}

pub struct OrientationState {
    pub status: SensorStatus,
    /// Present exactly when status is Error: the failure the kernel chose.
    pub err: Option<Message>,
}

impl OrientationState {
    /// The children it is written as.
    pub const FIELDS: u32 = 2;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.status as u32));
        w.u32(self.err.map(|m| m as u32));
    }
}

/// One division of the name, in its own name.
pub struct DivisionPart<'a> {
    /// Overture's subtype, broadest first (spec/spec/division.json).
    pub subtype: u8,
    pub name: &'a dyn Display,
    /// The view is in far enough for its name to show: as far as Overture's own tiles
    /// show it from (pxl-spec bake/tile/division.rs `Division::shown_from`).
    pub shown: bool,
    /// Held by an area the point is in; one without is the nearest found by its point.
    pub area: bool,
}

impl DivisionPart<'_> {
    /// The children it is written as.
    pub const FIELDS: u32 = 4;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u8(Some(self.subtype));
        w.str_display(self.name);
        w.bool(Some(self.shown));
        w.bool(Some(self.area));
    }
}

/// A file the kernel refused, as it opened it.
pub struct RefusedFile<'a> {
    /// file.json's `root`.
    pub root: u32,
    /// Its path below the root, as the kernel opened it.
    pub path: &'a dyn Display,
}

impl RefusedFile<'_> {
    /// The children it is written as.
    pub const FIELDS: u32 = 2;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.root));
        w.str_display(self.path);
    }
}

/// Command::SENSOR_START's event.
pub struct SensorStartEvent {
    pub sensor: Sensor,
}

impl SensorStartEvent {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.sensor as u32));
    }
}

impl xpute_guest::ipc::door::Event for SensorStartEvent {
    const NUMBER: u32 = 4609;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        SensorStartEvent::write(self, w);
    }
}

/// Command::SENSOR_STOP's event.
pub struct SensorStopEvent {
    pub sensor: Sensor,
}

impl SensorStopEvent {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.sensor as u32));
    }
}

impl xpute_guest::ipc::door::Event for SensorStopEvent {
    const NUMBER: u32 = 4611;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        SensorStopEvent::write(self, w);
    }
}

/// Command::SENSOR_POS's packet.
#[derive(Clone, Copy, Debug)]
pub struct SensorPos {
    pub lon: f64,
    pub lat: f64,
    pub alt: Option<f64>,
    pub accuracy: Option<f64>,
    pub course: Option<f64>,
}

impl SensorPos {
    pub fn read(pkt: &[u8]) -> Result<SensorPos, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SensorPos {
            lon: finite(f.at(0)?.get_number()?)?,
            lat: finite(f.at(1)?.get_number()?)?,
            alt: match f.at(2)?.opt() {
                Some(c) => Some(finite(c.get_number()?)?),
                None => None,
            },
            accuracy: match f.at(3)?.opt() {
                Some(c) => Some(finite(c.get_number()?)?),
                None => None,
            },
            course: match f.at(4)?.opt() {
                Some(c) => Some(finite(c.get_number()?)?),
                None => None,
            },
        })
    }
}

/// Command::SENSOR_ORIENT's packet.
#[derive(Clone, Copy, Debug)]
pub struct SensorOrient {
    pub webkit_compass_heading: Option<f64>,
    pub alpha: Option<f64>,
    pub beta: Option<f64>,
    pub gamma: Option<f64>,
}

impl SensorOrient {
    pub fn read(pkt: &[u8]) -> Result<SensorOrient, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SensorOrient {
            webkit_compass_heading: match f.at(0)?.opt() {
                Some(c) => Some(finite(c.get_number()?)?),
                None => None,
            },
            alpha: match f.at(1)?.opt() {
                Some(c) => Some(finite(c.get_number()?)?),
                None => None,
            },
            beta: match f.at(2)?.opt() {
                Some(c) => Some(finite(c.get_number()?)?),
                None => None,
            },
            gamma: match f.at(3)?.opt() {
                Some(c) => Some(finite(c.get_number()?)?),
                None => None,
            },
        })
    }
}

/// Command::SENSOR_ERR's packet.
#[derive(Clone, Copy, Debug)]
pub struct SensorErr {
    pub sensor: Sensor,
    /// Geolocation's; orientation gives none today.
    pub code: Option<GeolocationErrorCode>,
}

impl SensorErr {
    pub fn read(pkt: &[u8]) -> Result<SensorErr, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SensorErr {
            sensor: Sensor::of(integer::<u32>(f.at(0)?.get_number()?)?).ok_or_else(|| unknown())?,
            code: match f.at(1)?.opt() {
                Some(c) => Some(
                    GeolocationErrorCode::of(integer::<u32>(c.get_number()?)?)
                        .unwrap_or(GeolocationErrorCode::Other),
                ),
                None => None,
            },
        })
    }
}

/// Command::SYS_BOOT's packet.
#[derive(Clone, Copy, Debug)]
pub struct SysBoot {
    /// A development build, which offers the settings schema.json marks `dev`.
    pub development: bool,
    pub geolocation: bool,
    pub orient: bool,
    /// iOS 13+'s gate: requestPermission exists and must run inside a gesture.
    pub orient_permission: bool,
    /// A browser's compass talks while the device is still and cannot say it stopped, so
    /// silence is the only sign of loss; a device's speaks only when the heading turns and
    /// reports a failure itself (SENSOR_ERR).
    pub orient_silence_is_loss: bool,
}

impl SysBoot {
    pub fn read(pkt: &[u8]) -> Result<SysBoot, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SysBoot {
            development: f.at(0)?.get_bool()?,
            geolocation: f.at(1)?.get_bool()?,
            orient: f.at(2)?.get_bool()?,
            orient_permission: f.at(3)?.get_bool()?,
            orient_silence_is_loss: f.at(4)?.get_bool()?,
        })
    }
}

/// Command::SURFACE_FRAME's packet.
#[derive(Clone, Copy, Debug)]
pub struct SurfaceFrame {
    /// The display's refresh, as the host measures it from the animation frames it is handed,
    /// and the interval it paces turns at, a whole number of refreshes (the pacing the client
    /// boots it with, @xpute/host sched/frame_pacing.ts `FrameGrant`): the frame's budget.
    pub refresh_ms: f64,
    pub paced_ms: f64,
    pub interactive: bool,
    pub credits: u32,
    /// The page's one clock, in Unix milliseconds: the wall time the sky is placed by, and
    /// the step the tick takes, its change since the last frame.
    pub unix_ms: f64,
}

impl SurfaceFrame {
    pub fn read(pkt: &[u8]) -> Result<SurfaceFrame, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SurfaceFrame {
            refresh_ms: finite(f.at(0)?.get_number()?)?,
            paced_ms: finite(f.at(1)?.get_number()?)?,
            interactive: f.at(2)?.get_bool()?,
            credits: integer::<u32>(f.at(3)?.get_number()?)?,
            unix_ms: finite(f.at(4)?.get_number()?)?,
        })
    }
}

/// Command::SYS_SLEEP's packet.
#[derive(Clone, Copy, Debug)]
pub struct SysSleep {
    pub awake: bool,
}

impl SysSleep {
    pub fn read(pkt: &[u8]) -> Result<SysSleep, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SysSleep {
            awake: f.at(0)?.get_bool()?,
        })
    }
}

/// Command::FS_MOUNT's packet.
#[derive(Clone, Copy, Debug)]
pub struct FsMount<'a> {
    /// The coarsest zoom the source supplies whole.
    pub top_zoom: u32,
    /// Each kind's format, one a kind, indexed by its bake's number
    /// (pxl-spec bake/publication.spec.rs).
    pub format: &'a [u32],
    /// The shards it serves, by name. Their order means nothing: no two at a zoom
    /// are over one node, and the finer wins.
    pub shard: Strs<'a>,
}

impl<'a> FsMount<'a> {
    pub fn read(pkt: &'a [u8]) -> Result<FsMount<'a>, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(FsMount {
            top_zoom: integer::<u32>(f.at(0)?.get_number()?)?,
            format: f.at(1)?.get_array::<u32>()?,
            shard: f.at(2)?.get_strs()?,
        })
    }
}

/// Command::FS_TRUNCATED's packet.
#[derive(Clone, Copy, Debug)]
pub struct FsTruncated<'a> {
    /// file.json's `root`.
    pub root: u32,
    /// Its path below the root, as the kernel opened it.
    pub path: &'a str,
}

impl<'a> FsTruncated<'a> {
    pub fn read(pkt: &'a [u8]) -> Result<FsTruncated<'a>, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(FsTruncated {
            root: integer::<u32>(f.at(0)?.get_number()?)?,
            path: f.at(1)?.get_str_ref()?,
        })
    }
}

/// Command::SENSOR_PERMISSION's packet.
#[derive(Clone, Copy, Debug)]
pub struct SensorPermission {
    pub sensor: Sensor,
    pub answer: PermissionAnswer,
}

impl SensorPermission {
    pub fn read(pkt: &[u8]) -> Result<SensorPermission, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SensorPermission {
            sensor: Sensor::of(integer::<u32>(f.at(0)?.get_number()?)?).ok_or_else(|| unknown())?,
            answer: PermissionAnswer::of(integer::<u32>(f.at(1)?.get_number()?)?)
                .ok_or_else(|| unknown())?,
        })
    }
}

/// Command::INPUT_POINTER's packet.
#[derive(Clone, Copy, Debug)]
pub struct InputPointer {
    pub phase: PointerPhase,
    pub pointer_type: PointerType,
    pub id: i32,
    pub button: i32,
    pub x: f64,
    pub y: f64,
}

impl InputPointer {
    pub fn read(pkt: &[u8]) -> Result<InputPointer, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(InputPointer {
            phase: PointerPhase::of(integer::<u32>(f.at(0)?.get_number()?)?)
                .ok_or_else(|| unknown())?,
            pointer_type: PointerType::of(integer::<u32>(f.at(1)?.get_number()?)?)
                .ok_or_else(|| unknown())?,
            id: integer::<i32>(f.at(2)?.get_number()?)?,
            button: integer::<i32>(f.at(3)?.get_number()?)?,
            x: finite(f.at(4)?.get_number()?)?,
            y: finite(f.at(5)?.get_number()?)?,
        })
    }
}

/// Command::EDIT_TOOL's packet.
#[derive(Clone, Copy, Debug)]
pub struct EditTool {
    pub tool: Tool,
    /// The item to place, its id (id.json); read only for that tool.
    pub val: u32,
    /// A cube's scale, as a decor's `scale_steps`: 0 a unit, each step down its half, to 1 - OBJECT_SCALES (pxl-spec bake/object.spec.rs). Read only for an item, and only a cube's is chosen.
    pub scale_steps: i32,
}

impl EditTool {
    pub fn read(pkt: &[u8]) -> Result<EditTool, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(EditTool {
            tool: Tool::of(integer::<u32>(f.at(0)?.get_number()?)?).ok_or_else(|| unknown())?,
            val: integer::<u32>(f.at(1)?.get_number()?)?,
            scale_steps: integer::<i32>(f.at(2)?.get_number()?)?,
        })
    }
}

/// Command::EDIT_MARKER_TEXT's packet.
#[derive(Clone, Copy, Debug)]
pub struct EditMarkerText<'a> {
    pub text: &'a str,
}

impl<'a> EditMarkerText<'a> {
    pub fn read(pkt: &'a [u8]) -> Result<EditMarkerText<'a>, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(EditMarkerText {
            text: f.at(0)?.get_str_ref()?,
        })
    }
}

/// Command::INPUT_WHEEL's packet.
#[derive(Clone, Copy, Debug)]
pub struct InputWheel {
    pub delta_y: f64,
}

impl InputWheel {
    pub fn read(pkt: &[u8]) -> Result<InputWheel, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(InputWheel {
            delta_y: finite(f.at(0)?.get_number()?)?,
        })
    }
}

/// Command::INPUT_MOVE's packet.
#[derive(Clone, Copy, Debug)]
pub struct InputMove {
    pub x: i32,
    pub y: i32,
}

impl InputMove {
    pub fn read(pkt: &[u8]) -> Result<InputMove, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(InputMove {
            x: integer::<i32>(f.at(0)?.get_number()?)?,
            y: integer::<i32>(f.at(1)?.get_number()?)?,
        })
    }
}

/// Command::SURFACE_OPEN's packet.
#[derive(Clone, Copy, Debug)]
pub struct SurfaceOpen {
    /// The renderer's surface_role (render/gpu/api.json).
    pub role: u32,
}

impl SurfaceOpen {
    pub fn read(pkt: &[u8]) -> Result<SurfaceOpen, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SurfaceOpen {
            role: integer::<u32>(f.at(0)?.get_number()?)?,
        })
    }
}

/// Command::SURFACE_RESIZE's packet.
#[derive(Clone, Copy, Debug)]
pub struct SurfaceResize {
    pub surface: u32,
    pub width: u32,
    pub height: u32,
    pub device_pixel_ratio: f64,
}

impl SurfaceResize {
    pub fn read(pkt: &[u8]) -> Result<SurfaceResize, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SurfaceResize {
            surface: integer::<u32>(f.at(0)?.get_number()?)?,
            width: integer::<u32>(f.at(1)?.get_number()?)?,
            height: integer::<u32>(f.at(2)?.get_number()?)?,
            device_pixel_ratio: finite(f.at(3)?.get_number()?)?,
        })
    }
}

/// Command::SURFACE_CLOSE's packet.
#[derive(Clone, Copy, Debug)]
pub struct SurfaceClose {
    pub surface: u32,
}

impl SurfaceClose {
    pub fn read(pkt: &[u8]) -> Result<SurfaceClose, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SurfaceClose {
            surface: integer::<u32>(f.at(0)?.get_number()?)?,
        })
    }
}

/// Command::SCENE_PREVIEW_OPEN's packet.
#[derive(Clone, Copy, Debug)]
pub struct ScenePreviewOpen {
    pub surface: u32,
    pub group: pxl_spec::asset::actor::Group,
    pub number: u8,
}

impl ScenePreviewOpen {
    pub fn read(pkt: &[u8]) -> Result<ScenePreviewOpen, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(ScenePreviewOpen {
            surface: integer::<u32>(f.at(0)?.get_number()?)?,
            group: pxl_spec::asset::actor::Group::of(integer::<u8>(f.at(1)?.get_number()?)?)
                .ok_or_else(|| unknown())?,
            number: integer::<u8>(f.at(2)?.get_number()?)?,
        })
    }
}

/// Command::SCENE_PREVIEW_CLOSE's packet.
#[derive(Clone, Copy, Debug)]
pub struct ScenePreviewClose {
    pub surface: u32,
}

impl ScenePreviewClose {
    pub fn read(pkt: &[u8]) -> Result<ScenePreviewClose, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(ScenePreviewClose {
            surface: integer::<u32>(f.at(0)?.get_number()?)?,
        })
    }
}

/// Command::SETTING_SET's packet.
#[derive(Clone, Copy, Debug)]
pub struct SettingSet {
    pub key: crate::domain::view::setting::schema_spec::SettingKey,
    /// A toggle's 0 or 1, a choice's option by its index, or a number.
    pub value: u32,
}

impl SettingSet {
    pub fn read(pkt: &[u8]) -> Result<SettingSet, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SettingSet {
            key: crate::domain::view::setting::schema_spec::SettingKey::of(integer::<u32>(
                f.at(0)?.get_number()?,
            )?)
            .ok_or_else(|| unknown())?,
            value: integer::<u32>(f.at(1)?.get_number()?)?,
        })
    }
}

/// Command::SCENE_CHAT's packet.
#[derive(Clone, Copy, Debug)]
pub struct SceneChat<'a> {
    pub text: &'a str,
}

impl<'a> SceneChat<'a> {
    pub fn read(pkt: &'a [u8]) -> Result<SceneChat<'a>, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SceneChat {
            text: f.at(0)?.get_str_ref()?,
        })
    }
}

/// Command::CAMERA_TRACKING's packet.
#[derive(Clone, Copy, Debug)]
pub struct CameraTracking {
    pub tracking: Tracking,
}

impl CameraTracking {
    pub fn read(pkt: &[u8]) -> Result<CameraTracking, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(CameraTracking {
            tracking: Tracking::of(integer::<u32>(f.at(0)?.get_number()?)?)
                .ok_or_else(|| unknown())?,
        })
    }
}

/// Command::CAMERA_GOTO's packet.
#[derive(Clone, Copy, Debug)]
pub struct CameraGoto {
    pub lon: f64,
    pub lat: f64,
    /// The zoom, the eye's distance from the pivot, clamped to the zoom's range; absent, a view far out comes in.
    pub dist: Option<f64>,
    /// The heading about Z in radians, wrapped.
    pub azimuth: Option<f64>,
    /// The angle down from vertical in radians, clamped.
    pub polar: Option<f64>,
}

impl CameraGoto {
    pub fn read(pkt: &[u8]) -> Result<CameraGoto, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(CameraGoto {
            lon: finite(f.at(0)?.get_number()?)?,
            lat: finite(f.at(1)?.get_number()?)?,
            dist: match f.at(2)?.opt() {
                Some(c) => Some(finite(c.get_number()?)?),
                None => None,
            },
            azimuth: match f.at(3)?.opt() {
                Some(c) => Some(finite(c.get_number()?)?),
                None => None,
            },
            polar: match f.at(4)?.opt() {
                Some(c) => Some(finite(c.get_number()?)?),
                None => None,
            },
        })
    }
}

/// Command::INPUT_XR_POSE's packet.
#[derive(Clone, Copy, Debug)]
pub struct InputXrPose {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub qx: f64,
    pub qy: f64,
    pub qz: f64,
    pub qw: f64,
}

impl InputXrPose {
    pub fn read(pkt: &[u8]) -> Result<InputXrPose, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(InputXrPose {
            x: finite(f.at(0)?.get_number()?)?,
            y: finite(f.at(1)?.get_number()?)?,
            z: finite(f.at(2)?.get_number()?)?,
            qx: finite(f.at(3)?.get_number()?)?,
            qy: finite(f.at(4)?.get_number()?)?,
            qz: finite(f.at(5)?.get_number()?)?,
            qw: finite(f.at(6)?.get_number()?)?,
        })
    }
}

/// Command::PERF_STATS's packet.
#[derive(Clone, Copy, Debug)]
pub struct PerfStats {
    pub window_ms: Option<f64>,
}

impl PerfStats {
    pub fn read(pkt: &[u8]) -> Result<PerfStats, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(PerfStats {
            window_ms: match f.at(0)?.opt() {
                Some(c) => Some(finite(c.get_number()?)?),
                None => None,
            },
        })
    }
}

/// Command::SHELL_SNAPSHOT's event.
pub struct ShellSnapshotEvent {
    pub at: u32,
    pub len: u32,
}

impl ShellSnapshotEvent {
    /// The children it is written as.
    pub const FIELDS: u32 = 2;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.at));
        w.u32(Some(self.len));
    }
}

impl xpute_guest::ipc::door::Event for ShellSnapshotEvent {
    const NUMBER: u32 = 6913;
    const FIELDS: u32 = 2;
    fn write(&self, w: &mut PacketWriter) {
        ShellSnapshotEvent::write(self, w);
    }
}
