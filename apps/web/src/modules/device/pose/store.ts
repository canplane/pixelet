// @pxl/web/modules/device/pose/store.ts
// global store for the device's physical sensors (GPS, compass) — data and reception state.

import { create } from "zustand";

import type { f64 } from "@xpute/core/kit/word.ts";

import type { G4326 } from "@pxl/shared/spec/space/adapter/4326.ts";

export const enum DevicePoseStatus {
  IDLE = "idle",
  LOADING = "loading",
  READY = "ready",
  ERROR = "error",
}

export type DeviceLocation = {
  /** Current latitude/longitude/altitude. */
  coords: G4326;
  /** GPS accuracy, in meters. */
  accuracy: f64;
  /** Heading of travel from GPS course, 0-360 degrees. */
  course: f64 | null;

  timestamp: f64;
};

export type DeviceOrientation = {
  /** 0-360: compass heading (Z rotation). */
  heading: f64 | null;
  /** -180-180: front/back tilt (X rotation). */
  beta: f64 | null;
  /** -90-90: left/right tilt (Y rotation). */
  gamma: f64 | null;

  timestamp: f64;
};

function is_delta_over(a: f64 | null, b: f64 | null, eps: f64): boolean {
  if (a === null || b === null) return a !== b;
  return Math.abs(a - b) > eps;
}

/** Shortest distance between two values on a circle of the given period (e.g. 360 for compass degrees) — unlike a plain subtraction, this stays small across the wrap (359 vs 1 is a 2-unit gap, not 358). */
function circular_delta(a: f64, b: f64, period: f64): f64 {
  const raw: f64 = Math.abs(a - b) % period;
  return raw > period / 2 ? period - raw : raw;
}

/** Same dead-zone check as is_delta_over, but for a circular (wrapping) value like compass heading. Plain is_delta_over would see 359° vs 1° as a 358° jump and let every bit of sensor noise through right around due north. */
function is_heading_delta_over(a: f64 | null, b: f64 | null, eps: f64): boolean {
  if (a === null || b === null) return a !== b;
  return circular_delta(a, b, 360) > eps;
}

type DeviceLocationState = {
  // ---- Geolocation: lat/lng/altitude data ----

  /** Whether location tracking is active. */
  is_tracking_location: boolean;
  /** Updates on GPS data reception, after filtering. */
  set_is_tracking_location: (enabled: boolean) => void;

  location: DeviceLocation | null;
  set_location: (coords: G4326, accuracy: f64, course: f64 | null) => void;

  // ---- Lifecycle state: shared status/message ----
  /** Overall location-service reception status. */
  status: DevicePoseStatus;
  /** System message (error detail, etc). */
  error_msg: string | null;

  // ---- Common actions ----
  /** Service status transition (idle, loading, ready). */
  set_status: (status: DevicePoseStatus) => void;
  /** Records an error on failure. */
  set_error: (error_msg: string) => void;
};

type DeviceOrientationState = {
  // ---- Orientation: compass and permission state ----

  /** Whether the compass listener is registered and running. */
  is_tracking_orientation: boolean;
  /** Orientation tracking state change */
  set_is_tracking_orientation: (enabled: boolean) => void;

  orientation: DeviceOrientation | null;
  /** Updates compass sensor data (dead-zone filtered). */
  set_orientation: (heading: f64 | null, beta: f64 | null, gamma: f64 | null) => void;

  // ---- Lifecycle state: shared status/message ----
  /** Overall location-service reception status. */
  status: DevicePoseStatus;
  /** System message (error detail, etc). */
  error_msg: string | null;

  // ---- Common actions ----
  /** Service status transition (idle, loading, ready). */
  set_status: (status: DevicePoseStatus) => void;
  /** Records an error on failure. */
  set_error: (error_msg: string) => void;
};

export const useDeviceLocationStore = create<DeviceLocationState>((set) => ({
  is_tracking_location: false,
  location: null,

  status: DevicePoseStatus.IDLE,
  error_msg: null,

  /** Validates raw GPS data from hardware, and only updates state on a significant change. */
  set_location: (coords, accuracy, course) =>
    set((state) => {
      const timestamp: f64 = performance.now();
      const prev: DeviceLocation | null = state.location;

      // 1. First-fix pass
      if (!prev) {
        return {
          location: { coords, accuracy, course, timestamp },

          status: DevicePoseStatus.READY,
          error_msg: null,
        };
      }

      // 2. Accuracy guard (accept if < 100m, or if accuracy is improving)
      const is_improving = accuracy < prev.accuracy;
      if (accuracy > 100 && !is_improving) return state;

      // 3. Dead-zone filtering (~1.1m for lat/lng, 0.5m for altitude)
      const lng_diff: f64 = Math.abs(prev.coords[0] - coords[0]);
      const lat_diff: f64 = Math.abs(prev.coords[1] - coords[1]);
      const alt_diff: f64 = Math.abs((prev.coords[2] ?? 0) - (coords[2] ?? 0));

      const is_significant_change: boolean = lat_diff > 0.00001 || lng_diff > 0.00001 || alt_diff > 0.5;

      if (!is_significant_change && prev.accuracy === accuracy && prev.course === course) {
        return state;
      }

      // 4. Update strategy: even without significant movement, refresh
      // metadata (accuracy/course/timestamp) so the UI has the freshest
      // signal info.
      return {
        location: {
          coords: is_significant_change ? coords : prev.coords,
          accuracy,
          course,
          timestamp,
        },

        status: DevicePoseStatus.READY,
        error_msg: null,
      };
    }),

  set_is_tracking_location: (enabled) =>
    set((state) => {
      // no-op if unchanged, to avoid a re-render
      if (state.is_tracking_location === enabled) return state;

      // reset location to null on stop, to avoid stale data
      return {
        is_tracking_location: enabled,
        location: enabled ? state.location : null,
      };
    }),

  set_status: (status) =>
    set((state) => {
      // same reference back if unchanged (React optimization)
      if (state.status === status) return state;
      return { status };
    }),

  set_error: (error_msg) =>
    set((state) => {
      if (state.status === DevicePoseStatus.ERROR && state.error_msg === error_msg) return state;
      return { status: DevicePoseStatus.ERROR, error_msg };
    }),
}));

export const useDeviceOrientationStore = create<DeviceOrientationState>((set) => ({
  orientation: null,
  is_tracking_orientation: false,

  status: DevicePoseStatus.IDLE,
  error_msg: null,

  set_orientation: (heading, beta, gamma) =>
    set((state) => {
      const timestamp: f64 = performance.now();
      const prev: DeviceOrientation | null = state.orientation;

      if (!prev) {
        return {
          orientation: { heading, beta, gamma, timestamp },

          status: DevicePoseStatus.READY, // orientation data alone is enough to count as usable
          error_msg: null,
        };
      }

      // dead-zone filter, keyed on heading (the most important field): 1.5°
      // change, using the circular-aware delta since heading wraps
      const is_heading_changed: boolean = is_heading_delta_over(heading, prev.heading, 1.5);

      // tilt (beta/gamma) change check, ~2° — avoids visible jitter in the voxel renderer
      const is_tilt_changed: boolean = is_delta_over(beta, prev.beta, 2.0) || is_delta_over(gamma, prev.gamma, 2.0);

      if (!is_heading_changed && !is_tilt_changed) return state;

      return {
        orientation: { heading, beta, gamma, timestamp },
      };
    }),

  set_is_tracking_orientation: (enabled) =>
    set((state) => {
      // no-op if unchanged, to avoid a re-render
      if (state.is_tracking_orientation === enabled) return state;

      // reset orientation to null on stop
      return {
        is_tracking_orientation: enabled,
        orientation: enabled ? state.orientation : null,
      };
    }),

  set_status: (status) =>
    set((state) => {
      // same reference back if unchanged (React optimization)
      if (state.status === status) return state;
      return { status };
    }),

  set_error: (error_msg) =>
    set((state) => {
      if (state.status === DevicePoseStatus.ERROR && state.error_msg === error_msg) return state;
      return { status: DevicePoseStatus.ERROR, error_msg };
    }),
}));
