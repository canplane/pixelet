// ts/client/src/abi/cmd.spec.ts
//
// GENERATED from spec/client/core/door/ui.json

/**
 * The UI's protocol with the core: its commands, by number (@xpute/runtime io/frame.ts `CMD`:
 * `major << 8 | minor`, past xpute's majors 0x00 to 0x0F), each with the packet
 * it carries — an XTP branch whose children are its fields, in order. A command with no
 * fields carries no packet. The JavaScript around the core is two
 * things to it, its runtime, which holds the turns and carries its imports to what
 * answers them (ts/client/src/service/sys/), and its UI, which sends what the user
 * does and what the place's sensors, visibility and frames tell it
 * (ts/client/src/service/core/). A command's major is the module it is for.
 *
 * `export` is what the UI sends the core and `import` what the core posts to it, `call`
 * one its sender waits on. Each is written and read by what this file generates, on
 * both sides, so no field is found by its position anywhere else.
 *
 * A field is a number of a width, a `bool`, a `str`, a member of an enum
 * below or of one another spec declares (`extern`), or a record below. An
 * `optional` one may be absent, its kind kept; a `list` is a branch of them,
 * or a packed array where the item is a number. A record crosses as a branch
 * of its own fields, and is how a block past a slot is laid out. An enum's
 * `unknown` is the member a number it does not give out reads as; an enum
 * without one refuses the packet. The generator refuses a number given twice.
 */

import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import type { U32Array } from "@xpute/kit/abi/array.ts";
import type { f64, i32, u32, u8 } from "@xpute/kit/abi/word.ts";
import { FINITE, INT_U32, INT_U8, MEMBER } from "@xpute/runtime/abi/number.ts";
import type { Path } from "@xpute/runtime/io/path.ts";
import { read_path } from "@xpute/runtime/io/path.ts";
import * as Major from "./cmd/major.spec.ts";
import * as InputMinor from "./cmd/input_minor.spec.ts";
import * as SensorMinor from "./cmd/sensor_minor.spec.ts";
import * as SourceMinor from "./cmd/source_minor.spec.ts";
import * as TurnMinor from "./cmd/turn_minor.spec.ts";
import * as SettingMinor from "./cmd/setting_minor.spec.ts";
import * as SceneMinor from "./cmd/scene_minor.spec.ts";
import * as CameraMinor from "./cmd/camera_minor.spec.ts";
import * as PerfMinor from "./cmd/perf_minor.spec.ts";
import * as EditMinor from "./cmd/edit_minor.spec.ts";
import * as UiMinor from "./cmd/ui_minor.spec.ts";
import * as PointerPhase from "./cmd/pointer_phase.spec.ts";
import * as PointerType from "./cmd/pointer_type.spec.ts";
import * as Tool from "./cmd/tool.spec.ts";
import * as SceneContent from "./cmd/scene_content.spec.ts";
import * as Tracking from "./cmd/tracking.spec.ts";
import * as GeolocationErrorCode from "./cmd/geolocation_error_code.spec.ts";
import * as Sensor from "./cmd/sensor.spec.ts";
import * as PermissionAnswer from "./cmd/permission_answer.spec.ts";
import * as GroundStatus from "./cmd/ground_status.spec.ts";
import * as SensorStatus from "./cmd/sensor_status.spec.ts";
import * as Message from "./cmd/message.spec.ts";
import * as Command from "./cmd/command.spec.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import type { Link, Protocol, OnImport } from "@xpute/runtime";
import { refusal_of } from "@xpute/runtime";

/** One of Major's, as its enum was a type. */
type Major = (typeof Major)[keyof typeof Major];
export { Major };

/** One of InputMinor's, as its enum was a type. */
type InputMinor = (typeof InputMinor)[keyof typeof InputMinor];
export { InputMinor };

/** One of SensorMinor's, as its enum was a type. */
type SensorMinor = (typeof SensorMinor)[keyof typeof SensorMinor];
export { SensorMinor };

/** One of SourceMinor's, as its enum was a type. */
type SourceMinor = (typeof SourceMinor)[keyof typeof SourceMinor];
export { SourceMinor };

/** One of TurnMinor's, as its enum was a type. */
type TurnMinor = (typeof TurnMinor)[keyof typeof TurnMinor];
export { TurnMinor };

/** One of SettingMinor's, as its enum was a type. */
type SettingMinor = (typeof SettingMinor)[keyof typeof SettingMinor];
export { SettingMinor };

/** One of SceneMinor's, as its enum was a type. */
type SceneMinor = (typeof SceneMinor)[keyof typeof SceneMinor];
export { SceneMinor };

/** One of CameraMinor's, as its enum was a type. */
type CameraMinor = (typeof CameraMinor)[keyof typeof CameraMinor];
export { CameraMinor };

/** One of PerfMinor's, as its enum was a type. */
type PerfMinor = (typeof PerfMinor)[keyof typeof PerfMinor];
export { PerfMinor };

/** One of EditMinor's, as its enum was a type. */
type EditMinor = (typeof EditMinor)[keyof typeof EditMinor];
export { EditMinor };

/** One of UiMinor's, as its enum was a type. */
type UiMinor = (typeof UiMinor)[keyof typeof UiMinor];
export { UiMinor };

/** One of PointerPhase's, as its enum was a type. */
type PointerPhase = (typeof PointerPhase)[keyof typeof PointerPhase];
export { PointerPhase };

/** One of PointerType's, as its enum was a type. */
type PointerType = (typeof PointerType)[keyof typeof PointerType];
export { PointerType };

/** One of Tool's, as its enum was a type. */
type Tool = (typeof Tool)[keyof typeof Tool];
export { Tool };

/** One of SceneContent's, as its enum was a type. */
type SceneContent = (typeof SceneContent)[keyof typeof SceneContent];
export { SceneContent };

/** One of Tracking's, as its enum was a type. */
type Tracking = (typeof Tracking)[keyof typeof Tracking];
export { Tracking };

/** One of GeolocationErrorCode's, as its enum was a type. */
type GeolocationErrorCode =
  (typeof GeolocationErrorCode)[keyof typeof GeolocationErrorCode];
export { GeolocationErrorCode };

/** One of Sensor's, as its enum was a type. */
type Sensor = (typeof Sensor)[keyof typeof Sensor];
export { Sensor };

/** One of PermissionAnswer's, as its enum was a type. */
type PermissionAnswer =
  (typeof PermissionAnswer)[keyof typeof PermissionAnswer];
export { PermissionAnswer };

/** One of GroundStatus's, as its enum was a type. */
type GroundStatus = (typeof GroundStatus)[keyof typeof GroundStatus];
export { GroundStatus };

/** One of SensorStatus's, as its enum was a type. */
type SensorStatus = (typeof SensorStatus)[keyof typeof SensorStatus];
export { SensorStatus };

/** One of Message's, as its enum was a type. */
type Message = (typeof Message)[keyof typeof Message];
export { Message };

/** One of Command's, as its enum was a type. */
type Command = (typeof Command)[keyof typeof Command];
export { Command };

/** A branch of records, each read by `read`. */
function list<T>(b: BranchCursor, read: (b: BranchCursor) => T): T[] {
  return Array.from({ length: b.len }, (_, k) => read(b.at_branch(k)));
}

/** What the page's UI shows of the core (ui.rs): written after a turn that changed it, and replaced whole. */
export interface Snapshot {
  pivot: PivotState;
  location: LocationState;
  companion: CompanionState;
  /** A marker's draft, its text so far, absent where none is open: opened by the marker tool's tap, with the marker's own text where one stands there, empty where it is new, and closed by EDIT_MARKER_COMMIT or EDIT_MARKER_CANCEL. */
  draft: string | null;
  orient: OrientationState;
  is_follow_mode: boolean;
  camera_busy: boolean;
  /** A zoom is in hand: two fingers are down, or a wheel's or a trackpad pinch's zoom is still arriving. The scale is shown while it is. */
  zooming: boolean;
  /** The pointer is over a scene marker a tap would act on with the tool held: a place to go to, or a pin to erase or draft. */
  over_marker: boolean;
  /** Every setting's value by its SettingKey, as SETTING_SET carries one; none until the core
   * has read its storage file, a few turns from boot. */
  settings: u32[];
  /** The division the pivot is in and every one above it, broad to narrow; empty
   * where the pivot is in none or the tile it is read out of has not arrived. */
  division: DivisionPart[];
  /** The surfaces a character is drawn on: its model and skin have arrived. The UI shows one loading until its surface is here. */
  character_surfaces: u32[];
  /** The files the core refused, not of their format, not decoding, or too large for any
   * heap: a storage file (file.json's `storage`) or a placed objects' file (`object`). The
   * core read none of it and writes nothing over it until FS_CHANGED says it is
   * empty. */
  refused_files: RefusedFile[];
  /** Why the server does not answer the world's reads now: failing since the last answered, or given up. Absent while it answers. The network and the GPU are the runtime's to say. */
  stall: Message | null;
  /** How much of what the renderer makes before its first frame is made, 0 to 1 (pxl-render gpu/pipeline.rs `PipelineCache::share`): the pipelines made of those asked for, in whatever order they finish, 0 before any is asked; 1 once it draws, 0 again while a lost device's next renderer is made. The loading bar is the UI's to draw of it. */
  renderer_progress: f64;
  /** How much of the view's ground shows, 0 to 1: of the tiles asked for, those with an answer or drawn coarser. 0 where none is asked yet, 1 once the area is shown; it goes back as more is asked for. */
  area_progress: f64;
}

export function read_snapshot(b: BranchCursor): Snapshot {
  return {
    pivot: read_pivot_state(b.at(0).as_branch()),
    location: read_location_state(b.at(1).as_branch()),
    companion: read_companion_state(b.at(2).as_branch()),
    draft: b.at(3).get_opt((c) => c.get_str()),
    orient: read_orientation_state(b.at(4).as_branch()),
    is_follow_mode: b.at(5).get_bool(),
    camera_busy: b.at(6).get_bool(),
    zooming: b.at(7).get_bool(),
    over_marker: b.at(8).get_bool(),
    settings: Array.from(b.at(9).get_u32s()),
    division: list(b.at_branch(10), read_division_part),
    character_surfaces: Array.from(b.at(11).get_u32s()),
    refused_files: list(b.at_branch(12), read_refused_file),
    stall: b.at(13).get_opt((c) => MEMBER(c.get_number(), Message)),
    renderer_progress: FINITE(b.at(14).get_number()),
    area_progress: FINITE(b.at(15).get_number()),
  };
}

export interface PivotState {
  status: GroundStatus;
  is_off_center: boolean;
  /** The pivot in degrees: what the search measures distance from. */
  lon_lat: LonLat | null;
  /** The pivot is past the service area: the source serves no shard over it (pxl-core sys/io/source.rs). */
  outside: boolean;
}

function read_pivot_state(b: BranchCursor): PivotState {
  return {
    status: MEMBER(b.at(0).get_number(), GroundStatus),
    is_off_center: b.at(1).get_bool(),
    lon_lat: b.at(2).get_opt((c) => read_lon_lat(c.as_branch())),
    outside: b.at(3).get_bool(),
  };
}

export interface LonLat {
  lon: f64;
  lat: f64;
}

function read_lon_lat(b: BranchCursor): LonLat {
  return {
    lon: FINITE(b.at(0).get_number()),
    lat: FINITE(b.at(1).get_number()),
  };
}

export interface LocationState {
  status: SensorStatus;
  has_location: boolean;
  /** Whether a watch runs, so that asking for the location again would start nothing. */
  watching: boolean;
  /** Present exactly when status is Error: the failure the core chose. */
  err: Message | null;
}

function read_location_state(b: BranchCursor): LocationState {
  return {
    status: MEMBER(b.at(0).get_number(), SensorStatus),
    has_location: b.at(1).get_bool(),
    watching: b.at(2).get_bool(),
    err: b.at(3).get_opt((c) => MEMBER(c.get_number(), Message)),
  };
}

export interface CompanionState {
  person_ready: boolean;
  pet_ready: boolean;
}

function read_companion_state(b: BranchCursor): CompanionState {
  return { person_ready: b.at(0).get_bool(), pet_ready: b.at(1).get_bool() };
}

export interface OrientationState {
  status: SensorStatus;
}

function read_orientation_state(b: BranchCursor): OrientationState {
  return { status: MEMBER(b.at(0).get_number(), SensorStatus) };
}

/** One division of the name, in its own name. */
export interface DivisionPart {
  /** Overture's subtype, broadest first (spec/spec/division.json). */
  subtype: u8;
  name: string;
  /** The view is in far enough for its name to show: as far as Overture's own tiles
   * show it from (pxl-spec bake/tile/division.rs `Division::shown_from`). */
  shown: boolean;
  /** Held by an area the point is in; one without is the nearest found by its point. */
  area: boolean;
}

function read_division_part(b: BranchCursor): DivisionPart {
  return {
    subtype: INT_U8(b.at(0).get_number()),
    name: b.at(1).get_str(),
    shown: b.at(2).get_bool(),
    area: b.at(3).get_bool(),
  };
}

export interface RefusedFile {
  /** Its path, as the core opened it (file.json `path`). */
  path: Path;
}

function read_refused_file(b: BranchCursor): RefusedFile {
  return { path: read_path(b.at_branch(0)) };
}

/** Command.SENSOR_START's packet. */
export interface SensorStart {
  sensor: Sensor;
}

/** Command.SENSOR_STOP's packet. */
export interface SensorStop {
  sensor: Sensor;
}

/** Command.UI_SNAPSHOT's packet. */
export interface UiSnapshot {
  at: u32;
  len: u32;
}

/** Command.UI_VIEW's packet. */
export interface UiView {
  azimuth: f64;
  /** The meters a logical pixel spans at the pivot, at the camera's distance; 0 where the camera stands there. The scale bar is the UI's to make of it; which division names show, the core's (`DivisionPart.shown`). */
  m_per_px: f64;
}

/** Each import's packet, read from its branch, by its number. */
export const read = {
  [Command.SENSOR_START]: (b: BranchCursor): SensorStart => ({
    sensor: MEMBER(b.at(0).get_number(), Sensor),
  }),
  [Command.SENSOR_STOP]: (b: BranchCursor): SensorStop => ({
    sensor: MEMBER(b.at(0).get_number(), Sensor),
  }),
  [Command.UI_SNAPSHOT]: (b: BranchCursor): UiSnapshot => ({
    at: INT_U32(b.at(0).get_number()),
    len: INT_U32(b.at(1).get_number()),
  }),
  [Command.UI_VIEW]: (b: BranchCursor): UiView => ({
    azimuth: FINITE(b.at(0).get_number()),
    m_per_px: FINITE(b.at(1).get_number()),
  }),
};

/** The ui protocol's exports as the UI calls them over `link`: a send returns
 * nothing, and a request a Promise of its reply's value, or of the memory its reply
 * names, which rejects with the errno it was refused with. `fits` says whether a
 * send's packet fits a frame. */
export function ui_exports(link: Link) {
  return {
    /** A GPS fix; a field is absent where the browser gave none. */
    sensor_pos: (
      lon: f64,
      lat: f64,
      alt: f64 | null,
      accuracy: f64 | null,
      course: f64 | null,
    ) => link.send(Command.SENSOR_POS, lon, lat, alt, accuracy, course),
    /** An orientation event; a field is absent where the browser gave none. */
    sensor_orient: (
      webkit_compass_heading: f64 | null,
      alpha: f64 | null,
      beta: f64 | null,
      gamma: f64 | null,
    ) =>
      link.send(
        Command.SENSOR_ORIENT,
        webkit_compass_heading,
        alpha,
        beta,
        gamma,
      ),
    /** A watch's failure: refused from the start (PermissionDenied), cut off, or one it rides
     * out; the compass's, where the platform says so (SENSOR_PLATFORM's orient_silence_is_loss). */
    sensor_err: (
      sensor: Sensor,
      code: GeolocationErrorCode | null,
      watching: boolean,
    ) => link.send(Command.SENSOR_ERR, sensor, code, watching),
    /** What the tile source declares of itself, once the client has it: the tile root's
     * `source`, the published one out of the bucket or the dev tile server's, read by the
     * client (ts/client/src/service/sys/fs/source.ts) and handed over as numbers and names.
     * Until it comes nothing is observed: the floor is held at the zoom it names, and no
     * tile is asked for that no shard it serves is over or under (pxl-core sys/io/source.rs). */
    source_declare: (min_zoom: u32, format: U32Array, shard: string[]) =>
      link
        .request(Command.SOURCE_DECLARE, min_zoom, format, shard)
        .then((r) => r.val),
    /** A permission as the UI's prompt or the browser's change reported it. */
    sensor_permission: (sensor: Sensor, answer: PermissionAnswer) =>
      link.send(Command.SENSOR_PERMISSION, sensor, answer),
    /** What the place's sensors are: whether it has a position and a heading, whether the
     * heading asks a permission, and how it says it is lost. Sent once the core is attached. */
    sensor_platform: (
      geolocation: boolean,
      orient: boolean,
      orient_permission: boolean,
      orient_silence_is_loss: boolean,
    ) =>
      link.send(
        Command.SENSOR_PLATFORM,
        geolocation,
        orient,
        orient_permission,
        orient_silence_is_loss,
      ),
    /** Whether this is a development build, which offers the settings schema.json marks `dev`.
     * Sent once the core is attached. */
    setting_dev: (dev: boolean) => link.send(Command.SETTING_DEV, dev),
    /** A pointer event on the world's canvas, in CSS pixels from the top left of its layout box
     * (transforms aside), as the world surface's size is (GPU_SURFACE_RESIZE). */
    input_pointer: (
      phase: PointerPhase,
      pointer_type: PointerType,
      id: i32,
      button: i32,
      x: f64,
      y: f64,
    ) =>
      link.send(Command.INPUT_POINTER, phase, pointer_type, id, button, x, y),
    /** Whether the UI is taking typed text. A pointer pressed while this holds, a device's touch too (pxl-core door/touch.rs), is spent on ending the typing: its lift ends the gesture with no tap, and its drag still moves the world. */
    input_typing: (typing: boolean) => link.send(Command.INPUT_TYPING, typing),
    /** The tool and what it takes, the UI's to choose and keep: the core is told, and answers EINVAL for an item the catalog does not give out. It picks again under a hovering pointer. */
    edit_tool: (tool: Tool, val: u32, scale_steps: i32) =>
      link
        .request(Command.EDIT_TOOL, tool, val, scale_steps)
        .then((r) => r.val),
    /** The draft's text (Snapshot.draft), which the marker is drawn with and committed with. Text past MARKER_TEXT_CHARS (spec/spec/asset/marker.json) is refused whole, never cut: EMSGSIZE, which the UI shows as what does not fit. */
    edit_marker_text: (text: string) =>
      link.request(Command.EDIT_MARKER_TEXT, text).then((r) => r.val),
    edit_marker_cancel: () => link.send(Command.EDIT_MARKER_CANCEL),
    /** The marker's draft committed with its text, where its place is still free or still a marker, and closed. */
    edit_marker_commit: () => link.send(Command.EDIT_MARKER_COMMIT),
    input_wheel: (delta_y: f64) => link.send(Command.INPUT_WHEEL, delta_y),
    /** A pinch the platform recognized, as a trackpad's is: the factor the fingers' spread changed by since the last. Over one, they spread, and the camera comes nearer by that factor, as under two touches. */
    input_pinch: (scale: f64) => link.send(Command.INPUT_PINCH, scale),
    /** The screen direction the held movement keys point, sent when it changes: each axis -1, 0 or 1, x to the right and y down. */
    input_move: (x: i32, y: i32) => link.send(Command.INPUT_MOVE, x, y),
    /** What a surface the UI opened draws, until its GPU_SURFACE_CLOSE. The world, with the
     * sensors watched while it shows; or the companion's character of a group alone, as the picker shows it, a second one
     * on the same surface replacing the first. EBADF where the surface is not open, EBUSY where it already draws the
     * other content, EINVAL where the character's fields are not all given for a character or any is given for the
     * world. */
    scene_draw: (
      surface: u32,
      content: SceneContent,
      group: u32 | null,
      bg: u32 | null,
    ) => link.send(Command.SCENE_DRAW, surface, content, group, bg),
    /** A setting set from the sheet or the character picker; refused until the core has read the
     * kept settings, which would be read over it. */
    setting_set: (key: u32, val: u32) =>
      link.send(Command.SETTING_SET, key, val),
    /** A line the player said, shown over the character. */
    scene_chat: (text: string) => link.send(Command.SCENE_CHAT, text),
    /** The tracking button's press, as the button showed it. */
    camera_tracking: (tracking: Tracking) =>
      link.send(Command.CAMERA_TRACKING, tracking),
    /** The compass was pressed: north comes up. */
    camera_north_up: () => link.send(Command.CAMERA_NORTH_UP),
    /** The search panel's location card's press: the location asked for. The compass is
     * asked for where it is used, entering follow mode (TRACKING), and never alone: nothing
     * reads it without the location. */
    sensor_locate: () => link.send(Command.SENSOR_LOCATE),
    /** A search result: the pivot moved there, and the view asked with it at once, as the
     * controls ask it where the camera stands; a view field absent keeps what the view has. */
    camera_goto: (
      lon: f64,
      lat: f64,
      dist: f64 | null,
      azimuth: f64 | null,
      polar: f64 | null,
    ) => link.send(Command.CAMERA_GOTO, lon, lat, dist, azimuth, polar),
    /** A tracked pose: where a tracker has the camera, and its turn as a unit quaternion
     * (x, y, z, w) from the camera's axes to the tracker's, in the world's units. The
     * first of a run is taken to be where the camera already is, so nothing moves as
     * it begins; the camera follows the tracker from there until POSE_END. Nothing in the
     * client sends it: it is the way in for an immersive mode's head pose to drive the
     * camera, from the device app. */
    input_xr_pose: (
      x: f64,
      y: f64,
      z: f64,
      qx: f64,
      qy: f64,
      qz: f64,
      qw: f64,
    ) => link.send(Command.INPUT_XR_POSE, x, y, z, qx, qy, qz, qw),
    /** The tracker let the camera go: back to the controller it had. */
    input_xr_pose_end: () => link.send(Command.INPUT_XR_POSE_END),
    /** Starts a fresh recording window. */
    perf_start: () => link.send(Command.PERF_START),
    /** Freezes the window, so what is exported holds still. */
    perf_stop: () => link.send(Command.PERF_STOP),
    /** The profiler's statistics and what it holds now: the reply names STAT's words then NOW's (perf.json), the statistics
     * over the last `window_ms`, the whole recording where it is absent. A
     * number past the door is finite, so no window is infinite. */
    perf_stats: (window_ms: f64 | null) =>
      link.request(Command.PERF_STATS, window_ms).then((r) => r.named),
    /** One ring, nothing reduced: the reply names the records of the ring `ring` (its index in perf.json's RING_NAMES), the frames and the tasks oldest first, the worst frames longest first. */
    perf_rings: (ring: u32) =>
      link.request(Command.PERF_RINGS, ring).then((r) => r.named),
    /** The string table of the tasks' failure sites: the reply names its bytes, into which a TASK's `at_file` is an offset (perf.json). */
    perf_strtab: () => link.request(Command.PERF_STRTAB).then((r) => r.named),
    fits: {
      sensor_pos: (
        lon: f64,
        lat: f64,
        alt: f64 | null,
        accuracy: f64 | null,
        course: f64 | null,
      ) => link.fits(Command.SENSOR_POS, lon, lat, alt, accuracy, course),
      sensor_orient: (
        webkit_compass_heading: f64 | null,
        alpha: f64 | null,
        beta: f64 | null,
        gamma: f64 | null,
      ) =>
        link.fits(
          Command.SENSOR_ORIENT,
          webkit_compass_heading,
          alpha,
          beta,
          gamma,
        ),
      sensor_err: (
        sensor: Sensor,
        code: GeolocationErrorCode | null,
        watching: boolean,
      ) => link.fits(Command.SENSOR_ERR, sensor, code, watching),
      sensor_permission: (sensor: Sensor, answer: PermissionAnswer) =>
        link.fits(Command.SENSOR_PERMISSION, sensor, answer),
      sensor_platform: (
        geolocation: boolean,
        orient: boolean,
        orient_permission: boolean,
        orient_silence_is_loss: boolean,
      ) =>
        link.fits(
          Command.SENSOR_PLATFORM,
          geolocation,
          orient,
          orient_permission,
          orient_silence_is_loss,
        ),
      setting_dev: (dev: boolean) => link.fits(Command.SETTING_DEV, dev),
      input_pointer: (
        phase: PointerPhase,
        pointer_type: PointerType,
        id: i32,
        button: i32,
        x: f64,
        y: f64,
      ) =>
        link.fits(Command.INPUT_POINTER, phase, pointer_type, id, button, x, y),
      input_typing: (typing: boolean) =>
        link.fits(Command.INPUT_TYPING, typing),
      edit_marker_cancel: () => link.fits(Command.EDIT_MARKER_CANCEL),
      edit_marker_commit: () => link.fits(Command.EDIT_MARKER_COMMIT),
      input_wheel: (delta_y: f64) => link.fits(Command.INPUT_WHEEL, delta_y),
      input_pinch: (scale: f64) => link.fits(Command.INPUT_PINCH, scale),
      input_move: (x: i32, y: i32) => link.fits(Command.INPUT_MOVE, x, y),
      scene_draw: (
        surface: u32,
        content: SceneContent,
        group: u32 | null,
        bg: u32 | null,
      ) => link.fits(Command.SCENE_DRAW, surface, content, group, bg),
      setting_set: (key: u32, val: u32) =>
        link.fits(Command.SETTING_SET, key, val),
      scene_chat: (text: string) => link.fits(Command.SCENE_CHAT, text),
      camera_tracking: (tracking: Tracking) =>
        link.fits(Command.CAMERA_TRACKING, tracking),
      camera_north_up: () => link.fits(Command.CAMERA_NORTH_UP),
      sensor_locate: () => link.fits(Command.SENSOR_LOCATE),
      camera_goto: (
        lon: f64,
        lat: f64,
        dist: f64 | null,
        azimuth: f64 | null,
        polar: f64 | null,
      ) => link.fits(Command.CAMERA_GOTO, lon, lat, dist, azimuth, polar),
      input_xr_pose: (
        x: f64,
        y: f64,
        z: f64,
        qx: f64,
        qy: f64,
        qz: f64,
        qw: f64,
      ) => link.fits(Command.INPUT_XR_POSE, x, y, z, qx, qy, qz, qw),
      input_xr_pose_end: () => link.fits(Command.INPUT_XR_POSE_END),
      perf_start: () => link.fits(Command.PERF_START),
      perf_stop: () => link.fits(Command.PERF_STOP),
    },
  };
}

/** The ui protocol's exports, as `ui_exports` binds them. */
export type UiExports = ReturnType<typeof ui_exports>;

/** The majors the runtime routes the protocol's imports by. */
export const MAJORS: readonly u32[] = [Major.SENSOR, Major.UI];

/** What the program does with each command the core posts on the ui protocol: a
 * call's member answers its value, and throws kit's error to refuse it. */
export interface UiImports {
  [Command.SENSOR_START](e: SensorStart): void;
  [Command.SENSOR_STOP](e: SensorStop): void;
  /** The UI's snapshot, when it changed: a Snapshot packet far past a frame's room, so a
   * block the core holds, valid until the next snapshot. */
  [Command.UI_SNAPSHOT](e: UiSnapshot, named: U8Array): void;
  /** The view's azimuth and scale, when either moved: a frame of its own, so a rotation or a
   * zoom, which moves them every frame, leaves the snapshot as it was. */
  [Command.UI_VIEW](e: UiView): void;
}

/** `h` as the protocol the runtime routes the ui protocol's majors to: each
 * command read and handed to its member, whose refusal is answered with its
 * errno; ENOSYS for one the protocol does not carry. */
export function ui_imports(h: UiImports): Protocol {
  const on: OnImport = (_tag, cmd, f, _pkt, named) => {
    try {
      switch (cmd) {
        case Command.SENSOR_START:
          return h[Command.SENSOR_START](read[Command.SENSOR_START](need(f)));
        case Command.SENSOR_STOP:
          return h[Command.SENSOR_STOP](read[Command.SENSOR_STOP](need(f)));
        case Command.UI_SNAPSHOT:
          return h[Command.UI_SNAPSHOT](
            read[Command.UI_SNAPSHOT](need(f)),
            need(named),
          );
        case Command.UI_VIEW:
          return h[Command.UI_VIEW](read[Command.UI_VIEW](need(f)));
        default:
          return Errno.ENOSYS;
      }
    } catch (err) {
      return refusal_of(err);
    }
  };
  return { majors: MAJORS, on };
}

/** A packet the command carries, which a frame without one breaks. */
function need<T>(f: T | null): T {
  if (f === null) throw new MarshalError(Errno.EBADMSG);
  return f;
}
