// rust/client/core/src/door/cmd.spec.rs
//
// GENERATED from spec/client/core/door/ui.json

//! The UI's protocol with the core: its commands, by number (@xpute/runtime io/frame.ts `CMD`:
//! `major << 8 | minor`, past xpute's majors 0x00 to 0x0F), each with the packet
//! it carries — an XTP branch whose children are its fields, in order. A command with no
//! fields carries no packet. The JavaScript around the core is two
//! things to it, its runtime, which holds the turns and carries its imports to what
//! answers them (ts/client/src/service/sys/), and its UI, which sends what the user
//! does and what the place's sensors, visibility and frames tell it
//! (ts/client/src/service/core/). A command's major is the module it is for.
//!
//! `export` is what the UI sends the core and `import` what the core posts to it, `call`
//! one its sender waits on. Each is written and read by what this file generates, on
//! both sides, so no field is found by its position anywhere else.
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
// with no import or no enum read off the wire leaves a writer or a
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

pub use crate::domain::cmd::{
    GeolocationErrorCode, Message, PermissionAnswer, PointerType, SensorStatus, Tracking,
};
pub use crate::domain::setting::schema_spec::SettingKey;
pub use pxl_spec::asset::character::Group as CharacterGroup;
use xpute_core::abi::number::{finite, integer, unknown};
use xpute_core::io::fs::{Path, Token};
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{PacketWriter, StrList, TreeReader};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Major {
    GPU = 0x10,
    INPUT = 0x11,
    SENSOR = 0x12,
    SOURCE = 0x13,
    SETTING = 0x16,
    SCENE = 0x17,
    CAMERA = 0x18,
    PERF = 0x19,
    EDIT = 0x1a,
    UI = 0x1b,
    TURN = 0x1d,
}

impl Major {
    pub const ALL: [Major; 11] = [
        Major::GPU,
        Major::INPUT,
        Major::SENSOR,
        Major::SOURCE,
        Major::SETTING,
        Major::SCENE,
        Major::CAMERA,
        Major::PERF,
        Major::EDIT,
        Major::UI,
        Major::TURN,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Major> {
        Major::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum InputMinor {
    POINTER = 0x02,
    WHEEL = 0x03,
    MOVE = 0x06,
    XR_POSE = 0x09,
    XR_POSE_END = 0x0a,
    TYPING = 0x0b,
    PINCH = 0x0c,
}

impl InputMinor {
    pub const ALL: [InputMinor; 7] = [
        InputMinor::POINTER,
        InputMinor::WHEEL,
        InputMinor::MOVE,
        InputMinor::XR_POSE,
        InputMinor::XR_POSE_END,
        InputMinor::TYPING,
        InputMinor::PINCH,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<InputMinor> {
        InputMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

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
    PLATFORM = 0x0d,
}

impl SensorMinor {
    pub const ALL: [SensorMinor; 8] = [
        SensorMinor::START,
        SensorMinor::POS,
        SensorMinor::STOP,
        SensorMinor::ORIENT,
        SensorMinor::PERMISSION,
        SensorMinor::ERR,
        SensorMinor::LOCATE,
        SensorMinor::PLATFORM,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<SensorMinor> {
        SensorMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SourceMinor {
    DECLARE = 0x01,
}

impl SourceMinor {
    pub const ALL: [SourceMinor; 1] = [SourceMinor::DECLARE];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<SourceMinor> {
        SourceMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum TurnMinor {
    GRANT = 0x01,
    PACE = 0x02,
}

impl TurnMinor {
    pub const ALL: [TurnMinor; 2] = [TurnMinor::GRANT, TurnMinor::PACE];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<TurnMinor> {
        TurnMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SettingMinor {
    SET = 0x01,
    DEV = 0x02,
}

impl SettingMinor {
    pub const ALL: [SettingMinor; 2] = [SettingMinor::SET, SettingMinor::DEV];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<SettingMinor> {
        SettingMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SceneMinor {
    CHAT = 0x03,
    DRAW = 0x04,
}

impl SceneMinor {
    pub const ALL: [SceneMinor; 2] = [SceneMinor::CHAT, SceneMinor::DRAW];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<SceneMinor> {
        SceneMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum PerfMinor {
    START = 0x01,
    STOP = 0x02,
    STATS = 0x03,
    RINGS = 0x04,
    STRTAB = 0x05,
}

impl PerfMinor {
    pub const ALL: [PerfMinor; 5] = [
        PerfMinor::START,
        PerfMinor::STOP,
        PerfMinor::STATS,
        PerfMinor::RINGS,
        PerfMinor::STRTAB,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<PerfMinor> {
        PerfMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum UiMinor {
    SNAPSHOT = 0x01,
    VIEW = 0x02,
}

impl UiMinor {
    pub const ALL: [UiMinor; 2] = [UiMinor::SNAPSHOT, UiMinor::VIEW];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<UiMinor> {
        UiMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum PointerPhase {
    Down = 0,
    Move = 1,
    Up = 2,
    Out = 3,
    /// The pointer's gesture ends with no tap: the platform took it, or the UI spent it on something of its own.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SceneContent {
    World = 0,
    Character = 1,
}

impl SceneContent {
    pub const ALL: [SceneContent; 2] = [SceneContent::World, SceneContent::Character];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<SceneContent> {
        SceneContent::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

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

/// Every command, by the side that sends it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Command {
    // ---- ui, export ----
    /// A GPS fix; a field is absent where the browser gave none.
    SENSOR_POS = 0x1202,
    /// An orientation event; a field is absent where the browser gave none.
    SENSOR_ORIENT = 0x1205,
    /// A watch's failure: refused from the start (PermissionDenied), cut off, or one it rides
    /// out; the compass's, where the platform says so (SENSOR_PLATFORM's orient_silence_is_loss).
    SENSOR_ERR = 0x120b,
    /// What the tile source declares of itself, once the client has it: the tile root's
    /// `source`, the published one out of the bucket or the dev tile server's, read by the
    /// client (ts/client/src/service/sys/fs/source.ts) and handed over as numbers and names.
    /// Until it comes nothing is observed: the floor is held at the zoom it names, and no
    /// tile is asked for that no shard it serves is over or under (pxl-core sys/io/source.rs).
    SOURCE_DECLARE = 0x1301,
    /// A permission as the UI's prompt or the browser's change reported it.
    SENSOR_PERMISSION = 0x1206,
    /// What the place's sensors are: whether it has a position and a heading, whether the
    /// heading asks a permission, and how it says it is lost. Sent once the core is attached.
    SENSOR_PLATFORM = 0x120d,
    /// Whether this is a development build, which offers the settings schema.json marks `dev`.
    /// Sent once the core is attached.
    SETTING_DEV = 0x1602,
    /// A pointer event on the world's canvas, in CSS pixels from the top left of its layout box
    /// (transforms aside), as the world surface's size is (GPU_SURFACE_RESIZE).
    INPUT_POINTER = 0x1102,
    /// Whether the UI is taking typed text. A pointer pressed while this holds, a device's touch too (pxl-core door/touch.rs), is spent on ending the typing: its lift ends the gesture with no tap, and its drag still moves the world.
    INPUT_TYPING = 0x110b,
    /// The tool and what it takes, the UI's to choose and keep: the core is told, and answers EINVAL for an item the catalog does not give out. It picks again under a hovering pointer.
    EDIT_TOOL = 0x1a01,
    /// The draft's text (Snapshot.draft), which the marker is drawn with and committed with. Text past MARKER_TEXT_CHARS (spec/spec/asset/marker.json) is refused whole, never cut: EMSGSIZE, which the UI shows as what does not fit.
    EDIT_MARKER_TEXT = 0x1a02,
    EDIT_MARKER_CANCEL = 0x1a03,
    /// The marker's draft committed with its text, where its place is still free or still a marker, and closed.
    EDIT_MARKER_COMMIT = 0x1a04,
    INPUT_WHEEL = 0x1103,
    /// A pinch the platform recognized, as a trackpad's is: the factor the fingers' spread changed by since the last. Over one, they spread, and the camera comes nearer by that factor, as under two touches.
    INPUT_PINCH = 0x110c,
    /// The screen direction the held movement keys point, sent when it changes: each axis -1, 0 or 1, x to the right and y down.
    INPUT_MOVE = 0x1106,
    /// What a surface the UI opened draws, until its GPU_SURFACE_CLOSE. The world, with the
    /// sensors watched while it shows; or the companion's character of a group alone, as the picker shows it, a second one
    /// on the same surface replacing the first. EBADF where the surface is not open, EBUSY where it already draws the
    /// other content, EINVAL where the character's fields are not all given for a character or any is given for the
    /// world.
    SCENE_DRAW = 0x1704,
    /// A setting set from the sheet or the character picker; refused until the core has read the
    /// kept settings, which would be read over it.
    SETTING_SET = 0x1601,
    /// A line the player said, shown over the character.
    SCENE_CHAT = 0x1703,
    /// The tracking button's press, as the button showed it.
    CAMERA_TRACKING = 0x1801,
    /// The compass was pressed: north comes up.
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
    /// it begins; the camera follows the tracker from there until POSE_END. Nothing in the
    /// client sends it: it is the way in for an immersive mode's head pose to drive the
    /// camera, from the device app.
    INPUT_XR_POSE = 0x1109,
    /// The tracker let the camera go: back to the controller it had.
    INPUT_XR_POSE_END = 0x110a,
    /// Starts a fresh recording window.
    PERF_START = 0x1901,
    /// Freezes the window, so what is exported holds still.
    PERF_STOP = 0x1902,
    /// The profiler's statistics and what it holds now: the reply names STAT's words then NOW's (perf.json), the statistics
    /// over the last `window_ms`, the whole recording where it is absent. A
    /// number past the door is finite, so no window is infinite.
    PERF_STATS = 0x1903,
    /// One ring, nothing reduced: the reply names the records of the ring `ring` (its index in perf.json's RING_NAMES), the frames and the tasks oldest first, the worst frames longest first.
    PERF_RINGS = 0x1904,
    /// The string table of the tasks' failure sites: the reply names its bytes, into which a TASK's `at_file` is an offset (perf.json).
    PERF_STRTAB = 0x1905,

    // ---- ui, import ----
    SENSOR_START = 0x1201,
    SENSOR_STOP = 0x1203,
    /// The UI's snapshot, when it changed: a Snapshot packet far past a frame's room, so a
    /// block the core holds, valid until the next snapshot.
    UI_SNAPSHOT = 0x1b01,
    /// The view's azimuth and scale, when either moved: a frame of its own, so a rotation or a
    /// zoom, which moves them every frame, leaves the snapshot as it was.
    UI_VIEW = 0x1b02,
}

impl Command {
    pub const ALL: [Command; 34] = [
        Command::SENSOR_POS,
        Command::SENSOR_ORIENT,
        Command::SENSOR_ERR,
        Command::SOURCE_DECLARE,
        Command::SENSOR_PERMISSION,
        Command::SENSOR_PLATFORM,
        Command::SETTING_DEV,
        Command::INPUT_POINTER,
        Command::INPUT_TYPING,
        Command::EDIT_TOOL,
        Command::EDIT_MARKER_TEXT,
        Command::EDIT_MARKER_CANCEL,
        Command::EDIT_MARKER_COMMIT,
        Command::INPUT_WHEEL,
        Command::INPUT_PINCH,
        Command::INPUT_MOVE,
        Command::SCENE_DRAW,
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
        Command::PERF_STRTAB,
        Command::SENSOR_START,
        Command::SENSOR_STOP,
        Command::UI_SNAPSHOT,
        Command::UI_VIEW,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Command> {
        Command::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// What the page's UI shows of the core (ui.rs): written after a turn that changed it, and replaced whole.
pub struct Snapshot<'a> {
    pub pivot: PivotState,
    pub location: LocationState,
    pub companion: CompanionState,
    /// A marker's draft, its text so far, absent where none is open: opened by the marker tool's tap, with the marker's own text where one stands there, empty where it is new, and closed by EDIT_MARKER_COMMIT or EDIT_MARKER_CANCEL.
    pub draft: Option<&'a str>,
    pub orient: OrientationState,
    pub is_follow_mode: bool,
    pub camera_busy: bool,
    /// A zoom is in hand: two fingers are down, or a wheel's or a trackpad pinch's zoom is still arriving. The scale is shown while it is.
    pub zooming: bool,
    /// The pointer is over a scene marker a tap would act on with the tool held: a place to go to, or a pin to erase or draft.
    pub over_marker: bool,
    /// Every setting's value by its SettingKey, as SETTING_SET carries one; none until the core
    /// has read its storage file, a few turns from boot.
    pub settings: &'a [u32],
    /// The division the pivot is in and every one above it, broad to narrow; empty
    /// where the pivot is in none or the tile it is read out of has not arrived.
    pub division: &'a dyn Fn(&mut dyn FnMut(&DivisionPart<'_>)),
    /// The surfaces a character is drawn on: its model and skin have arrived. The UI shows one loading until its surface is here.
    pub character_surfaces: &'a [u32],
    /// The files the core refused, not of their format, not decoding, or too large for any
    /// heap: a storage file (file.json's `storage`) or a placed objects' file (`object`). The
    /// core read none of it and writes nothing over it until FS_CHANGED says it is
    /// empty.
    pub refused_files: &'a dyn Fn(&mut dyn FnMut(&RefusedFile<'_>)),
    /// Why the server does not answer the world's reads now: failing since the last answered, or given up. Absent while it answers. The network and the GPU are the runtime's to say.
    pub stall: Option<Message>,
    /// How much of what the renderer makes before its first frame is made, 0 to 1 (pxl-render gpu/pipeline.rs `PipelineCache::share`): the pipelines made of those asked for, in whatever order they finish, 0 before any is asked; 1 once it draws, 0 again while a lost device's next renderer is made. The loading bar is the UI's to draw of it.
    pub renderer_progress: f64,
    /// How much of the view's ground shows, 0 to 1: of the tiles asked for, those with an answer or drawn coarser. 0 where none is asked yet, 1 once the area is shown; it goes back as more is asked for.
    pub area_progress: f64,
}

impl Snapshot<'_> {
    /// The children it is written as.
    pub const FIELDS: u32 = 16;

    pub fn write(&self, w: &mut PacketWriter) {
        w.branch(PivotState::FIELDS, |w| self.pivot.write(w));
        w.branch(LocationState::FIELDS, |w| self.location.write(w));
        w.branch(CompanionState::FIELDS, |w| self.companion.write(w));
        w.str(self.draft);
        w.branch(OrientationState::FIELDS, |w| self.orient.write(w));
        w.bool(Some(self.is_follow_mode));
        w.bool(Some(self.camera_busy));
        w.bool(Some(self.zooming));
        w.bool(Some(self.over_marker));
        w.u32_array(Some(self.settings));
        let mut n = 0;
        (self.division)(&mut |_| n += 1);
        w.branch(n, |w| {
            (self.division)(&mut |item| {
                w.branch(DivisionPart::FIELDS, |w| item.write(w));
            });
        });
        w.u32_array(Some(self.character_surfaces));
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
    /// The pivot is past the service area: the source serves no shard over it (pxl-core sys/io/source.rs).
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
    /// Whether a watch runs, so that asking for the location again would start nothing.
    pub watching: bool,
    /// Present exactly when status is Error: the failure the core chose.
    pub err: Option<Message>,
}

impl LocationState {
    /// The children it is written as.
    pub const FIELDS: u32 = 4;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.status as u32));
        w.bool(Some(self.has_location));
        w.bool(Some(self.watching));
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
}

impl OrientationState {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.status as u32));
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

pub struct RefusedFile<'a> {
    /// Its path, as the core opened it (file.json `path`).
    pub path: &'a [Token<'a>],
}

impl RefusedFile<'_> {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        Path::write(w, self.path);
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
    /// Geolocation's; orientation gives none.
    pub code: Option<GeolocationErrorCode>,
    /// Whether the watch runs on past it, which the platform alone knows. A compass that
    /// is lost runs no more.
    pub watching: bool,
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
            watching: f.at(2)?.get_bool()?,
        })
    }
}

/// Command::SOURCE_DECLARE's packet.
#[derive(Clone, Copy, Debug)]
pub struct SourceDeclare<'a> {
    /// The coarsest zoom the source supplies whole.
    pub min_zoom: u32,
    /// Each kind's format, one a kind, indexed by its bake's number
    /// (pxl-spec bake/publication.spec.rs).
    pub format: &'a [u32],
    /// The shards it serves, by name. Their order means nothing: no two at a zoom
    /// are over one node, and the finer wins.
    pub shard: StrList<'a>,
}

impl<'a> SourceDeclare<'a> {
    pub fn read(pkt: &'a [u8]) -> Result<SourceDeclare<'a>, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SourceDeclare {
            min_zoom: integer::<u32>(f.at(0)?.get_number()?)?,
            format: f.at(1)?.get_array::<u32>()?,
            shard: f.at(2)?.get_strs()?,
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

/// Command::SENSOR_PLATFORM's packet.
#[derive(Clone, Copy, Debug)]
pub struct SensorPlatform {
    pub geolocation: bool,
    pub orient: bool,
    /// iOS 13+'s gate: requestPermission exists and must run inside a gesture.
    pub orient_permission: bool,
    /// A browser's compass talks while the device is still and cannot say it stopped, so
    /// silence is the only sign of loss; a device's speaks only when the heading turns and
    /// reports a failure itself (SENSOR_ERR).
    pub orient_silence_is_loss: bool,
}

impl SensorPlatform {
    pub fn read(pkt: &[u8]) -> Result<SensorPlatform, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SensorPlatform {
            geolocation: f.at(0)?.get_bool()?,
            orient: f.at(1)?.get_bool()?,
            orient_permission: f.at(2)?.get_bool()?,
            orient_silence_is_loss: f.at(3)?.get_bool()?,
        })
    }
}

/// Command::SETTING_DEV's packet.
#[derive(Clone, Copy, Debug)]
pub struct SettingDev {
    pub dev: bool,
}

impl SettingDev {
    pub fn read(pkt: &[u8]) -> Result<SettingDev, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SettingDev {
            dev: f.at(0)?.get_bool()?,
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

/// Command::INPUT_TYPING's packet.
#[derive(Clone, Copy, Debug)]
pub struct InputTyping {
    pub typing: bool,
}

impl InputTyping {
    pub fn read(pkt: &[u8]) -> Result<InputTyping, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(InputTyping {
            typing: f.at(0)?.get_bool()?,
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

/// Command::INPUT_PINCH's packet.
#[derive(Clone, Copy, Debug)]
pub struct InputPinch {
    pub scale: f64,
}

impl InputPinch {
    pub fn read(pkt: &[u8]) -> Result<InputPinch, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(InputPinch {
            scale: finite(f.at(0)?.get_number()?)?,
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

/// Command::SCENE_DRAW's packet.
#[derive(Clone, Copy, Debug)]
pub struct SceneDraw {
    pub surface: u32,
    pub content: SceneContent,
    pub group: Option<CharacterGroup>,
    /// What the character is drawn over, 0xRRGGBB, sRGB.
    pub bg: Option<u32>,
}

impl SceneDraw {
    pub fn read(pkt: &[u8]) -> Result<SceneDraw, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SceneDraw {
            surface: integer::<u32>(f.at(0)?.get_number()?)?,
            content: SceneContent::of(integer::<u32>(f.at(1)?.get_number()?)?)
                .ok_or_else(|| unknown())?,
            group: match f.at(2)?.opt() {
                Some(c) => Some(
                    CharacterGroup::of(integer::<u8>(c.get_number()?)?).ok_or_else(|| unknown())?,
                ),
                None => None,
            },
            bg: match f.at(3)?.opt() {
                Some(c) => Some(integer::<u32>(c.get_number()?)?),
                None => None,
            },
        })
    }
}

/// Command::SETTING_SET's packet.
#[derive(Clone, Copy, Debug)]
pub struct SettingSet {
    pub key: SettingKey,
    /// A toggle's 0 or 1, a choice's option by its index, or a number.
    pub val: u32,
}

impl SettingSet {
    pub fn read(pkt: &[u8]) -> Result<SettingSet, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SettingSet {
            key: SettingKey::of(integer::<u32>(f.at(0)?.get_number()?)?)
                .ok_or_else(|| unknown())?,
            val: integer::<u32>(f.at(1)?.get_number()?)?,
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

/// Command::PERF_RINGS's packet.
#[derive(Clone, Copy, Debug)]
pub struct PerfRings {
    pub ring: u32,
}

impl PerfRings {
    pub fn read(pkt: &[u8]) -> Result<PerfRings, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(PerfRings {
            ring: integer::<u32>(f.at(0)?.get_number()?)?,
        })
    }
}

/// Command::SENSOR_START's packet.
pub struct SensorStart {
    pub sensor: Sensor,
}

impl SensorStart {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.sensor as u32));
    }
}

impl xpute_core::io::door::Import for SensorStart {
    const NUMBER: u32 = 4609;
    const FIELDS: u32 = 1;
    const ACKREQ: bool = false;
    fn write(&self, w: &mut PacketWriter) {
        SensorStart::write(self, w);
    }
    fn named(&self) -> (u32, u32) {
        (0, 0)
    }
}

/// Command::SENSOR_STOP's packet.
pub struct SensorStop {
    pub sensor: Sensor,
}

impl SensorStop {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.sensor as u32));
    }
}

impl xpute_core::io::door::Import for SensorStop {
    const NUMBER: u32 = 4611;
    const FIELDS: u32 = 1;
    const ACKREQ: bool = false;
    fn write(&self, w: &mut PacketWriter) {
        SensorStop::write(self, w);
    }
    fn named(&self) -> (u32, u32) {
        (0, 0)
    }
}

/// Command::UI_SNAPSHOT's packet.
pub struct UiSnapshot {
    pub at: u32,
    pub len: u32,
}

impl UiSnapshot {
    /// The children it is written as.
    pub const FIELDS: u32 = 2;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.at));
        w.u32(Some(self.len));
    }
}

impl xpute_core::io::door::Import for UiSnapshot {
    const NUMBER: u32 = 6913;
    const FIELDS: u32 = 2;
    const ACKREQ: bool = false;
    fn write(&self, w: &mut PacketWriter) {
        UiSnapshot::write(self, w);
    }
    fn named(&self) -> (u32, u32) {
        (self.at, self.len)
    }
}

/// Command::UI_VIEW's packet.
pub struct UiView {
    pub azimuth: f64,
    /// The meters a logical pixel spans at the pivot, at the camera's distance; 0 where the camera stands there. The scale bar is the UI's to make of it; which division names show, the core's (`DivisionPart.shown`).
    pub m_per_px: f64,
}

impl UiView {
    /// The children it is written as.
    pub const FIELDS: u32 = 2;

    pub fn write(&self, w: &mut PacketWriter) {
        w.f64(Some(self.azimuth));
        w.f64(Some(self.m_per_px));
    }
}

impl xpute_core::io::door::Import for UiView {
    const NUMBER: u32 = 6914;
    const FIELDS: u32 = 2;
    const ACKREQ: bool = false;
    fn write(&self, w: &mut PacketWriter) {
        UiView::write(self, w);
    }
    fn named(&self) -> (u32, u32) {
        (0, 0)
    }
}

/// The ui protocol's core end: what it posts to the runtime and what it hears from it.
pub mod ui {
    use super::*;
    use xpute_core::io::door::{door, Outcome};
    use xpute_kit::status::errno::Errno;

    /// Command::SENSOR_START, posted for the runtime.
    pub fn sensor_start(e: &SensorStart) {
        door().send(e);
    }

    /// Command::SENSOR_STOP, posted for the runtime.
    pub fn sensor_stop(e: &SensorStop) {
        door().send(e);
    }

    /// Command::UI_SNAPSHOT, posted for the runtime.
    pub fn ui_snapshot(e: &UiSnapshot) {
        door().send(e);
    }

    /// Command::UI_VIEW, posted for the runtime.
    pub fn ui_view(e: &UiView) {
        door().send(e);
    }

    /// What the program does with each command the runtime sends.
    pub trait Exports {
        fn sensor_pos(&mut self, c: SensorPos) -> Outcome;
        fn sensor_orient(&mut self, c: SensorOrient) -> Outcome;
        fn sensor_err(&mut self, c: SensorErr) -> Outcome;
        fn source_declare(&mut self, c: SourceDeclare<'_>) -> Outcome;
        fn sensor_permission(&mut self, c: SensorPermission) -> Outcome;
        fn sensor_platform(&mut self, c: SensorPlatform) -> Outcome;
        fn setting_dev(&mut self, c: SettingDev) -> Outcome;
        fn input_pointer(&mut self, c: InputPointer) -> Outcome;
        fn input_typing(&mut self, c: InputTyping) -> Outcome;
        fn edit_tool(&mut self, c: EditTool) -> Outcome;
        fn edit_marker_text(&mut self, c: EditMarkerText<'_>) -> Outcome;
        fn edit_marker_cancel(&mut self) -> Outcome;
        fn edit_marker_commit(&mut self) -> Outcome;
        fn input_wheel(&mut self, c: InputWheel) -> Outcome;
        fn input_pinch(&mut self, c: InputPinch) -> Outcome;
        fn input_move(&mut self, c: InputMove) -> Outcome;
        fn scene_draw(&mut self, c: SceneDraw) -> Outcome;
        fn setting_set(&mut self, c: SettingSet) -> Outcome;
        fn scene_chat(&mut self, c: SceneChat<'_>) -> Outcome;
        fn camera_tracking(&mut self, c: CameraTracking) -> Outcome;
        fn camera_north_up(&mut self) -> Outcome;
        fn sensor_locate(&mut self) -> Outcome;
        fn camera_goto(&mut self, c: CameraGoto) -> Outcome;
        fn input_xr_pose(&mut self, c: InputXrPose) -> Outcome;
        fn input_xr_pose_end(&mut self) -> Outcome;
        fn perf_start(&mut self) -> Outcome;
        fn perf_stop(&mut self) -> Outcome;
        fn perf_stats(&mut self, c: PerfStats) -> Outcome;
        fn perf_rings(&mut self, c: PerfRings) -> Outcome;
        fn perf_strtab(&mut self) -> Outcome;
    }

    /// `cmd` read and handed to `h`; ENOSYS for one this protocol does not carry.
    pub fn apply<H: Exports>(h: &mut H, cmd: u32, pkt: &Option<&[u8]>) -> Outcome {
        match cmd {
            x if x == Command::SENSOR_POS as u32 => h.sensor_pos(SensorPos::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::SENSOR_ORIENT as u32 => h.sensor_orient(SensorOrient::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::SENSOR_ERR as u32 => h.sensor_err(SensorErr::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::SOURCE_DECLARE as u32 => h.source_declare(SourceDeclare::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::SENSOR_PERMISSION as u32 => h.sensor_permission(
                SensorPermission::read(pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?)?,
            ),
            x if x == Command::SENSOR_PLATFORM as u32 => h.sensor_platform(SensorPlatform::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::SETTING_DEV as u32 => h.setting_dev(SettingDev::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::INPUT_POINTER as u32 => h.input_pointer(InputPointer::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::INPUT_TYPING as u32 => h.input_typing(InputTyping::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::EDIT_TOOL as u32 => h.edit_tool(EditTool::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::EDIT_MARKER_TEXT as u32 => h.edit_marker_text(EditMarkerText::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::EDIT_MARKER_CANCEL as u32 => h.edit_marker_cancel(),
            x if x == Command::EDIT_MARKER_COMMIT as u32 => h.edit_marker_commit(),
            x if x == Command::INPUT_WHEEL as u32 => h.input_wheel(InputWheel::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::INPUT_PINCH as u32 => h.input_pinch(InputPinch::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::INPUT_MOVE as u32 => h.input_move(InputMove::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::SCENE_DRAW as u32 => h.scene_draw(SceneDraw::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::SETTING_SET as u32 => h.setting_set(SettingSet::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::SCENE_CHAT as u32 => h.scene_chat(SceneChat::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::CAMERA_TRACKING as u32 => h.camera_tracking(CameraTracking::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::CAMERA_NORTH_UP as u32 => h.camera_north_up(),
            x if x == Command::SENSOR_LOCATE as u32 => h.sensor_locate(),
            x if x == Command::CAMERA_GOTO as u32 => h.camera_goto(CameraGoto::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::INPUT_XR_POSE as u32 => h.input_xr_pose(InputXrPose::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::INPUT_XR_POSE_END as u32 => h.input_xr_pose_end(),
            x if x == Command::PERF_START as u32 => h.perf_start(),
            x if x == Command::PERF_STOP as u32 => h.perf_stop(),
            x if x == Command::PERF_STATS as u32 => h.perf_stats(PerfStats::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::PERF_RINGS as u32 => h.perf_rings(PerfRings::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            x if x == Command::PERF_STRTAB as u32 => h.perf_strtab(),
            _ => Err(MarshalError::new(Errno::ENOSYS)),
        }
    }
}
