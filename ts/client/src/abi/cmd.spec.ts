// ts/client/src/abi/cmd.spec.ts
//
// GENERATED from spec/client/core/door/ui.json

/**
 * The UI's protocol with the core: its commands, by number (@xpute/kit/abi/cmd.ts:
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

import type { BranchCursor, NodeCursor } from "@xpute/kit/wire/xtp/mod.ts";
import type { U32Array } from "@xpute/kit/abi/array.ts";
import type { f64, i32, u32, u8 } from "@xpute/kit/abi/word.ts";
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
import { MarshalError } from "@xpute/kit/status/error.ts";
import type { Protocol, OnImport } from "@xpute/runtime";

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

/** A record that may be absent. */
function optional<T>(c: NodeCursor, read: (b: BranchCursor) => T): T | null {
  const present = c.opt();
  return present === undefined ? null : read(present.as_branch());
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
  /** A zoom is in hand: two fingers are down, or a wheel's zoom is still arriving. The scale is shown while it is. */
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
    pivot: read_pivot_state(b.at_branch(0)),
    location: read_location_state(b.at_branch(1)),
    companion: read_companion_state(b.at_branch(2)),
    draft: b.at(3).get<string | null>(),
    orient: read_orientation_state(b.at_branch(4)),
    is_follow_mode: b.at(5).get<boolean>(),
    camera_busy: b.at(6).get<boolean>(),
    zooming: b.at(7).get<boolean>(),
    over_marker: b.at(8).get<boolean>(),
    settings: Array.from(b.at(9).get<U32Array>()),
    division: list(b.at_branch(10), read_division_part),
    character_surfaces: Array.from(b.at(11).get<U32Array>()),
    refused_files: list(b.at_branch(12), read_refused_file),
    stall: b.at(13).get<Message | null>(),
    renderer_progress: b.at(14).get<f64>(),
    area_progress: b.at(15).get<f64>(),
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
    status: b.at(0).get<GroundStatus>(),
    is_off_center: b.at(1).get<boolean>(),
    lon_lat: optional(b.at(2), read_lon_lat),
    outside: b.at(3).get<boolean>(),
  };
}

export interface LonLat {
  lon: f64;
  lat: f64;
}

function read_lon_lat(b: BranchCursor): LonLat {
  return { lon: b.at(0).get<f64>(), lat: b.at(1).get<f64>() };
}

export interface LocationState {
  status: SensorStatus;
  has_location: boolean;
  /** Present exactly when status is Error: the failure the core chose. */
  err: Message | null;
}

function read_location_state(b: BranchCursor): LocationState {
  return {
    status: b.at(0).get<SensorStatus>(),
    has_location: b.at(1).get<boolean>(),
    err: b.at(2).get<Message | null>(),
  };
}

export interface CompanionState {
  person_ready: boolean;
  pet_ready: boolean;
}

function read_companion_state(b: BranchCursor): CompanionState {
  return {
    person_ready: b.at(0).get<boolean>(),
    pet_ready: b.at(1).get<boolean>(),
  };
}

export interface OrientationState {
  status: SensorStatus;
  /** Present exactly when status is Error: the failure the core chose. */
  err: Message | null;
}

function read_orientation_state(b: BranchCursor): OrientationState {
  return {
    status: b.at(0).get<SensorStatus>(),
    err: b.at(1).get<Message | null>(),
  };
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
    subtype: b.at(0).get<u8>(),
    name: b.at(1).get<string>(),
    shown: b.at(2).get<boolean>(),
    area: b.at(3).get<boolean>(),
  };
}

export interface RefusedFile {
  /** file.json's `root`. */
  root: u32;
  /** Its path below the root, as the core opened it. */
  path: string;
}

function read_refused_file(b: BranchCursor): RefusedFile {
  return { root: b.at(0).get<u32>(), path: b.at(1).get<string>() };
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

/** Command.TURN_PACE's packet. */
export interface TurnPace {
  /** 0 asks none, and turns go at the display's own rate. */
  interval_ms: f64;
}

/** Each import's packet, read from its branch, by its number. */
export const read = {
  [Command.SENSOR_START]: (b: BranchCursor): SensorStart => ({
    sensor: b.at(0).get<Sensor>(),
  }),
  [Command.SENSOR_STOP]: (b: BranchCursor): SensorStop => ({
    sensor: b.at(0).get<Sensor>(),
  }),
  [Command.UI_SNAPSHOT]: (b: BranchCursor): UiSnapshot => ({
    at: b.at(0).get<u32>(),
    len: b.at(1).get<u32>(),
  }),
  [Command.UI_VIEW]: (b: BranchCursor): UiView => ({
    azimuth: b.at(0).get<f64>(),
    m_per_px: b.at(1).get<f64>(),
  }),
  [Command.TURN_PACE]: (b: BranchCursor): TurnPace => ({
    interval_ms: b.at(0).get<f64>(),
  }),
};

/** What the runtime sends on the ui protocol: each command by its number, and
 * the fields its packet takes in order. It is sent, requested and answered
 * later, or called and answered before it returns. */
export type UiSend = {
  [Command.SENSOR_POS]: [
    lon: f64,
    lat: f64,
    alt: f64 | null,
    accuracy: f64 | null,
    course: f64 | null,
  ];
  [Command.SENSOR_ORIENT]: [
    webkit_compass_heading: f64 | null,
    alpha: f64 | null,
    beta: f64 | null,
    gamma: f64 | null,
  ];
  /** @param code Geolocation's; orientation gives none. */
  [Command.SENSOR_ERR]: [sensor: Sensor, code: GeolocationErrorCode | null];
  /** @param refresh_ms The display's refresh, as the runtime measures it from the animation frames it is handed, and the interval it paces turns at, a whole number of refreshes (the pacing the client boots it with, @xpute/runtime sched/frame_pacing.ts `FrameGrant`): the frame's budget. */
  [Command.TURN_GRANT]: [
    refresh_ms: f64,
    paced_ms: f64,
    interactive: boolean,
    credits: u32,
  ];
  [Command.SENSOR_PERMISSION]: [sensor: Sensor, answer: PermissionAnswer];
  /** @param orient_permission iOS 13+'s gate: requestPermission exists and must run inside a gesture.
   * @param orient_silence_is_loss A browser's compass talks while the device is still and cannot say it stopped, so silence is the only sign of loss; a device's speaks only when the heading turns and reports a failure itself (SENSOR_ERR). */
  [Command.SENSOR_PLATFORM]: [
    geolocation: boolean,
    orient: boolean,
    orient_permission: boolean,
    orient_silence_is_loss: boolean,
  ];
  [Command.SETTING_DEV]: [dev: boolean];
  [Command.INPUT_POINTER]: [
    phase: PointerPhase,
    pointer_type: PointerType,
    id: i32,
    button: i32,
    x: f64,
    y: f64,
  ];
  [Command.EDIT_MARKER_CANCEL]: [];
  [Command.EDIT_MARKER_COMMIT]: [];
  [Command.INPUT_WHEEL]: [delta_y: f64];
  [Command.INPUT_MOVE]: [x: i32, y: i32];
  /** @param bg What the character is drawn over, 0xRRGGBB, sRGB. */
  [Command.SCENE_DRAW]: [
    surface: u32,
    content: SceneContent,
    group: u32 | null,
    bg: u32 | null,
  ];
  /** @param val A toggle's 0 or 1, a choice's option by its index, or a number. */
  [Command.SETTING_SET]: [key: u32, val: u32];
  [Command.SCENE_CHAT]: [text: string];
  [Command.CAMERA_TRACKING]: [tracking: Tracking];
  [Command.CAMERA_NORTH_UP]: [];
  [Command.SENSOR_LOCATE]: [];
  /** @param dist The zoom, the eye's distance from the pivot, clamped to the zoom's range; absent, a view far out comes in.
   * @param azimuth The heading about Z in radians, wrapped.
   * @param polar The angle down from vertical in radians, clamped. */
  [Command.CAMERA_GOTO]: [
    lon: f64,
    lat: f64,
    dist: f64 | null,
    azimuth: f64 | null,
    polar: f64 | null,
  ];
  [Command.INPUT_XR_POSE]: [
    x: f64,
    y: f64,
    z: f64,
    qx: f64,
    qy: f64,
    qz: f64,
    qw: f64,
  ];
  [Command.INPUT_XR_POSE_END]: [];
  [Command.PERF_START]: [];
  [Command.PERF_STOP]: [];
};
export type UiRequest = {
  /** @param val The item to place, its id (id.json); read only for that tool.
   * @param scale_steps A cube's scale, as a decor's `scale_steps`: 0 a unit, each step down its half, to 1 - OBJECT_SCALES (pxl-spec bake/object.spec.rs). Read only for an item, and only a cube's is chosen. */
  [Command.EDIT_TOOL]: [tool: Tool, val: u32, scale_steps: i32];
  [Command.EDIT_MARKER_TEXT]: [text: string];
};
export type UiCall = {
  /** @param min_zoom The coarsest zoom the source supplies whole.
   * @param format Each kind's format, one a kind, indexed by its bake's number (pxl-spec bake/publication.spec.rs).
   * @param shard The shards it serves, by name. Their order means nothing: no two at a zoom are over one node, and the finer wins. */
  [Command.SOURCE_DECLARE]: [min_zoom: u32, format: U32Array, shard: string[]];
  [Command.PERF_STATS]: [window_ms: f64 | null];
  [Command.PERF_RINGS]: [];
};

/** The ui protocol over a link. */
export type UiProtocol = Protocol<UiSend, UiRequest, UiCall>;

/** What the program does with each command the core posts on the ui protocol; a
 * call's answer is its return. */
export interface UiImports {
  [Command.SENSOR_START](e: SensorStart): void;
  [Command.SENSOR_STOP](e: SensorStop): void;
  /** The UI's snapshot, when it changed: a Snapshot packet far past a ring frame, so a
   * block the core holds, valid until the next snapshot. */
  [Command.UI_SNAPSHOT](e: UiSnapshot): void;
  /** The view's azimuth and scale, when either moved: a frame of its own, so a rotation or a
   * zoom, which moves them every frame, leaves the snapshot as it was. */
  [Command.UI_VIEW](e: UiView): void;
  /** The least interval the core asks between its turns: its load's word on the rate, which
   * the next TURN_GRANT's `paced_ms` answers, a whole number of refreshes (the pacing the
   * client boots the runtime with, @xpute/runtime sched/frame_pacing.ts `pace`). */
  [Command.TURN_PACE](e: TurnPace): void;
}

/** `h` as the handler a runtime routes the ui protocol's commands to, each one read and
 * handed to its member; ENOSYS for one the protocol does not carry. */
export function ui_imports(h: UiImports): OnImport {
  return (_tag, cmd, f) => {
    switch (cmd) {
      case Command.SENSOR_START:
        return h[Command.SENSOR_START](read[Command.SENSOR_START](need(f)));
      case Command.SENSOR_STOP:
        return h[Command.SENSOR_STOP](read[Command.SENSOR_STOP](need(f)));
      case Command.UI_SNAPSHOT:
        return h[Command.UI_SNAPSHOT](read[Command.UI_SNAPSHOT](need(f)));
      case Command.UI_VIEW:
        return h[Command.UI_VIEW](read[Command.UI_VIEW](need(f)));
      case Command.TURN_PACE:
        return h[Command.TURN_PACE](read[Command.TURN_PACE](need(f)));
      default:
        return Errno.ENOSYS;
    }
  };
}

/** The core's memory a command on the ui protocol names, where it is and how
 * long; null for none. */
export function ui_mem(cmd: u32, f: BranchCursor | null): [u32, u32] | null {
  switch (cmd) {
    case Command.UI_SNAPSHOT: {
      const e = read[Command.UI_SNAPSHOT](need(f));
      return [e.at, e.len];
    }
    default:
      return null;
  }
}

/** A packet the command carries, which a frame without one breaks. */
function need<T>(f: T | null): T {
  if (f === null) throw new MarshalError(Errno.EBADMSG);
  return f;
}
