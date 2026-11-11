// rust/client/core/src/door/cmd.runtime.spec.rs
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

pub use crate::domain::setting::schema_spec::SettingKey;
pub use crate::door::cmd::{
    CameraMinor, Command, EditMinor, GeolocationErrorCode, GroundStatus, InputMinor, Major,
    Message, PerfMinor, PermissionAnswer, PointerPhase, PointerType, SceneContent, SceneMinor,
    Sensor, SensorMinor, SensorStatus, SettingMinor, SourceMinor, Tool, Tracking, TurnMinor,
    UiMinor,
};
pub use pxl_spec::asset::character::Group as CharacterGroup;
use xpute_core::abi::number::{finite, integer, unknown};
use xpute_core::io::fs::{Path, Token};
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, StrList, TreeReader};
use xpute_runtime::abi::{Export, Outcome};

/// What the page's UI shows of the core (ui.rs): written after a turn that changed it, and replaced whole.
#[derive(Clone, Debug)]
pub struct Snapshot<'a>(BranchCursor<'a>);

impl<'a> Snapshot<'a> {
    pub fn read(pkt: &'a [u8]) -> Result<Snapshot<'a>, MarshalError> {
        Ok(Snapshot(TreeReader::new(pkt)?.read_branch()?))
    }

    pub fn of(c: Cursor<'a>) -> Result<Snapshot<'a>, MarshalError> {
        Ok(Snapshot(c.as_branch()?))
    }

    pub fn pivot(&self) -> Result<PivotState<'a>, MarshalError> {
        Ok({
            let c = self.0.at(0)?;
            PivotState::of(c)?
        })
    }

    pub fn location(&self) -> Result<LocationState<'a>, MarshalError> {
        Ok({
            let c = self.0.at(1)?;
            LocationState::of(c)?
        })
    }

    pub fn companion(&self) -> Result<CompanionState<'a>, MarshalError> {
        Ok({
            let c = self.0.at(2)?;
            CompanionState::of(c)?
        })
    }

    /// A marker's draft, its text so far, absent where none is open: opened by the marker tool's tap, with the marker's own text where one stands there, empty where it is new, and closed by EDIT_MARKER_COMMIT or EDIT_MARKER_CANCEL.
    pub fn draft(&self) -> Result<Option<&'a str>, MarshalError> {
        Ok(match self.0.at(3)?.opt() {
            Some(c) => Some(c.get_str_ref()?),
            None => None,
        })
    }

    pub fn orient(&self) -> Result<OrientationState<'a>, MarshalError> {
        Ok({
            let c = self.0.at(4)?;
            OrientationState::of(c)?
        })
    }

    pub fn is_follow_mode(&self) -> Result<bool, MarshalError> {
        Ok({
            let c = self.0.at(5)?;
            c.get_bool()?
        })
    }

    pub fn camera_busy(&self) -> Result<bool, MarshalError> {
        Ok({
            let c = self.0.at(6)?;
            c.get_bool()?
        })
    }

    /// A zoom is in hand: two fingers are down, or a wheel's or a trackpad pinch's zoom is still arriving. The scale is shown while it is.
    pub fn zooming(&self) -> Result<bool, MarshalError> {
        Ok({
            let c = self.0.at(7)?;
            c.get_bool()?
        })
    }

    /// The pointer is over a scene marker a tap would act on with the tool held: a place to go to, or a pin to erase or draft.
    pub fn over_marker(&self) -> Result<bool, MarshalError> {
        Ok({
            let c = self.0.at(8)?;
            c.get_bool()?
        })
    }

    /// Every setting's value by its SettingKey, as SETTING_SET carries one; none until the core
    /// has read its storage file, a few turns from boot.
    pub fn settings(&self) -> Result<&'a [u32], MarshalError> {
        Ok({
            let c = self.0.at(9)?;
            c.get_array::<u32>()?
        })
    }

    /// The division the pivot is in and every one above it, broad to narrow; empty
    /// where the pivot is in none or the tile it is read out of has not arrived.
    pub fn division(
        &self,
    ) -> Result<impl Iterator<Item = Result<DivisionPart<'a>, MarshalError>> + 'a, MarshalError>
    {
        Ok({
            let c = self.0.at(10)?;
            {
                let b = c.as_branch()?;
                (0..b.len).map(move |k| DivisionPart::of(b.at(k)?))
            }
        })
    }

    /// The surfaces a character is drawn on: its model and skin have arrived. The UI shows one loading until its surface is here.
    pub fn character_surfaces(&self) -> Result<&'a [u32], MarshalError> {
        Ok({
            let c = self.0.at(11)?;
            c.get_array::<u32>()?
        })
    }

    /// The files the core refused, not of their format, not decoding, or too large for any
    /// heap: a storage file (file.json's `storage`) or a placed objects' file (`object`). The
    /// core read none of it and writes nothing over it until FS_CHANGED says it is
    /// empty.
    pub fn refused_files(
        &self,
    ) -> Result<impl Iterator<Item = Result<RefusedFile<'a>, MarshalError>> + 'a, MarshalError>
    {
        Ok({
            let c = self.0.at(12)?;
            {
                let b = c.as_branch()?;
                (0..b.len).map(move |k| RefusedFile::of(b.at(k)?))
            }
        })
    }

    /// Why the server does not answer the world's reads now: failing since the last answered, or given up. Absent while it answers. The network and the GPU are the runtime's to say.
    pub fn stall(&self) -> Result<Option<Message>, MarshalError> {
        Ok(match self.0.at(13)?.opt() {
            Some(c) => {
                Some(Message::of(integer::<u32>(c.get_number()?)?).ok_or_else(|| unknown())?)
            }
            None => None,
        })
    }

    /// How much of what the renderer makes before its first frame is made, 0 to 1 (pxl-render gpu/pipeline.rs `PipelineCache::share`): the pipelines made of those asked for, in whatever order they finish, 0 before any is asked; 1 once it draws, 0 again while a lost device's next renderer is made. The loading bar is the UI's to draw of it.
    pub fn renderer_progress(&self) -> Result<f64, MarshalError> {
        Ok({
            let c = self.0.at(14)?;
            finite(c.get_number()?)?
        })
    }

    /// How much of the view's ground shows, 0 to 1: of the tiles asked for, those with an answer or drawn coarser. 0 where none is asked yet, 1 once the area is shown; it goes back as more is asked for.
    pub fn area_progress(&self) -> Result<f64, MarshalError> {
        Ok({
            let c = self.0.at(15)?;
            finite(c.get_number()?)?
        })
    }
}

#[derive(Clone, Debug)]
pub struct PivotState<'a>(BranchCursor<'a>);

impl<'a> PivotState<'a> {
    pub fn read(pkt: &'a [u8]) -> Result<PivotState<'a>, MarshalError> {
        Ok(PivotState(TreeReader::new(pkt)?.read_branch()?))
    }

    pub fn of(c: Cursor<'a>) -> Result<PivotState<'a>, MarshalError> {
        Ok(PivotState(c.as_branch()?))
    }

    pub fn status(&self) -> Result<GroundStatus, MarshalError> {
        Ok({
            let c = self.0.at(0)?;
            GroundStatus::of(integer::<u32>(c.get_number()?)?).ok_or_else(|| unknown())?
        })
    }

    pub fn is_off_center(&self) -> Result<bool, MarshalError> {
        Ok({
            let c = self.0.at(1)?;
            c.get_bool()?
        })
    }

    /// The pivot in degrees: what the search measures distance from.
    pub fn lon_lat(&self) -> Result<Option<LonLat<'a>>, MarshalError> {
        Ok(match self.0.at(2)?.opt() {
            Some(c) => Some(LonLat::of(c)?),
            None => None,
        })
    }

    /// The pivot is past the service area: the source serves no shard over it (pxl-core sys/io/source.rs).
    pub fn outside(&self) -> Result<bool, MarshalError> {
        Ok({
            let c = self.0.at(3)?;
            c.get_bool()?
        })
    }
}

#[derive(Clone, Debug)]
pub struct LonLat<'a>(BranchCursor<'a>);

impl<'a> LonLat<'a> {
    pub fn read(pkt: &'a [u8]) -> Result<LonLat<'a>, MarshalError> {
        Ok(LonLat(TreeReader::new(pkt)?.read_branch()?))
    }

    pub fn of(c: Cursor<'a>) -> Result<LonLat<'a>, MarshalError> {
        Ok(LonLat(c.as_branch()?))
    }

    pub fn lon(&self) -> Result<f64, MarshalError> {
        Ok({
            let c = self.0.at(0)?;
            finite(c.get_number()?)?
        })
    }

    pub fn lat(&self) -> Result<f64, MarshalError> {
        Ok({
            let c = self.0.at(1)?;
            finite(c.get_number()?)?
        })
    }
}

#[derive(Clone, Debug)]
pub struct LocationState<'a>(BranchCursor<'a>);

impl<'a> LocationState<'a> {
    pub fn read(pkt: &'a [u8]) -> Result<LocationState<'a>, MarshalError> {
        Ok(LocationState(TreeReader::new(pkt)?.read_branch()?))
    }

    pub fn of(c: Cursor<'a>) -> Result<LocationState<'a>, MarshalError> {
        Ok(LocationState(c.as_branch()?))
    }

    pub fn status(&self) -> Result<SensorStatus, MarshalError> {
        Ok({
            let c = self.0.at(0)?;
            SensorStatus::of(integer::<u32>(c.get_number()?)?).ok_or_else(|| unknown())?
        })
    }

    pub fn has_location(&self) -> Result<bool, MarshalError> {
        Ok({
            let c = self.0.at(1)?;
            c.get_bool()?
        })
    }

    /// Whether a watch runs, so that asking for the location again would start nothing.
    pub fn watching(&self) -> Result<bool, MarshalError> {
        Ok({
            let c = self.0.at(2)?;
            c.get_bool()?
        })
    }

    /// Present exactly when status is Error: the failure the core chose.
    pub fn err(&self) -> Result<Option<Message>, MarshalError> {
        Ok(match self.0.at(3)?.opt() {
            Some(c) => {
                Some(Message::of(integer::<u32>(c.get_number()?)?).ok_or_else(|| unknown())?)
            }
            None => None,
        })
    }
}

#[derive(Clone, Debug)]
pub struct CompanionState<'a>(BranchCursor<'a>);

impl<'a> CompanionState<'a> {
    pub fn read(pkt: &'a [u8]) -> Result<CompanionState<'a>, MarshalError> {
        Ok(CompanionState(TreeReader::new(pkt)?.read_branch()?))
    }

    pub fn of(c: Cursor<'a>) -> Result<CompanionState<'a>, MarshalError> {
        Ok(CompanionState(c.as_branch()?))
    }

    pub fn person_ready(&self) -> Result<bool, MarshalError> {
        Ok({
            let c = self.0.at(0)?;
            c.get_bool()?
        })
    }

    pub fn pet_ready(&self) -> Result<bool, MarshalError> {
        Ok({
            let c = self.0.at(1)?;
            c.get_bool()?
        })
    }
}

#[derive(Clone, Debug)]
pub struct OrientationState<'a>(BranchCursor<'a>);

impl<'a> OrientationState<'a> {
    pub fn read(pkt: &'a [u8]) -> Result<OrientationState<'a>, MarshalError> {
        Ok(OrientationState(TreeReader::new(pkt)?.read_branch()?))
    }

    pub fn of(c: Cursor<'a>) -> Result<OrientationState<'a>, MarshalError> {
        Ok(OrientationState(c.as_branch()?))
    }

    pub fn status(&self) -> Result<SensorStatus, MarshalError> {
        Ok({
            let c = self.0.at(0)?;
            SensorStatus::of(integer::<u32>(c.get_number()?)?).ok_or_else(|| unknown())?
        })
    }
}

/// One division of the name, in its own name.
#[derive(Clone, Debug)]
pub struct DivisionPart<'a>(BranchCursor<'a>);

impl<'a> DivisionPart<'a> {
    pub fn read(pkt: &'a [u8]) -> Result<DivisionPart<'a>, MarshalError> {
        Ok(DivisionPart(TreeReader::new(pkt)?.read_branch()?))
    }

    pub fn of(c: Cursor<'a>) -> Result<DivisionPart<'a>, MarshalError> {
        Ok(DivisionPart(c.as_branch()?))
    }

    /// Overture's subtype, broadest first (spec/spec/division.json).
    pub fn subtype(&self) -> Result<u8, MarshalError> {
        Ok({
            let c = self.0.at(0)?;
            integer::<u8>(c.get_number()?)?
        })
    }

    pub fn name(&self) -> Result<&'a str, MarshalError> {
        Ok({
            let c = self.0.at(1)?;
            c.get_str_ref()?
        })
    }

    /// The view is in far enough for its name to show: as far as Overture's own tiles
    /// show it from (pxl-spec bake/tile/division.rs `Division::shown_from`).
    pub fn shown(&self) -> Result<bool, MarshalError> {
        Ok({
            let c = self.0.at(2)?;
            c.get_bool()?
        })
    }

    /// Held by an area the point is in; one without is the nearest found by its point.
    pub fn area(&self) -> Result<bool, MarshalError> {
        Ok({
            let c = self.0.at(3)?;
            c.get_bool()?
        })
    }
}

#[derive(Clone, Debug)]
pub struct RefusedFile<'a>(BranchCursor<'a>);

impl<'a> RefusedFile<'a> {
    pub fn read(pkt: &'a [u8]) -> Result<RefusedFile<'a>, MarshalError> {
        Ok(RefusedFile(TreeReader::new(pkt)?.read_branch()?))
    }

    pub fn of(c: Cursor<'a>) -> Result<RefusedFile<'a>, MarshalError> {
        Ok(RefusedFile(c.as_branch()?))
    }

    /// Its path, as the core opened it (file.json `path`).
    pub fn path(&self) -> Result<Path<'a>, MarshalError> {
        let pkt = self.0.cursor.pkt;
        Ok({
            let c = self.0.at(0)?;
            Path::read(pkt, c)?
        })
    }
}

/// Command::SENSOR_POS's packet.
pub struct SensorPos {
    pub lon: f64,
    pub lat: f64,
    pub alt: Option<f64>,
    pub accuracy: Option<f64>,
    pub course: Option<f64>,
}

impl SensorPos {
    /// The children it is written as.
    pub const FIELDS: u32 = 5;

    pub fn write(&self, w: &mut PacketWriter) {
        w.f64(Some(self.lon));
        w.f64(Some(self.lat));
        w.f64(self.alt);
        w.f64(self.accuracy);
        w.f64(self.course);
    }
}

impl Export for SensorPos {
    const NUMBER: u32 = 4610;
    const FIELDS: u32 = 5;
    fn write(&self, w: &mut PacketWriter) {
        SensorPos::write(self, w);
    }
}

/// Command::SENSOR_ORIENT's packet.
pub struct SensorOrient {
    pub webkit_compass_heading: Option<f64>,
    pub alpha: Option<f64>,
    pub beta: Option<f64>,
    pub gamma: Option<f64>,
}

impl SensorOrient {
    /// The children it is written as.
    pub const FIELDS: u32 = 4;

    pub fn write(&self, w: &mut PacketWriter) {
        w.f64(self.webkit_compass_heading);
        w.f64(self.alpha);
        w.f64(self.beta);
        w.f64(self.gamma);
    }
}

impl Export for SensorOrient {
    const NUMBER: u32 = 4613;
    const FIELDS: u32 = 4;
    fn write(&self, w: &mut PacketWriter) {
        SensorOrient::write(self, w);
    }
}

/// Command::SENSOR_ERR's packet.
pub struct SensorErr {
    pub sensor: Sensor,
    /// Geolocation's; orientation gives none.
    pub code: Option<GeolocationErrorCode>,
    /// Whether the watch runs on past it, which the platform alone knows. A compass that
    /// is lost runs no more.
    pub watching: bool,
}

impl SensorErr {
    /// The children it is written as.
    pub const FIELDS: u32 = 3;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.sensor as u32));
        w.u32(self.code.map(|m| m as u32));
        w.bool(Some(self.watching));
    }
}

impl Export for SensorErr {
    const NUMBER: u32 = 4619;
    const FIELDS: u32 = 3;
    fn write(&self, w: &mut PacketWriter) {
        SensorErr::write(self, w);
    }
}

/// Command::SOURCE_DECLARE's packet.
pub struct SourceDeclare<'a> {
    /// The coarsest zoom the source supplies whole.
    pub min_zoom: u32,
    /// Each kind's format, one a kind, indexed by its bake's number
    /// (pxl-spec bake/publication.spec.rs).
    pub format: &'a [u32],
    /// The shards it serves, by name. Their order means nothing: no two at a zoom
    /// are over one node, and the finer wins.
    pub shard: &'a [&'a str],
}

impl SourceDeclare<'_> {
    /// The children it is written as.
    pub const FIELDS: u32 = 3;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.min_zoom));
        w.u32_array(Some(self.format));
        w.strs(Some(self.shard));
    }
}

impl Export for SourceDeclare<'_> {
    const NUMBER: u32 = 4865;
    const FIELDS: u32 = 3;
    fn write(&self, w: &mut PacketWriter) {
        SourceDeclare::write(self, w);
    }
}

/// Command::SENSOR_PERMISSION's packet.
pub struct SensorPermission {
    pub sensor: Sensor,
    pub answer: PermissionAnswer,
}

impl SensorPermission {
    /// The children it is written as.
    pub const FIELDS: u32 = 2;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.sensor as u32));
        w.u32(Some(self.answer as u32));
    }
}

impl Export for SensorPermission {
    const NUMBER: u32 = 4614;
    const FIELDS: u32 = 2;
    fn write(&self, w: &mut PacketWriter) {
        SensorPermission::write(self, w);
    }
}

/// Command::SENSOR_PLATFORM's packet.
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
    /// The children it is written as.
    pub const FIELDS: u32 = 4;

    pub fn write(&self, w: &mut PacketWriter) {
        w.bool(Some(self.geolocation));
        w.bool(Some(self.orient));
        w.bool(Some(self.orient_permission));
        w.bool(Some(self.orient_silence_is_loss));
    }
}

impl Export for SensorPlatform {
    const NUMBER: u32 = 4621;
    const FIELDS: u32 = 4;
    fn write(&self, w: &mut PacketWriter) {
        SensorPlatform::write(self, w);
    }
}

/// Command::SETTING_DEV's packet.
pub struct SettingDev {
    pub dev: bool,
}

impl SettingDev {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.bool(Some(self.dev));
    }
}

impl Export for SettingDev {
    const NUMBER: u32 = 5634;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        SettingDev::write(self, w);
    }
}

/// Command::INPUT_POINTER's packet.
pub struct InputPointer {
    pub phase: PointerPhase,
    pub pointer_type: PointerType,
    pub id: i32,
    pub button: i32,
    pub x: f64,
    pub y: f64,
}

impl InputPointer {
    /// The children it is written as.
    pub const FIELDS: u32 = 6;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.phase as u32));
        w.u32(Some(self.pointer_type as u32));
        w.i32(Some(self.id));
        w.i32(Some(self.button));
        w.f64(Some(self.x));
        w.f64(Some(self.y));
    }
}

impl Export for InputPointer {
    const NUMBER: u32 = 4354;
    const FIELDS: u32 = 6;
    fn write(&self, w: &mut PacketWriter) {
        InputPointer::write(self, w);
    }
}

/// Command::INPUT_TYPING's packet.
pub struct InputTyping {
    pub typing: bool,
}

impl InputTyping {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.bool(Some(self.typing));
    }
}

impl Export for InputTyping {
    const NUMBER: u32 = 4363;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        InputTyping::write(self, w);
    }
}

/// Command::EDIT_TOOL's packet.
pub struct EditTool {
    pub tool: Tool,
    /// The item to place, its id (id.json); read only for that tool.
    pub val: u32,
    /// A cube's scale, as a decor's `scale_steps`: 0 a unit, each step down its half, to 1 - OBJECT_SCALES (pxl-spec bake/object.spec.rs). Read only for an item, and only a cube's is chosen.
    pub scale_steps: i32,
}

impl EditTool {
    /// The children it is written as.
    pub const FIELDS: u32 = 3;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.tool as u32));
        w.u32(Some(self.val));
        w.i32(Some(self.scale_steps));
    }
}

impl Export for EditTool {
    const NUMBER: u32 = 6657;
    const FIELDS: u32 = 3;
    fn write(&self, w: &mut PacketWriter) {
        EditTool::write(self, w);
    }
}

/// Command::EDIT_MARKER_TEXT's packet.
pub struct EditMarkerText<'a> {
    pub text: &'a dyn Display,
}

impl EditMarkerText<'_> {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.str_display(self.text);
    }
}

impl Export for EditMarkerText<'_> {
    const NUMBER: u32 = 6658;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        EditMarkerText::write(self, w);
    }
}

/// Command::EDIT_MARKER_CANCEL's packet.
pub struct EditMarkerCancel {}

impl EditMarkerCancel {
    /// The children it is written as.
    pub const FIELDS: u32 = 0;

    pub fn write(&self, _: &mut PacketWriter) {}
}

impl Export for EditMarkerCancel {
    const NUMBER: u32 = 6659;
    const FIELDS: u32 = 0;
    fn write(&self, w: &mut PacketWriter) {
        EditMarkerCancel::write(self, w);
    }
}

/// Command::EDIT_MARKER_COMMIT's packet.
pub struct EditMarkerCommit {}

impl EditMarkerCommit {
    /// The children it is written as.
    pub const FIELDS: u32 = 0;

    pub fn write(&self, _: &mut PacketWriter) {}
}

impl Export for EditMarkerCommit {
    const NUMBER: u32 = 6660;
    const FIELDS: u32 = 0;
    fn write(&self, w: &mut PacketWriter) {
        EditMarkerCommit::write(self, w);
    }
}

/// Command::INPUT_WHEEL's packet.
pub struct InputWheel {
    pub delta_y: f64,
}

impl InputWheel {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.f64(Some(self.delta_y));
    }
}

impl Export for InputWheel {
    const NUMBER: u32 = 4355;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        InputWheel::write(self, w);
    }
}

/// Command::INPUT_PINCH's packet.
pub struct InputPinch {
    pub scale: f64,
}

impl InputPinch {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.f64(Some(self.scale));
    }
}

impl Export for InputPinch {
    const NUMBER: u32 = 4364;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        InputPinch::write(self, w);
    }
}

/// Command::INPUT_MOVE's packet.
pub struct InputMove {
    pub x: i32,
    pub y: i32,
}

impl InputMove {
    /// The children it is written as.
    pub const FIELDS: u32 = 2;

    pub fn write(&self, w: &mut PacketWriter) {
        w.i32(Some(self.x));
        w.i32(Some(self.y));
    }
}

impl Export for InputMove {
    const NUMBER: u32 = 4358;
    const FIELDS: u32 = 2;
    fn write(&self, w: &mut PacketWriter) {
        InputMove::write(self, w);
    }
}

/// Command::SCENE_DRAW's packet.
pub struct SceneDraw {
    pub surface: u32,
    pub content: SceneContent,
    pub group: Option<CharacterGroup>,
    /// What the character is drawn over, 0xRRGGBB, sRGB.
    pub bg: Option<u32>,
}

impl SceneDraw {
    /// The children it is written as.
    pub const FIELDS: u32 = 4;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.surface));
        w.u32(Some(self.content as u32));
        w.u32(self.group.map(|m| m as u32));
        w.u32(self.bg);
    }
}

impl Export for SceneDraw {
    const NUMBER: u32 = 5892;
    const FIELDS: u32 = 4;
    fn write(&self, w: &mut PacketWriter) {
        SceneDraw::write(self, w);
    }
}

/// Command::SETTING_SET's packet.
pub struct SettingSet {
    pub key: SettingKey,
    /// A toggle's 0 or 1, a choice's option by its index, or a number.
    pub val: u32,
}

impl SettingSet {
    /// The children it is written as.
    pub const FIELDS: u32 = 2;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.key as u32));
        w.u32(Some(self.val));
    }
}

impl Export for SettingSet {
    const NUMBER: u32 = 5633;
    const FIELDS: u32 = 2;
    fn write(&self, w: &mut PacketWriter) {
        SettingSet::write(self, w);
    }
}

/// Command::SCENE_CHAT's packet.
pub struct SceneChat<'a> {
    pub text: &'a dyn Display,
}

impl SceneChat<'_> {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.str_display(self.text);
    }
}

impl Export for SceneChat<'_> {
    const NUMBER: u32 = 5891;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        SceneChat::write(self, w);
    }
}

/// Command::CAMERA_TRACKING's packet.
pub struct CameraTracking {
    pub tracking: Tracking,
}

impl CameraTracking {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.tracking as u32));
    }
}

impl Export for CameraTracking {
    const NUMBER: u32 = 6145;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        CameraTracking::write(self, w);
    }
}

/// Command::CAMERA_NORTH_UP's packet.
pub struct CameraNorthUp {}

impl CameraNorthUp {
    /// The children it is written as.
    pub const FIELDS: u32 = 0;

    pub fn write(&self, _: &mut PacketWriter) {}
}

impl Export for CameraNorthUp {
    const NUMBER: u32 = 6146;
    const FIELDS: u32 = 0;
    fn write(&self, w: &mut PacketWriter) {
        CameraNorthUp::write(self, w);
    }
}

/// Command::SENSOR_LOCATE's packet.
pub struct SensorLocate {}

impl SensorLocate {
    /// The children it is written as.
    pub const FIELDS: u32 = 0;

    pub fn write(&self, _: &mut PacketWriter) {}
}

impl Export for SensorLocate {
    const NUMBER: u32 = 4620;
    const FIELDS: u32 = 0;
    fn write(&self, w: &mut PacketWriter) {
        SensorLocate::write(self, w);
    }
}

/// Command::CAMERA_GOTO's packet.
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
    /// The children it is written as.
    pub const FIELDS: u32 = 5;

    pub fn write(&self, w: &mut PacketWriter) {
        w.f64(Some(self.lon));
        w.f64(Some(self.lat));
        w.f64(self.dist);
        w.f64(self.azimuth);
        w.f64(self.polar);
    }
}

impl Export for CameraGoto {
    const NUMBER: u32 = 6148;
    const FIELDS: u32 = 5;
    fn write(&self, w: &mut PacketWriter) {
        CameraGoto::write(self, w);
    }
}

/// Command::INPUT_XR_POSE's packet.
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
    /// The children it is written as.
    pub const FIELDS: u32 = 7;

    pub fn write(&self, w: &mut PacketWriter) {
        w.f64(Some(self.x));
        w.f64(Some(self.y));
        w.f64(Some(self.z));
        w.f64(Some(self.qx));
        w.f64(Some(self.qy));
        w.f64(Some(self.qz));
        w.f64(Some(self.qw));
    }
}

impl Export for InputXrPose {
    const NUMBER: u32 = 4361;
    const FIELDS: u32 = 7;
    fn write(&self, w: &mut PacketWriter) {
        InputXrPose::write(self, w);
    }
}

/// Command::INPUT_XR_POSE_END's packet.
pub struct InputXrPoseEnd {}

impl InputXrPoseEnd {
    /// The children it is written as.
    pub const FIELDS: u32 = 0;

    pub fn write(&self, _: &mut PacketWriter) {}
}

impl Export for InputXrPoseEnd {
    const NUMBER: u32 = 4362;
    const FIELDS: u32 = 0;
    fn write(&self, w: &mut PacketWriter) {
        InputXrPoseEnd::write(self, w);
    }
}

/// Command::PERF_START's packet.
pub struct PerfStart {}

impl PerfStart {
    /// The children it is written as.
    pub const FIELDS: u32 = 0;

    pub fn write(&self, _: &mut PacketWriter) {}
}

impl Export for PerfStart {
    const NUMBER: u32 = 6401;
    const FIELDS: u32 = 0;
    fn write(&self, w: &mut PacketWriter) {
        PerfStart::write(self, w);
    }
}

/// Command::PERF_STOP's packet.
pub struct PerfStop {}

impl PerfStop {
    /// The children it is written as.
    pub const FIELDS: u32 = 0;

    pub fn write(&self, _: &mut PacketWriter) {}
}

impl Export for PerfStop {
    const NUMBER: u32 = 6402;
    const FIELDS: u32 = 0;
    fn write(&self, w: &mut PacketWriter) {
        PerfStop::write(self, w);
    }
}

/// Command::PERF_STATS's packet.
pub struct PerfStats {
    pub window_ms: Option<f64>,
}

impl PerfStats {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.f64(self.window_ms);
    }
}

impl Export for PerfStats {
    const NUMBER: u32 = 6403;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        PerfStats::write(self, w);
    }
}

/// Command::PERF_RINGS's packet.
pub struct PerfRings {
    pub ring: u32,
}

impl PerfRings {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.ring));
    }
}

impl Export for PerfRings {
    const NUMBER: u32 = 6404;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        PerfRings::write(self, w);
    }
}

/// Command::PERF_STRTAB's packet.
pub struct PerfStrtab {}

impl PerfStrtab {
    /// The children it is written as.
    pub const FIELDS: u32 = 0;

    pub fn write(&self, _: &mut PacketWriter) {}
}

impl Export for PerfStrtab {
    const NUMBER: u32 = 6405;
    const FIELDS: u32 = 0;
    fn write(&self, w: &mut PacketWriter) {
        PerfStrtab::write(self, w);
    }
}

/// Command::SENSOR_START's packet.
#[derive(Clone, Copy, Debug)]
pub struct SensorStart {
    pub sensor: Sensor,
}

impl SensorStart {
    pub fn read(pkt: &[u8]) -> Result<SensorStart, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SensorStart {
            sensor: Sensor::of(integer::<u32>(f.at(0)?.get_number()?)?).ok_or_else(|| unknown())?,
        })
    }
}

/// Command::SENSOR_STOP's packet.
#[derive(Clone, Copy, Debug)]
pub struct SensorStop {
    pub sensor: Sensor,
}

impl SensorStop {
    pub fn read(pkt: &[u8]) -> Result<SensorStop, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SensorStop {
            sensor: Sensor::of(integer::<u32>(f.at(0)?.get_number()?)?).ok_or_else(|| unknown())?,
        })
    }
}

/// Command::UI_SNAPSHOT's packet.
#[derive(Clone, Copy, Debug)]
pub struct UiSnapshot {
    pub at: u32,
    pub len: u32,
}

impl UiSnapshot {
    pub fn read(pkt: &[u8]) -> Result<UiSnapshot, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(UiSnapshot {
            at: integer::<u32>(f.at(0)?.get_number()?)?,
            len: integer::<u32>(f.at(1)?.get_number()?)?,
        })
    }
}

/// Command::UI_VIEW's packet.
#[derive(Clone, Copy, Debug)]
pub struct UiView {
    pub azimuth: f64,
    /// The meters a logical pixel spans at the pivot, at the camera's distance; 0 where the camera stands there. The scale bar is the UI's to make of it; which division names show, the core's (`DivisionPart.shown`).
    pub m_per_px: f64,
}

impl UiView {
    pub fn read(pkt: &[u8]) -> Result<UiView, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(UiView {
            azimuth: finite(f.at(0)?.get_number()?)?,
            m_per_px: finite(f.at(1)?.get_number()?)?,
        })
    }
}

/// The majors the runtime routes the protocol's imports by, to `serve`.
pub const MAJORS: [u32; 2] = [Major::SENSOR as u32, Major::UI as u32];

/// What the runtime does with each command the core posts.
pub trait Imports {
    fn sensor_start(&mut self, e: SensorStart) -> Outcome;
    fn sensor_stop(&mut self, e: SensorStop) -> Outcome;
    fn ui_snapshot(&mut self, e: UiSnapshot, named: &[u8]) -> Outcome;
    fn ui_view(&mut self, e: UiView) -> Outcome;
}

/// `cmd` read and handed to `h`; ENOSYS for one this protocol does not carry.
pub fn serve<H: Imports>(
    h: &mut H,
    _tag: u32,
    cmd: u32,
    pkt: &Option<&[u8]>,
    named: &[u8],
) -> Outcome {
    match cmd {
        x if x == Command::SENSOR_START as u32 => h.sensor_start(SensorStart::read(
            pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
        )?),
        x if x == Command::SENSOR_STOP as u32 => h.sensor_stop(SensorStop::read(
            pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
        )?),
        x if x == Command::UI_SNAPSHOT as u32 => h.ui_snapshot(
            UiSnapshot::read(pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?)?,
            named,
        ),
        x if x == Command::UI_VIEW as u32 => h.ui_view(UiView::read(
            pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
        )?),
        _ => Err(MarshalError::new(Errno::ENOSYS)),
    }
}
