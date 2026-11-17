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
  coord: [f64, f64];
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
function isDeltaOver(
  a: f64 | null,
  b: f64 | null,
  eps: f64,
  dist: (a: f64, b: f64) => f64,
): boolean {
  if (a !== null && b !== null) return dist(a, b) > eps;
  return a !== b;
}

function plainDelta(a: f64, b: f64): f64 {
  return Math.abs(a - b);
}

/** How far apart two headings lie on the circle: 359° and 1° are 2° apart,
 * where a plain difference would make them 358°. */
function headingDelta(a: f64, b: f64): f64 {
  return Math.abs(wrap(a - b, -180, 180));
}

/** The location's state as the UI shows it: its status, and the locale key of
 * what failed when it failed. `error` is null except at `SENSOR_STATUS_ERROR`. */
export interface SensorState {
  status: LocationSensorStatus;
  error: string | null;
}

export function ofSensorState(status: LocationSensorStatus): SensorState {
  return { status, error: null };
}

export function failedSensorState(error: string): SensorState {
  return { status: SENSOR_STATUS_ERROR, error };
}

export interface DeviceLocationStore {
  isTrackingLocation: boolean;
  location: DeviceLocation | null;
  state: SensorState;
}

/** A sensor is idle until it is asked to start. */
export function defaultDeviceLocationStore(): DeviceLocationStore {
  return {
    isTrackingLocation: false,
    location: null,
    state: ofSensorState(SENSOR_STATUS_IDLE),
  };
}

/** Takes a fix. The location changes only on a significant change, but any
 * fix marks the sensor Ready, since it shows the watch delivers. */
export function setLocation(
  s: DeviceLocationStore,
  coord: [f64, f64],
  alt: f64 | null,
  accuracy: f64,
  course: f64 | null,
): void {
  s.state = ofSensorState(SENSOR_STATUS_READY);
  const prev = s.location;
  if (prev === null) {
    s.location = { coord, alt, accuracy, course };
    return;
  }

  // A fix is accepted when its accuracy is within 100 m, or when the
  // accuracy improves.
  const isImproving = accuracy < prev.accuracy;
  if (accuracy > 100 && !isImproving) return;

  // The dead zone is about 1.1 m horizontally and 0.5 m in altitude.
  const longitudeDelta = Math.abs(prev.coord[0] - coord[0]);
  const latitudeDelta = Math.abs(prev.coord[1] - coord[1]);
  const isSignificantChange =
    latitudeDelta > 0.00001 ||
    longitudeDelta > 0.00001 ||
    isDeltaOver(prev.alt, alt, 0.5, plainDelta);

  if (
    !isSignificantChange &&
    prev.accuracy === accuracy &&
    prev.course === course
  )
    return;

  // Without movement, the accuracy and course still refresh, so the UI shows
  // the freshest signal.
  s.location = {
    coord: isSignificantChange ? coord : prev.coord,
    alt: isSignificantChange ? alt : prev.alt,
    accuracy,
    course,
  };
}

export interface DeviceOrientationStore {
  isTrackingOrientation: boolean;
  orientation: DeviceOrientation | null;
  status: SensorStatus;
}

/** The device's orientation is idle until it is asked to start. */
export function defaultDeviceOrientationStore(): DeviceOrientationStore {
  return {
    isTrackingOrientation: false,
    orientation: null,
    status: SENSOR_STATUS_IDLE,
  };
}

/** Takes a reading, ignoring a change of heading within a dead zone of 1.5°.
 * The first reading makes the sensor Ready, even one without a heading. */
export function setOrientation(
  s: DeviceOrientationStore,
  heading: f64 | null,
): void {
  const prev = s.orientation;
  if (prev === null) {
    s.orientation = { heading };
    s.status = SENSOR_STATUS_READY;
    return;
  }
  if (isDeltaOver(heading, prev.heading, 1.5, headingDelta))
    s.orientation = { heading };
}

export function setIsTrackingOrientation(
  s: DeviceOrientationStore,
  enabled: boolean,
): void {
  if (s.isTrackingOrientation === enabled) return;
  s.isTrackingOrientation = enabled;
  if (!enabled) s.orientation = null;
}

const LOCATION: DeviceLocationStore = defaultDeviceLocationStore();
const ORIENTATION: DeviceOrientationStore = defaultDeviceOrientationStore();

export function deviceLocationStore(): DeviceLocationStore {
  return LOCATION;
}

export function deviceOrientationStore(): DeviceOrientationStore {
  return ORIENTATION;
}
