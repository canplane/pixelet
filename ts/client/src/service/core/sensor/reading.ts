// @pxl/client/service/core/sensor/reading.ts

/** The device's GPS and compass readings, and which changes are significant
 * enough to keep. */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { wrap } from "@xpute/kit/math/scalar.ts";

/** A sensor's status: idle until it is asked to start, loading until a
 * reading, ready once one came, error where it failed, and unavailable where
 * the place has no compass. */
export type SensorStatus =
  | typeof SENSOR_STATUS_IDLE
  | typeof SENSOR_STATUS_LOADING
  | typeof SENSOR_STATUS_READY
  | typeof SENSOR_STATUS_ERROR
  | typeof SENSOR_STATUS_UNAVAILABLE;
export const SENSOR_STATUS_IDLE = 0;
export const SENSOR_STATUS_LOADING = 1;
export const SENSOR_STATUS_READY = 2;
export const SENSOR_STATUS_ERROR = 3;
export const SENSOR_STATUS_UNAVAILABLE = 4;

/** The location's status, never unavailable: a place without a location
 * reads as failed. */
export type LocationSensorStatus = Exclude<
  SensorStatus,
  typeof SENSOR_STATUS_UNAVAILABLE
>;

interface DeviceLocation {
  /** Longitude and latitude, in degrees. */
  lon_lat: [f64, f64];
  /** Meters over the ellipsoid. Null where the device gives no altitude. */
  alt: f64 | null;
  /** The fix's accuracy, in meters. */
  accuracy: f64;
  /** Course over ground, 0-360 degrees. */
  course: f64 | null;
}

interface DeviceOrientation {
  /** Compass heading, 0-360 degrees, or null where the reading gave none. */
  heading: f64 | null;
}

/** Whether `a` lies more than `eps` from `b` by `dist`; a value that appears
 * or goes is always a change. */
function is_delta_over(
  a: f64 | null,
  b: f64 | null,
  eps: f64,
  dist: (a: f64, b: f64) => f64,
): boolean {
  if (a !== null && b !== null) return dist(a, b) > eps;
  return a !== b;
}

function plain_delta(a: f64, b: f64): f64 {
  return Math.abs(a - b);
}

/** How far apart two headings lie on the circle: 359° and 1° are 2° apart,
 * where a plain difference would make them 358°. */
function heading_delta(a: f64, b: f64): f64 {
  return Math.abs(wrap(a - b, -180, 180));
}

/** The location's state as the UI shows it: its status, and the locale key of
 * what failed when it failed. `err` is null except at `SENSOR_STATUS_ERROR`. */
export interface SensorState {
  status: LocationSensorStatus;
  err: string | null;
}

export function of_SensorState(status: LocationSensorStatus): SensorState {
  return { status, err: null };
}

export function failed_SensorState(err: string): SensorState {
  return { status: SENSOR_STATUS_ERROR, err };
}

export interface DeviceLocationStore {
  is_tracking_location: boolean;
  location: DeviceLocation | null;
  state: SensorState;
}

/** A sensor is idle until it is asked to start. */
export function default_DeviceLocationStore(): DeviceLocationStore {
  return {
    is_tracking_location: false,
    location: null,
    state: of_SensorState(SENSOR_STATUS_IDLE),
  };
}

/** Takes a fix. The location changes only on a significant change, but any
 * fix marks the sensor Ready, since it shows the watch delivers. */
export function set_location(
  s: DeviceLocationStore,
  lon_lat: [f64, f64],
  alt: f64 | null,
  accuracy: f64,
  course: f64 | null,
): void {
  s.state = of_SensorState(SENSOR_STATUS_READY);
  const prev = s.location;
  if (prev === null) {
    s.location = { lon_lat, alt, accuracy, course };
    return;
  }

  // A fix is accepted when its accuracy is within 100 m, or when the
  // accuracy improves.
  const is_improving = accuracy < prev.accuracy;
  if (accuracy > 100 && !is_improving) return;

  // The dead zone is about 1.1 m horizontally and 0.5 m in altitude.
  const lon_diff = Math.abs(prev.lon_lat[0] - lon_lat[0]);
  const lat_diff = Math.abs(prev.lon_lat[1] - lon_lat[1]);
  const is_significant_change =
    lat_diff > 0.00001 ||
    lon_diff > 0.00001 ||
    is_delta_over(prev.alt, alt, 0.5, plain_delta);

  if (
    !is_significant_change &&
    prev.accuracy === accuracy &&
    prev.course === course
  )
    return;

  // Without movement, the accuracy and course still refresh, so the UI shows
  // the freshest signal.
  s.location = {
    lon_lat: is_significant_change ? lon_lat : prev.lon_lat,
    alt: is_significant_change ? alt : prev.alt,
    accuracy,
    course,
  };
}

export interface DeviceOrientationStore {
  is_tracking_orient: boolean;
  orient: DeviceOrientation | null;
  status: SensorStatus;
}

/** The device's orientation is idle until it is asked to start. */
export function default_DeviceOrientationStore(): DeviceOrientationStore {
  return { is_tracking_orient: false, orient: null, status: SENSOR_STATUS_IDLE };
}

/** Takes a reading, ignoring a change of heading within a dead zone of 1.5°.
 * The first reading makes the sensor Ready, even one without a heading. */
export function set_orient(
  s: DeviceOrientationStore,
  heading: f64 | null,
): void {
  const prev = s.orient;
  if (prev === null) {
    s.orient = { heading };
    s.status = SENSOR_STATUS_READY;
    return;
  }
  if (is_delta_over(heading, prev.heading, 1.5, heading_delta))
    s.orient = { heading };
}

export function set_is_tracking_orient(
  s: DeviceOrientationStore,
  enabled: boolean,
): void {
  if (s.is_tracking_orient === enabled) return;
  s.is_tracking_orient = enabled;
  if (!enabled) s.orient = null;
}

const LOCATION: DeviceLocationStore = default_DeviceLocationStore();
const ORIENT: DeviceOrientationStore = default_DeviceOrientationStore();

export function device_location_store(): DeviceLocationStore {
  return LOCATION;
}

export function device_orient_store(): DeviceOrientationStore {
  return ORIENT;
}
