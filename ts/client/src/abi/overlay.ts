// @pxl/client/abi/overlay.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import type { U32Array } from "@xpute/kit/abi/array.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import {
  SequenceType,
  type NodeCursor,
  type PacketWriter,
} from "@xpute/kit/wire/xtp/mod.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
  type Handle,
  get_as,
} from "@xpute/link/link.ts";
import * as types from "@xpute/link/abi/types.ts";

/** The interface's handle, named by `pxl:core/overlay` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x7b1393;

/** `show` */
export const SHOW: Method = 0;
/** `view` */
export const VIEW: Method = 1;

export const GroundStatus = {
  Idle: 0,
  Loading: 1,
  Success: 2,
} as const;
export type GroundStatus = (typeof GroundStatus)[keyof typeof GroundStatus];

/** The name of the `GroundStatus` member numbered `n`, or undefined for a number
 * no member has. */
export function ground_status_name(
  n: number,
): keyof typeof GroundStatus | undefined {
  for (const k in GroundStatus)
    if (GroundStatus[k as keyof typeof GroundStatus] === n)
      return k as keyof typeof GroundStatus;
  return undefined;
}

/** The `GroundStatus` member `n` is, or null for a number no member has. */
export function ground_status_of(n: number): GroundStatus | null {
  return ground_status_name(n) === undefined ? null : (n as GroundStatus);
}

export const SensorStatus = {
  Idle: 0,
  Loading: 1,
  Ready: 2,
  Error: 3,
  Unavailable: 4,
} as const;
export type SensorStatus = (typeof SensorStatus)[keyof typeof SensorStatus];

/** The name of the `SensorStatus` member numbered `n`, or undefined for a number
 * no member has. */
export function sensor_status_name(
  n: number,
): keyof typeof SensorStatus | undefined {
  for (const k in SensorStatus)
    if (SensorStatus[k as keyof typeof SensorStatus] === n)
      return k as keyof typeof SensorStatus;
  return undefined;
}

/** The `SensorStatus` member `n` is, or null for a number no member has. */
export function sensor_status_of(n: number): SensorStatus | null {
  return sensor_status_name(n) === undefined ? null : (n as SensorStatus);
}

export const Message = {
  GeolocationUnsupported: 0,
  GeolocationPermissionDenied: 1,
  GeolocationPositionUnavailable: 2,
  GeolocationTimeout: 3,
  GeolocationGenericError: 4,
  /** Reads reach the server and fail; they are tried again. */
  WorldReadFailing: 5,
  /** Reads failed past every retry; the world is not coming. */
  WorldUnavailable: 6,
} as const;
export type Message = (typeof Message)[keyof typeof Message];

/** The name of the `Message` member numbered `n`, or undefined for a number
 * no member has. */
export function message_name(n: number): keyof typeof Message | undefined {
  for (const k in Message)
    if (Message[k as keyof typeof Message] === n)
      return k as keyof typeof Message;
  return undefined;
}

/** The `Message` member `n` is, or null for a number no member has. */
export function message_of(n: number): Message | null {
  return message_name(n) === undefined ? null : (n as Message);
}

export interface LonLat {
  lon: number;
  lat: number;
}

export function write_LonLat(w: PacketWriter, r: LonLat): void {
  w.branch(2, (w) => {
    w.f64(r.lon);
    w.f64(r.lat);
  });
}

export function read_LonLat(c: NodeCursor): LonLat {
  const b = c.as_branch();
  return {
    lon: b.at(0).get_number(),
    lat: b.at(1).get_number(),
  };
}

export interface PivotState {
  status: GroundStatus;
  is_off_center: boolean;
  lon_lat: LonLat | null;
  outside: boolean;
}

export function write_PivotState(w: PacketWriter, r: PivotState): void {
  w.branch(4, (w) => {
    w.u32(r.status);
    w.bool(r.is_off_center);
    if (r.lon_lat === null) w.nil();
    else {
      write_LonLat(w, r.lon_lat);
    }
    w.bool(r.outside);
  });
}

export function read_PivotState(c: NodeCursor): PivotState {
  const b = c.as_branch();
  return {
    status: b.at(0).get_number() as GroundStatus,
    is_off_center: b.at(1).get_bool(),
    lon_lat: b.at(2).get_opt((c) => read_LonLat(c)),
    outside: b.at(3).get_bool(),
  };
}

export interface LocationState {
  status: SensorStatus;
  has_location: boolean;
  watching: boolean;
  err: Message | null;
}

export function write_LocationState(w: PacketWriter, r: LocationState): void {
  w.branch(4, (w) => {
    w.u32(r.status);
    w.bool(r.has_location);
    w.bool(r.watching);
    if (r.err === null) w.nil();
    else {
      w.u32(r.err);
    }
  });
}

export function read_LocationState(c: NodeCursor): LocationState {
  const b = c.as_branch();
  return {
    status: b.at(0).get_number() as SensorStatus,
    has_location: b.at(1).get_bool(),
    watching: b.at(2).get_bool(),
    err: b.at(3).get_opt((c) => c.get_number() as Message),
  };
}

export interface CompanionState {
  person_ready: boolean;
  pet_ready: boolean;
}

export function write_CompanionState(w: PacketWriter, r: CompanionState): void {
  w.branch(2, (w) => {
    w.bool(r.person_ready);
    w.bool(r.pet_ready);
  });
}

export function read_CompanionState(c: NodeCursor): CompanionState {
  const b = c.as_branch();
  return {
    person_ready: b.at(0).get_bool(),
    pet_ready: b.at(1).get_bool(),
  };
}

export interface OrientationState {
  status: SensorStatus;
}

export function write_OrientationState(
  w: PacketWriter,
  r: OrientationState,
): void {
  w.branch(1, (w) => {
    w.u32(r.status);
  });
}

export function read_OrientationState(c: NodeCursor): OrientationState {
  const b = c.as_branch();
  return {
    status: b.at(0).get_number() as SensorStatus,
  };
}

export interface DivisionPart {
  subtype: number;
  name: string;
  shown: boolean;
  area: boolean;
}

export function write_DivisionPart(w: PacketWriter, r: DivisionPart): void {
  w.branch(4, (w) => {
    w.u8(r.subtype);
    w.str(r.name);
    w.bool(r.shown);
    w.bool(r.area);
  });
}

export function read_DivisionPart(c: NodeCursor): DivisionPart {
  const b = c.as_branch();
  return {
    subtype: b.at(0).get_number(),
    name: b.at(1).get_str(),
    shown: b.at(2).get_bool(),
    area: b.at(3).get_bool(),
  };
}

export interface RefusedFile {
  path: types.Token[];
}

export function write_RefusedFile(w: PacketWriter, r: RefusedFile): void {
  w.branch(1, (w) => {
    w.branch(r.path.length, (w) =>
      r.path.forEach((x) => {
        types.write_Token(w, x);
      }),
    );
  });
}

export function read_RefusedFile(c: NodeCursor): RefusedFile {
  const b = c.as_branch();
  return {
    path: b
      .at(0)
      .as_branch()
      .children()
      .map((c) => types.read_Token(c)),
  };
}

export interface Snapshot {
  pivot: PivotState;
  location: LocationState;
  companion: CompanionState;
  draft: string | null;
  orient: OrientationState;
  is_follow_mode: boolean;
  camera_busy: boolean;
  zooming: boolean;
  over_marker: boolean;
  settings: U32Array;
  division: DivisionPart[];
  character_surfaces: U32Array;
  refused_files: RefusedFile[];
  stall: Message | null;
  renderer_progress: number;
  area_progress: number;
}

export function write_Snapshot(w: PacketWriter, r: Snapshot): void {
  w.branch(16, (w) => {
    write_PivotState(w, r.pivot);
    write_LocationState(w, r.location);
    write_CompanionState(w, r.companion);
    if (r.draft === null) w.nil();
    else {
      w.str(r.draft);
    }
    write_OrientationState(w, r.orient);
    w.bool(r.is_follow_mode);
    w.bool(r.camera_busy);
    w.bool(r.zooming);
    w.bool(r.over_marker);
    w.array(SequenceType.U32_ARRAY, r.settings);
    w.branch(r.division.length, (w) =>
      r.division.forEach((x) => {
        write_DivisionPart(w, x);
      }),
    );
    w.array(SequenceType.U32_ARRAY, r.character_surfaces);
    w.branch(r.refused_files.length, (w) =>
      r.refused_files.forEach((x) => {
        write_RefusedFile(w, x);
      }),
    );
    if (r.stall === null) w.nil();
    else {
      w.u32(r.stall);
    }
    w.f64(r.renderer_progress);
    w.f64(r.area_progress);
  });
}

export function read_Snapshot(c: NodeCursor): Snapshot {
  const b = c.as_branch();
  return {
    pivot: read_PivotState(b.at(0)),
    location: read_LocationState(b.at(1)),
    companion: read_CompanionState(b.at(2)),
    draft: b.at(3).get_opt((c) => c.get_str()),
    orient: read_OrientationState(b.at(4)),
    is_follow_mode: b.at(5).get_bool(),
    camera_busy: b.at(6).get_bool(),
    zooming: b.at(7).get_bool(),
    over_marker: b.at(8).get_bool(),
    settings: get_as<U32Array>(b.at(9), SequenceType.U32_ARRAY),
    division: b
      .at(10)
      .as_branch()
      .children()
      .map((c) => read_DivisionPart(c)),
    character_surfaces: get_as<U32Array>(b.at(11), SequenceType.U32_ARRAY),
    refused_files: b
      .at(12)
      .as_branch()
      .children()
      .map((c) => read_RefusedFile(c)),
    stall: b.at(13).get_opt((c) => c.get_number() as Message),
    renderer_progress: b.at(14).get_number(),
    area_progress: b.at(15).get_number(),
  };
}

export interface Overlay {
  show(snapshot: Snapshot): void;
  view(azimuth: number, m_per_px: number): void;
}

/** The object over `imp`. */
function object(imp: Overlay): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case SHOW: {
            if (args === null) return Errno.EBADMSG;
            imp.show(read_Snapshot(args.at(0)));
            return 0;
          }
          case VIEW: {
            if (args === null) return Errno.EBADMSG;
            imp.view(args.at(0).get_number(), args.at(1).get_number());
            return 0;
          }
          default:
            return Errno.ENOSYS;
        }
      } catch (e) {
        return refusal_of(e);
      }
    },
  };
}

/** Serves `imp` under the interface's handle. */
export function mount(l: Pick<Link, "mount_named">, imp: Overlay): void {
  l.mount_named(HANDLE, object(imp));
}
