// @pxl/client/abi/cmd.spec.ts
//
// GENERATED from spec/client/kernel/domain/control/cmd.json

/**
 * The kernel's commands, by number (@xpute/kit/abi/cmd.ts: `major << 8 | minor`),
 * and the packet each carries — an XTP branch whose children are its fields, in
 * order. A command with no fields carries no packet. The kernel is the process; the
 * JavaScript around it is three things to it, its host, which holds the turns, its
 * drivers, which answer its devices, and its shell, the UI. A command's major is the module it is for, and
 * the sections say which of them it is between.
 *
 * The same number is a command on the submission ring and an event on the
 * completion ring (its `RES` clear). The direction is the ring: `pkt` is
 * what the shell submits, `event` what the kernel posts. Each is written and
 * read by what this file generates, on both sides, so no field is found by
 * its position anywhere else.
 *
 * A field is a number of a width, a `bool`, a `str`, a member of an enum
 * below or of one another spec declares (`extern`), or a record below. An
 * `optional` one may be absent, its kind kept; a `list` is a branch of them,
 * or a packed array where the item is a number. A record crosses as a branch
 * of its own fields, and is how a block past a slot is laid out. An enum's
 * `unknown` is the member a number it does not give out reads as; an enum
 * without one refuses the packet. The generator refuses a number given twice.
 */

import type {
  BranchCursor,
  NodeCursor,
  PacketRoom,
  PacketWriter,
} from "@xpute/kit/wire/xtp/mod.ts";
import type { U32Array } from "@xpute/kit/abi/array.ts";
import type { f64, i32, u32, u8 } from "@xpute/kit/abi/word.ts";
import { SequenceType } from "@xpute/kit/wire/xtp/mod.ts";
import * as Major from "./cmd/major.spec.ts";
import * as SysMinor from "./cmd/sys_minor.spec.ts";
import * as InputMinor from "./cmd/input_minor.spec.ts";
import * as SensorMinor from "./cmd/sensor_minor.spec.ts";
import * as FsMinor from "./cmd/fs_minor.spec.ts";
import * as GpuMinor from "./cmd/gpu_minor.spec.ts";
import * as SurfaceMinor from "./cmd/surface_minor.spec.ts";
import * as SettingMinor from "./cmd/setting_minor.spec.ts";
import * as SceneMinor from "./cmd/scene_minor.spec.ts";
import * as CameraMinor from "./cmd/camera_minor.spec.ts";
import * as PerfMinor from "./cmd/perf_minor.spec.ts";
import * as EditMinor from "./cmd/edit_minor.spec.ts";
import * as ShellMinor from "./cmd/shell_minor.spec.ts";
import * as PointerPhase from "./cmd/pointer_phase.spec.ts";
import * as PointerType from "./cmd/pointer_type.spec.ts";
import * as Tool from "./cmd/tool.spec.ts";
import * as Tracking from "./cmd/tracking.spec.ts";
import * as GeolocationErrorCode from "./cmd/geolocation_error_code.spec.ts";
import * as Sensor from "./cmd/sensor.spec.ts";
import * as PermissionAnswer from "./cmd/permission_answer.spec.ts";
import * as GroundStatus from "./cmd/ground_status.spec.ts";
import * as SensorStatus from "./cmd/sensor_status.spec.ts";
import * as Message from "./cmd/message.spec.ts";
import * as Command from "./cmd/command.spec.ts";

/** One of Major's, as its enum was a type. */
type Major = (typeof Major)[keyof typeof Major];
export { Major };

/** One of SysMinor's, as its enum was a type. */
type SysMinor = (typeof SysMinor)[keyof typeof SysMinor];
export { SysMinor };

/** One of InputMinor's, as its enum was a type. */
type InputMinor = (typeof InputMinor)[keyof typeof InputMinor];
export { InputMinor };

/** One of SensorMinor's, as its enum was a type. */
type SensorMinor = (typeof SensorMinor)[keyof typeof SensorMinor];
export { SensorMinor };

/** One of FsMinor's, as its enum was a type. */
type FsMinor = (typeof FsMinor)[keyof typeof FsMinor];
export { FsMinor };

/** One of GpuMinor's, as its enum was a type. */
type GpuMinor = (typeof GpuMinor)[keyof typeof GpuMinor];
export { GpuMinor };

/** One of SurfaceMinor's, as its enum was a type. */
type SurfaceMinor = (typeof SurfaceMinor)[keyof typeof SurfaceMinor];
export { SurfaceMinor };

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

/** One of ShellMinor's, as its enum was a type. */
type ShellMinor = (typeof ShellMinor)[keyof typeof ShellMinor];
export { ShellMinor };

/** One of PointerPhase's, as its enum was a type. */
type PointerPhase = (typeof PointerPhase)[keyof typeof PointerPhase];
export { PointerPhase };

/** One of PointerType's, as its enum was a type. */
type PointerType = (typeof PointerType)[keyof typeof PointerType];
export { PointerType };

/** One of Tool's, as its enum was a type. */
type Tool = (typeof Tool)[keyof typeof Tool];
export { Tool };

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

/** What the page's UI shows of the kernel (shell.rs): written after a turn that changed it, and replaced whole. */
export interface Snapshot {
  pivot: PivotState;
  location: LocationState;
  companion: CompanionState;
  /** A marker's draft, its text so far, absent where none is open: opened by the marker tool's tap, with the marker's own text where one stands there, empty where it is new, and closed by EDIT_MARKER_COMMIT or EDIT_MARKER_CANCEL. */
  draft: string | null;
  orient: OrientationState;
  /** The view's azimuth. */
  azimuth: f64;
  is_follow_mode: boolean;
  camera_busy: boolean;
  /** A zoom is in hand: two fingers are down, or a wheel's zoom is still arriving. The scale is shown while it is. */
  zooming: boolean;
  /** The pointer is over a scene marker a tap would act on with the tool held: a place to go to, or a pin to erase or draft. */
  over_marker: boolean;
  /** Every setting's value by its SettingKey, as SETTING_SET carries one; none until the kernel
   * has read its storage file, a few turns from boot. */
  settings: u32[];
  /** The meters a logical pixel spans at the pivot, at the camera's distance; 0 where the camera stands there. The scale bar is the shell's to make of it; which division names show, the kernel's (`DivisionPart.shown`). */
  m_per_px: f64;
  /** The division the pivot is in and every one above it, broad to narrow; empty
   * where the pivot is in none or the tile it is read out of has not arrived. */
  division: DivisionPart[];
  /** The surfaces a character preview is drawn on: its model and skin have arrived. The shell shows a case loading until its surface is here. */
  previews: u32[];
  /** The files the kernel refused, not of their format, not decoding, or too large for any
   * heap: a storage file (file.json's `storage`) or a placed objects' file (`object`). The
   * kernel read none of it and writes nothing over it until FS_TRUNCATED says it is
   * empty. */
  refused_files: RefusedFile[];
  /** Why the server does not answer the world's reads now: failing since the last answered, or given up. Absent while it answers. The network and the GPU are the host's to say. */
  stall: Message | null;
  /** How much of what the renderer makes before its first frame is made, 0 to 1 (pxl-render gpu/pipeline.rs `Pipelines::share`): its shader modules, by their bytes, the first half, its pipelines made the second; 1 once it draws, 0 again while a lost device's next renderer is made. The loading bar is the shell's to draw of it. */
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
    azimuth: b.at(5).get<f64>(),
    is_follow_mode: b.at(6).get<boolean>(),
    camera_busy: b.at(7).get<boolean>(),
    zooming: b.at(8).get<boolean>(),
    over_marker: b.at(9).get<boolean>(),
    settings: Array.from(b.at(10).get<U32Array>()),
    m_per_px: b.at(11).get<f64>(),
    division: list(b.at_branch(12), read_division_part),
    previews: Array.from(b.at(13).get<U32Array>()),
    refused_files: list(b.at_branch(14), read_refused_file),
    stall: b.at(15).get<Message | null>(),
    renderer_progress: b.at(16).get<f64>(),
    area_progress: b.at(17).get<f64>(),
  };
}

export interface PivotState {
  status: GroundStatus;
  is_off_center: boolean;
  /** The pivot in degrees: what the search measures distance from. */
  lon_lat: LonLat | null;
  /** The pivot is past the service area: the source serves no shard over it (pxl-kernel sys/io/source.rs). */
  outside: boolean;
}

export function read_pivot_state(b: BranchCursor): PivotState {
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

export function read_lon_lat(b: BranchCursor): LonLat {
  return { lon: b.at(0).get<f64>(), lat: b.at(1).get<f64>() };
}

export interface LocationState {
  status: SensorStatus;
  has_location: boolean;
  /** Present exactly when status is Error: the failure the kernel chose. */
  err: Message | null;
}

export function read_location_state(b: BranchCursor): LocationState {
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

export function read_companion_state(b: BranchCursor): CompanionState {
  return {
    person_ready: b.at(0).get<boolean>(),
    pet_ready: b.at(1).get<boolean>(),
  };
}

export interface OrientationState {
  status: SensorStatus;
  /** Present exactly when status is Error: the failure the kernel chose. */
  err: Message | null;
}

export function read_orientation_state(b: BranchCursor): OrientationState {
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

export function read_division_part(b: BranchCursor): DivisionPart {
  return {
    subtype: b.at(0).get<u8>(),
    name: b.at(1).get<string>(),
    shown: b.at(2).get<boolean>(),
    area: b.at(3).get<boolean>(),
  };
}

/** A file the kernel refused, as it opened it. */
export interface RefusedFile {
  /** file.json's `root`. */
  root: u32;
  /** Its path below the root, as the kernel opened it. */
  path: string;
}

export function read_refused_file(b: BranchCursor): RefusedFile {
  return { root: b.at(0).get<u32>(), path: b.at(1).get<string>() };
}

/** Command.SENSOR_START's event. */
export interface SensorStartEvent {
  sensor: Sensor;
}

export function read_sensor_start(b: BranchCursor): SensorStartEvent {
  return { sensor: b.at(0).get<Sensor>() };
}

/** Command.SENSOR_STOP's event. */
export interface SensorStopEvent {
  sensor: Sensor;
}

export function read_sensor_stop(b: BranchCursor): SensorStopEvent {
  return { sensor: b.at(0).get<Sensor>() };
}

/** Command.SENSOR_POS's packet, written in place into `to`. */
export function encode_sensor_pos(
  to: PacketRoom,
  lon: f64,
  lat: f64,
  alt: f64 | null,
  accuracy: f64 | null,
  course: f64 | null,
): PacketWriter {
  return to.writer(5).f64(lon).f64(lat).f64(alt).f64(accuracy).f64(course);
}

/** Command.SENSOR_ORIENT's packet, written in place into `to`. */
export function encode_sensor_orient(
  to: PacketRoom,
  webkit_compass_heading: f64 | null,
  alpha: f64 | null,
  beta: f64 | null,
  gamma: f64 | null,
): PacketWriter {
  return to
    .writer(4)
    .f64(webkit_compass_heading)
    .f64(alpha)
    .f64(beta)
    .f64(gamma);
}

/** Command.SENSOR_ERR's packet, written in place into `to`.
 *
 * @param code Geolocation's; orientation gives none today. */
export function encode_sensor_err(
  to: PacketRoom,
  sensor: Sensor,
  code: GeolocationErrorCode | null,
): PacketWriter {
  return to.writer(2).u32(sensor).u32(code);
}

/** Command.SYS_BOOT's packet, written in place into `to`.
 *
 * @param development A development build, which offers the settings schema.json marks `dev`.
 * @param orient_permission iOS 13+'s gate: requestPermission exists and must run inside a gesture.
 * @param orient_silence_is_loss A browser's compass talks while the device is still and cannot say it stopped, so silence is the only sign of loss; a device's speaks only when the heading turns and reports a failure itself (SENSOR_ERR). */
export function encode_sys_boot(
  to: PacketRoom,
  development: boolean,
  geolocation: boolean,
  orient: boolean,
  orient_permission: boolean,
  orient_silence_is_loss: boolean,
): PacketWriter {
  return to
    .writer(5)
    .bool(development)
    .bool(geolocation)
    .bool(orient)
    .bool(orient_permission)
    .bool(orient_silence_is_loss);
}

/** Command.SURFACE_FRAME's packet, written in place into `to`.
 *
 * @param refresh_ms The display's refresh, as the host measures it from the animation frames it is handed, and the interval it paces turns at, a whole number of refreshes (the pacing the client boots it with, @xpute/host sched/frame_pacing.ts `FrameGrant`): the frame's budget.
 * @param unix_ms The page's one clock, in Unix milliseconds: the wall time the sky is placed by, and the step the tick takes, its change since the last frame. */
export function encode_surface_frame(
  to: PacketRoom,
  refresh_ms: f64,
  paced_ms: f64,
  interactive: boolean,
  credits: u32,
  unix_ms: f64,
): PacketWriter {
  return to
    .writer(5)
    .f64(refresh_ms)
    .f64(paced_ms)
    .bool(interactive)
    .u32(credits)
    .f64(unix_ms);
}

/** Command.SYS_SLEEP's packet, written in place into `to`. */
export function encode_sys_sleep(to: PacketRoom, awake: boolean): PacketWriter {
  return to.writer(1).bool(awake);
}

/** Command.FS_MOUNT's packet, written in place into `to`.
 *
 * @param top_zoom The coarsest zoom the source supplies whole.
 * @param format Each kind's format, one a kind, indexed by its bake's number (pxl-spec bake/publication.spec.rs).
 * @param shard The shards it serves, by name. Their order means nothing: no two at a zoom are over one node, and the finer wins. */
export function encode_fs_mount(
  to: PacketRoom,
  top_zoom: u32,
  format: u32[],
  shard: string[],
): PacketWriter {
  return to
    .writer(3)
    .u32(top_zoom)
    .array(SequenceType.U32_ARRAY, Uint32Array.from(format))
    .strs(shard);
}

/** Command.FS_TRUNCATED's packet, written in place into `to`.
 *
 * @param root file.json's `root`.
 * @param path Its path below the root, as the kernel opened it. */
export function encode_fs_truncated(
  to: PacketRoom,
  root: u32,
  path: string,
): PacketWriter {
  return to.writer(2).u32(root).str(path);
}

/** Command.SENSOR_PERMISSION's packet, written in place into `to`. */
export function encode_sensor_permission(
  to: PacketRoom,
  sensor: Sensor,
  answer: PermissionAnswer,
): PacketWriter {
  return to.writer(2).u32(sensor).u32(answer);
}

/** Command.INPUT_POINTER's packet, written in place into `to`. */
export function encode_input_pointer(
  to: PacketRoom,
  phase: PointerPhase,
  pointer_type: PointerType,
  id: i32,
  button: i32,
  x: f64,
  y: f64,
): PacketWriter {
  return to
    .writer(6)
    .u32(phase)
    .u32(pointer_type)
    .i32(id)
    .i32(button)
    .f64(x)
    .f64(y);
}

/** Command.EDIT_TOOL's packet, written in place into `to`.
 *
 * @param val The item to place, its id (id.json); read only for that tool.
 * @param scale_steps A cube's scale, as a decor's `scale_steps`: 0 a unit, each step down its half, to 1 - OBJECT_SCALES (pxl-spec bake/object.spec.rs). Read only for an item, and only a cube's is chosen. */
export function encode_edit_tool(
  to: PacketRoom,
  tool: Tool,
  val: u32,
  scale_steps: i32,
): PacketWriter {
  return to.writer(3).u32(tool).u32(val).i32(scale_steps);
}

/** Command.EDIT_MARKER_TEXT's packet, written in place into `to`. */
export function encode_edit_marker_text(
  to: PacketRoom,
  text: string,
): PacketWriter {
  return to.writer(1).str(text);
}

/** Command.INPUT_WHEEL's packet, written in place into `to`. */
export function encode_input_wheel(to: PacketRoom, delta_y: f64): PacketWriter {
  return to.writer(1).f64(delta_y);
}

/** Command.INPUT_MOVE's packet, written in place into `to`. */
export function encode_input_move(
  to: PacketRoom,
  x: i32,
  y: i32,
): PacketWriter {
  return to.writer(2).i32(x).i32(y);
}

/** Command.SURFACE_OPEN's packet, written in place into `to`.
 *
 * @param role The renderer's surface_role (render/gpu/api.json). */
export function encode_surface_open(to: PacketRoom, role: u32): PacketWriter {
  return to.writer(1).u32(role);
}

/** Command.SURFACE_RESIZE's packet, written in place into `to`. */
export function encode_surface_resize(
  to: PacketRoom,
  surface: u32,
  width: u32,
  height: u32,
  device_pixel_ratio: f64,
): PacketWriter {
  return to
    .writer(4)
    .u32(surface)
    .u32(width)
    .u32(height)
    .f64(device_pixel_ratio);
}

/** Command.SURFACE_CLOSE's packet, written in place into `to`. */
export function encode_surface_close(
  to: PacketRoom,
  surface: u32,
): PacketWriter {
  return to.writer(1).u32(surface);
}

/** Command.SCENE_PREVIEW_OPEN's packet, written in place into `to`. */
export function encode_scene_preview_open(
  to: PacketRoom,
  surface: u32,
  group: u32,
  number: u8,
): PacketWriter {
  return to.writer(3).u32(surface).u32(group).u8(number);
}

/** Command.SCENE_PREVIEW_CLOSE's packet, written in place into `to`. */
export function encode_scene_preview_close(
  to: PacketRoom,
  surface: u32,
): PacketWriter {
  return to.writer(1).u32(surface);
}

/** Command.SETTING_SET's packet, written in place into `to`.
 *
 * @param value A toggle's 0 or 1, a choice's option by its index, or a number. */
export function encode_setting_set(
  to: PacketRoom,
  key: u32,
  value: u32,
): PacketWriter {
  return to.writer(2).u32(key).u32(value);
}

/** Command.SCENE_CHAT's packet, written in place into `to`. */
export function encode_scene_chat(to: PacketRoom, text: string): PacketWriter {
  return to.writer(1).str(text);
}

/** Command.CAMERA_TRACKING's packet, written in place into `to`. */
export function encode_camera_tracking(
  to: PacketRoom,
  tracking: Tracking,
): PacketWriter {
  return to.writer(1).u32(tracking);
}

/** Command.CAMERA_GOTO's packet, written in place into `to`.
 *
 * @param dist The zoom, the eye's distance from the pivot, clamped to the zoom's range; absent, a view far out comes in.
 * @param azimuth The heading about Z in radians, wrapped.
 * @param polar The angle down from vertical in radians, clamped. */
export function encode_camera_goto(
  to: PacketRoom,
  lon: f64,
  lat: f64,
  dist: f64 | null,
  azimuth: f64 | null,
  polar: f64 | null,
): PacketWriter {
  return to.writer(5).f64(lon).f64(lat).f64(dist).f64(azimuth).f64(polar);
}

/** Command.INPUT_XR_POSE's packet, written in place into `to`. */
export function encode_input_xr_pose(
  to: PacketRoom,
  x: f64,
  y: f64,
  z: f64,
  qx: f64,
  qy: f64,
  qz: f64,
  qw: f64,
): PacketWriter {
  return to.writer(7).f64(x).f64(y).f64(z).f64(qx).f64(qy).f64(qz).f64(qw);
}

/** Command.PERF_STATS's packet, written in place into `to`. */
export function encode_perf_stats(
  to: PacketRoom,
  window_ms: f64 | null,
): PacketWriter {
  return to.writer(1).f64(window_ms);
}

/** Command.SHELL_SNAPSHOT's event. */
export interface ShellSnapshotEvent {
  at: u32;
  len: u32;
}

export function read_shell_snapshot(b: BranchCursor): ShellSnapshotEvent {
  return { at: b.at(0).get<u32>(), len: b.at(1).get<u32>() };
}
