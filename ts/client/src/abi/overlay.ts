// @pxl/client/abi/overlay.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import {
  type GroundStatus,
  type Message,
  type SensorStatus,
  of_GroundStatus,
  of_Message,
  of_SensorStatus,
} from "./overlay.enum.ts";
import { ERRNO_EBADMSG, ERRNO_ENOSYS } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { Lift, type Lower } from "@xpute/link/cabi.ts";
import {
  type Handle,
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/overlay` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x7b1393;

/** `show` */
export const SHOW: Method = 0;
/** `view` */
export const VIEW: Method = 1;

export function load_GroundStatus(r: Lift, at: number): GroundStatus {
  const v = of_GroundStatus(r.case(at, 3));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}

export function load_SensorStatus(r: Lift, at: number): SensorStatus {
  const v = of_SensorStatus(r.case(at, 5));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}

export function load_Message(r: Lift, at: number): Message {
  const v = of_Message(r.case(at, 7));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}

export interface LonLat {
  lon: number;
  lat: number;
}

export const LON_LAT_SZ = 16;
export const LON_LAT_ALIGN = 8;

export function store_LonLat(w: Lower, at: number, v: LonLat): void {
  w.f64(at, v.lon);
  w.f64(at + 8, v.lat);
}

export function load_LonLat(r: Lift, at: number): LonLat {
  return {
    lon: r.f64(at),
    lat: r.f64(at + 8),
  };
}

export interface PivotState {
  status: GroundStatus;
  is_off_center: boolean;
  lon_lat: LonLat | null;
  outside: boolean;
}

export const PIVOT_STATE_SZ = 40;
export const PIVOT_STATE_ALIGN = 8;

export function store_PivotState(w: Lower, at: number, v: PivotState): void {
  w.u8(at, v.status);
  w.bool(at + 1, v.is_off_center);
  if (v.lon_lat !== null) {
    w.u8(at + 8, 1);
    store_LonLat(w, at + 8 + 8, v.lon_lat);
  }
  w.bool(at + 32, v.outside);
}

export function load_PivotState(r: Lift, at: number): PivotState {
  return {
    status: load_GroundStatus(r, at),
    is_off_center: r.bool(at + 1),
    lon_lat: r.case(at + 8, 2) === 0 ? null : load_LonLat(r, at + 8 + 8),
    outside: r.bool(at + 32),
  };
}

export interface LocationState {
  status: SensorStatus;
  has_location: boolean;
  watching: boolean;
  err: Message | null;
}

export const LOCATION_STATE_SZ = 5;
export const LOCATION_STATE_ALIGN = 1;

export function store_LocationState(
  w: Lower,
  at: number,
  v: LocationState,
): void {
  w.u8(at, v.status);
  w.bool(at + 1, v.has_location);
  w.bool(at + 2, v.watching);
  if (v.err !== null) {
    w.u8(at + 3, 1);
    w.u8(at + 3 + 1, v.err);
  }
}

export function load_LocationState(r: Lift, at: number): LocationState {
  return {
    status: load_SensorStatus(r, at),
    has_location: r.bool(at + 1),
    watching: r.bool(at + 2),
    err: r.case(at + 3, 2) === 0 ? null : load_Message(r, at + 3 + 1),
  };
}

export interface CompanionState {
  person_ready: boolean;
  pet_ready: boolean;
}

export const COMPANION_STATE_SZ = 2;
export const COMPANION_STATE_ALIGN = 1;

export function store_CompanionState(
  w: Lower,
  at: number,
  v: CompanionState,
): void {
  w.bool(at, v.person_ready);
  w.bool(at + 1, v.pet_ready);
}

export function load_CompanionState(r: Lift, at: number): CompanionState {
  return {
    person_ready: r.bool(at),
    pet_ready: r.bool(at + 1),
  };
}

export interface OrientationState {
  status: SensorStatus;
}

export const ORIENTATION_STATE_SZ = 1;
export const ORIENTATION_STATE_ALIGN = 1;

export function store_OrientationState(
  w: Lower,
  at: number,
  v: OrientationState,
): void {
  w.u8(at, v.status);
}

export function load_OrientationState(r: Lift, at: number): OrientationState {
  return {
    status: load_SensorStatus(r, at),
  };
}

export interface DivisionPart {
  subtype: number;
  name: string;
  shown: boolean;
  area: boolean;
}

export const DIVISION_PART_SZ = 16;
export const DIVISION_PART_ALIGN = 4;

export function store_DivisionPart(
  w: Lower,
  at: number,
  v: DivisionPart,
): void {
  w.u8(at, v.subtype);
  w.str(at + 4, v.name);
  w.bool(at + 12, v.shown);
  w.bool(at + 13, v.area);
}

export function load_DivisionPart(r: Lift, at: number): DivisionPart {
  return {
    subtype: r.u8(at),
    name: r.str(at + 4),
    shown: r.bool(at + 12),
    area: r.bool(at + 13),
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
  settings: Uint32Array<ArrayBuffer>;
  division: DivisionPart[];
  character_surfaces: Uint32Array<ArrayBuffer>;
  stall: Message | null;
  renderer_progress: number;
  area_progress: number;
}

export const SNAPSHOT_SZ = 112;
export const SNAPSHOT_ALIGN = 8;

export function store_Snapshot(w: Lower, at: number, v: Snapshot): void {
  store_PivotState(w, at, v.pivot);
  store_LocationState(w, at + 40, v.location);
  store_CompanionState(w, at + 45, v.companion);
  if (v.draft !== null) {
    w.u8(at + 48, 1);
    w.str(at + 48 + 4, v.draft);
  }
  store_OrientationState(w, at + 60, v.orient);
  w.bool(at + 61, v.is_follow_mode);
  w.bool(at + 62, v.camera_busy);
  w.bool(at + 63, v.zooming);
  w.bool(at + 64, v.over_marker);
  w.arr(at + 68, v.settings);
  {
    const b0 = w.list(
      at + 76,
      v.division.length,
      DIVISION_PART_SZ,
      DIVISION_PART_ALIGN,
    );
    if (b0 !== null)
      v.division.forEach((x0, i0) => {
        store_DivisionPart(w, b0 + i0 * DIVISION_PART_SZ, x0);
      });
  }
  w.arr(at + 84, v.character_surfaces);
  if (v.stall !== null) {
    w.u8(at + 92, 1);
    w.u8(at + 92 + 1, v.stall);
  }
  w.f64(at + 96, v.renderer_progress);
  w.f64(at + 104, v.area_progress);
}

export function load_Snapshot(r: Lift, at: number): Snapshot {
  return {
    pivot: load_PivotState(r, at),
    location: load_LocationState(r, at + 40),
    companion: load_CompanionState(r, at + 45),
    draft: r.case(at + 48, 2) === 0 ? null : r.str(at + 48 + 4),
    orient: load_OrientationState(r, at + 60),
    is_follow_mode: r.bool(at + 61),
    camera_busy: r.bool(at + 62),
    zooming: r.bool(at + 63),
    over_marker: r.bool(at + 64),
    settings: r.arr(at + 68, Uint32Array),
    division: r.map(at + 76, DIVISION_PART_SZ, DIVISION_PART_ALIGN, (at) =>
      load_DivisionPart(r, at),
    ),
    character_surfaces: r.arr(at + 84, Uint32Array),
    stall: r.case(at + 92, 2) === 0 ? null : load_Message(r, at + 92 + 1),
    renderer_progress: r.f64(at + 96),
    area_progress: r.f64(at + 104),
  };
}

export interface Host {
  show(snapshot: Snapshot): void;
  view(azimuth: number, m_per_px: number): void;
}

/** The object over `implementation`. */
function object(implementation: Host): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case SHOW: {
            if (args === null) return ERRNO_EBADMSG;
            const r_ = new Lift(args);
            implementation.show(load_Snapshot(r_, 0));
            return 0;
          }
          case VIEW: {
            if (args === null) return ERRNO_EBADMSG;
            const r_ = new Lift(args);
            implementation.view(r_.f64(0), r_.f64(8));
            return 0;
          }
          default:
            return ERRNO_ENOSYS;
        }
      } catch (e) {
        return refusal_of(e);
      }
    },
    drop() {},
  };
}

/** Serves `implementation` under the interface's handle. */
export function mount(
  l: Pick<Link, "mount_named">,
  implementation: Host,
): void {
  l.mount_named(HANDLE, object(implementation));
}
