// @pxl/client/abi/overlay.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import {
  type CameraState,
  type Message,
  of_CameraState,
  of_Message,
} from "./overlay.enum.ts";
import type { TrackingMode } from "./types.enum.ts";
import { load_TrackingMode } from "./types.ts";
import {
  ERRNO_EBADMSG,
  ERRNO_EINPROGRESS,
  ERRNO_ENOSYS,
} from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import type { Lift, Lower } from "@xpute/link/cabi.ts";
import {
  type Callee,
  type Handle,
  type LinkObject,
  type Method,
  refusal_of,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/overlay` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x7b1393;

/** `show` */
export const SHOW: Method = 0;
/** `draft` */
export const DRAFT: Method = 1;
/** `view` */
export const VIEW: Method = 2;

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
  ground_shown: boolean;
  lon_lat: LonLat | null;
  outside: boolean;
}

export const PIVOT_STATE_SZ = 40;
export const PIVOT_STATE_ALIGN = 8;

export function store_PivotState(w: Lower, at: number, v: PivotState): void {
  w.bool(at, v.ground_shown);
  if (v.lon_lat !== null) {
    w.u8(at + 8, 1);
    store_LonLat(w, at + 8 + 8, v.lon_lat);
  }
  w.bool(at + 32, v.outside);
}

export function load_PivotState(r: Lift, at: number): PivotState {
  return {
    ground_shown: r.bool(at),
    lon_lat: r.case(at + 8, 2) === 0 ? null : load_LonLat(r, at + 8 + 8),
    outside: r.bool(at + 32),
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

export function load_CameraState(r: Lift, at: number): CameraState {
  const v = of_CameraState(r.case(at, 3));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}

export interface Snapshot {
  pivot: PivotState;
  camera: CameraState;
  companion_ready: boolean;
  drafting: boolean;
  tracking_mode: TrackingMode;
  over_marker: boolean;
  settings: Uint32Array<ArrayBuffer>;
  division: DivisionPart[];
  character_surfaces: Uint32Array<ArrayBuffer>;
  stall: Message | null;
  renderer_progress: number;
  area_progress: number;
}

export const SNAPSHOT_SZ = 96;
export const SNAPSHOT_ALIGN = 8;

export function store_Snapshot(w: Lower, at: number, v: Snapshot): void {
  store_PivotState(w, at, v.pivot);
  w.u8(at + 40, v.camera);
  w.bool(at + 41, v.companion_ready);
  w.bool(at + 42, v.drafting);
  w.u8(at + 43, v.tracking_mode);
  w.bool(at + 44, v.over_marker);
  w.arr(at + 48, v.settings);
  {
    const b0 = w.list(
      at + 56,
      v.division.length,
      DIVISION_PART_SZ,
      DIVISION_PART_ALIGN,
    );
    if (b0 !== null)
      v.division.forEach((x0, i0) => {
        store_DivisionPart(w, b0 + i0 * DIVISION_PART_SZ, x0);
      });
  }
  w.arr(at + 64, v.character_surfaces);
  if (v.stall !== null) {
    w.u8(at + 72, 1);
    w.u8(at + 72 + 1, v.stall);
  }
  w.f64(at + 80, v.renderer_progress);
  w.f64(at + 88, v.area_progress);
}

export function load_Snapshot(r: Lift, at: number): Snapshot {
  return {
    pivot: load_PivotState(r, at),
    camera: load_CameraState(r, at + 40),
    companion_ready: r.bool(at + 41),
    drafting: r.bool(at + 42),
    tracking_mode: load_TrackingMode(r, at + 43),
    over_marker: r.bool(at + 44),
    settings: r.arr(at + 48, Uint32Array),
    division: r.map(at + 56, DIVISION_PART_SZ, DIVISION_PART_ALIGN, (at) =>
      load_DivisionPart(r, at),
    ),
    character_surfaces: r.arr(at + 64, Uint32Array),
    stall: r.case(at + 72, 2) === 0 ? null : load_Message(r, at + 72 + 1),
    renderer_progress: r.f64(at + 80),
    area_progress: r.f64(at + 88),
  };
}

export interface Host {
  show(snapshot: Snapshot, signal_: AbortSignal): Promise<void>;
  draft(text: string, signal_: AbortSignal): Promise<void>;
  view(azimuth: number, m_per_px: number, signal_: AbortSignal): Promise<void>;
}

/** The object over `implementation`. */
function object(
  l: Pick<Callee, "task_return" | "on_cancel">,
  implementation: Host,
): LinkObject {
  return {
    call(m, tag, args, _ret) {
      try {
        switch (m) {
          case SHOW: {
            const stop = new AbortController();
            implementation.show(load_Snapshot(args, 0), stop.signal).then(
              () => l.task_return(tag, 0, null),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return ERRNO_EINPROGRESS;
          }
          case DRAFT: {
            const stop = new AbortController();
            implementation.draft(args.str(0), stop.signal).then(
              () => l.task_return(tag, 0, null),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return ERRNO_EINPROGRESS;
          }
          case VIEW: {
            const stop = new AbortController();
            implementation.view(args.f64(0), args.f64(8), stop.signal).then(
              () => l.task_return(tag, 0, null),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return ERRNO_EINPROGRESS;
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

/** Mounts `implementation` under the interface's handle. */
export function mount(
  l: Pick<Callee, "mount_named" | "task_return" | "on_cancel">,
  implementation: Host,
): void {
  l.mount_named(HANDLE, object(l, implementation));
}
