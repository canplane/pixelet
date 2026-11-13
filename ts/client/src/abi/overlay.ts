// @pxl/client/abi/overlay.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import type { U32Array } from "@xpute/kit/abi/array.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { Branch, type NodeCursor } from "@xpute/kit/wire/xtp/mod.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
  type Sender,
  type Handle,
  uri,
} from "@xpute/link/link.ts";
import * as types from "@xpute/link/abi/types.ts";

/** The interface's handle, named by `pxl:core/overlay` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x7b1393;

/** The interface's qualified name: its package and its name. */
export const NAME = uri("pxl:core", "overlay");

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
  return (Object.keys(GroundStatus) as (keyof typeof GroundStatus)[]).find(
    (k) => GroundStatus[k] === n,
  );
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
  return (Object.keys(SensorStatus) as (keyof typeof SensorStatus)[]).find(
    (k) => SensorStatus[k] === n,
  );
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
  WorldReadFailing: 5,
  WorldUnavailable: 6,
} as const;
export type Message = (typeof Message)[keyof typeof Message];

/** The name of the `Message` member numbered `n`, or undefined for a number
 * no member has. */
export function message_name(n: number): keyof typeof Message | undefined {
  return (Object.keys(Message) as (keyof typeof Message)[]).find(
    (k) => Message[k] === n,
  );
}

/** The `Message` member `n` is, or null for a number no member has. */
export function message_of(n: number): Message | null {
  return message_name(n) === undefined ? null : (n as Message);
}

export interface LonLat {
  lon: number;
  lat: number;
}

export function write_LonLat(r: LonLat): Branch {
  return new Branch([r.lon, r.lat]);
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

export function write_PivotState(r: PivotState): Branch {
  return new Branch([
    r.status,
    r.is_off_center,
    r.lon_lat === null ? null : write_LonLat(r.lon_lat),
    r.outside,
  ]);
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

export function write_LocationState(r: LocationState): Branch {
  return new Branch([r.status, r.has_location, r.watching, r.err]);
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

export function write_CompanionState(r: CompanionState): Branch {
  return new Branch([r.person_ready, r.pet_ready]);
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

export function write_OrientationState(r: OrientationState): Branch {
  return new Branch([r.status]);
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

export function write_DivisionPart(r: DivisionPart): Branch {
  return new Branch([r.subtype, r.name, r.shown, r.area]);
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

export function write_RefusedFile(r: RefusedFile): Branch {
  return new Branch([new Branch(r.path.map((x) => types.write_Token(x)))]);
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

export function write_Snapshot(r: Snapshot): Branch {
  return new Branch([
    write_PivotState(r.pivot),
    write_LocationState(r.location),
    write_CompanionState(r.companion),
    r.draft,
    write_OrientationState(r.orient),
    r.is_follow_mode,
    r.camera_busy,
    r.zooming,
    r.over_marker,
    r.settings,
    new Branch(r.division.map((x) => write_DivisionPart(x))),
    r.character_surfaces,
    new Branch(r.refused_files.map((x) => write_RefusedFile(x))),
    r.stall,
    r.renderer_progress,
    r.area_progress,
  ]);
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
    settings: b.at(9).get() as U32Array,
    division: b
      .at(10)
      .as_branch()
      .children()
      .map((c) => read_DivisionPart(c)),
    character_surfaces: b.at(11).get() as U32Array,
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

export async function show(link: Sender, snapshot: Snapshot): Promise<void> {
  const a = await link.send(HANDLE, SHOW, [write_Snapshot(snapshot)]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function view(
  link: Sender,
  azimuth: number,
  m_per_px: number,
): Promise<void> {
  const a = await link.send(HANDLE, VIEW, [azimuth, m_per_px]);
  if (a.result < 0) throw new XputeError(a.result);
}
