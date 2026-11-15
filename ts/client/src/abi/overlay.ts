// @pxl/client/abi/overlay.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import {
  type GroundStatus,
  type Message,
  of_GroundStatus,
  of_Message,
} from "./overlay.enum.ts";
import { ERRNO_EBADMSG, ERRNO_ENOSYS } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { Lift, type Lower } from "@xpute/link/cabi.ts";
import {
  type Handle,
  type LinkObject,
  type Method,
  type Side,
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

export function load_Message(r: Lift, at: number): Message {
  const v = of_Message(r.case(at, 2));
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
  companion: CompanionState;
  draft: string | null;
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

export const SNAPSHOT_SZ = 104;
export const SNAPSHOT_ALIGN = 8;

export function store_Snapshot(w: Lower, at: number, v: Snapshot): void {
  store_PivotState(w, at, v.pivot);
  store_CompanionState(w, at + 40, v.companion);
  if (v.draft !== null) {
    w.u8(at + 44, 1);
    w.str(at + 44 + 4, v.draft);
  }
  w.bool(at + 56, v.is_follow_mode);
  w.bool(at + 57, v.camera_busy);
  w.bool(at + 58, v.zooming);
  w.bool(at + 59, v.over_marker);
  w.arr(at + 60, v.settings);
  {
    const b0 = w.list(
      at + 68,
      v.division.length,
      DIVISION_PART_SZ,
      DIVISION_PART_ALIGN,
    );
    if (b0 !== null)
      v.division.forEach((x0, i0) => {
        store_DivisionPart(w, b0 + i0 * DIVISION_PART_SZ, x0);
      });
  }
  w.arr(at + 76, v.character_surfaces);
  if (v.stall !== null) {
    w.u8(at + 84, 1);
    w.u8(at + 84 + 1, v.stall);
  }
  w.f64(at + 88, v.renderer_progress);
  w.f64(at + 96, v.area_progress);
}

export function load_Snapshot(r: Lift, at: number): Snapshot {
  return {
    pivot: load_PivotState(r, at),
    companion: load_CompanionState(r, at + 40),
    draft: r.case(at + 44, 2) === 0 ? null : r.str(at + 44 + 4),
    is_follow_mode: r.bool(at + 56),
    camera_busy: r.bool(at + 57),
    zooming: r.bool(at + 58),
    over_marker: r.bool(at + 59),
    settings: r.arr(at + 60, Uint32Array),
    division: r.map(at + 68, DIVISION_PART_SZ, DIVISION_PART_ALIGN, (at) =>
      load_DivisionPart(r, at),
    ),
    character_surfaces: r.arr(at + 76, Uint32Array),
    stall: r.case(at + 84, 2) === 0 ? null : load_Message(r, at + 84 + 1),
    renderer_progress: r.f64(at + 88),
    area_progress: r.f64(at + 96),
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
  l: Pick<Side, "mount_named">,
  implementation: Host,
): void {
  l.mount_named(HANDLE, object(implementation));
}
