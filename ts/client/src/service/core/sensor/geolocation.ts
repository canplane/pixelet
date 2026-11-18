// @pxl/client/service/core/sensor/geolocation.ts

/** The GPS sensor's decisions: when a watch runs and what an error means.
 * The platform owns the watch itself, and says with each failure whether the
 * watch runs on (`SensorListener.positionError`'s `watching`): a W3C watch rides out
 * all but `PERMISSION_DENIED`
 * (https://www.w3.org/TR/geolocation/#watchposition-method), and a native
 * one ends on any. Nothing is retried here. A watch that ended starts again
 * on a grant, a return to the foreground or a press of the location card. */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { GeolocationError } from "@/service/platform/types.ts";
import {
  deviceLocationStore,
  failedSensorState,
  ofSensorState,
  setLocation,
  SENSOR_STATUS_IDLE,
  SENSOR_STATUS_LOADING,
} from "./reading.ts";

export type GeolocationCommand =
  typeof GEOLOCATION_COMMAND_START | typeof GEOLOCATION_COMMAND_STOP;
export const GEOLOCATION_COMMAND_START = 0;
export const GEOLOCATION_COMMAND_STOP = 1;

interface Geolocation {
  /** The last command. The platform's watch follows it and ignores a
   * repeated command. */
  command: GeolocationCommand | null;
}

const geolocation: Geolocation = { command: null };

/** The location as the platform reports it once the client starts: whether
 * the platform has one. */
export interface GeolocationPlatform {
  supported: boolean;
}

/** Until the platform reports it, the place has no location. */
const PLATFORM: GeolocationPlatform = { supported: false };

export function platform(): GeolocationPlatform {
  return PLATFORM;
}

/** A watch runs while the store tracks the location
 * (`DeviceLocationStore.isTrackingLocation`). */
export function startWatching(): void {
  const store = deviceLocationStore();
  if (store.isTrackingLocation) return;
  if (!PLATFORM.supported) {
    store.state = failedSensorState("device.geolocation.unsupported");
    return;
  }
  // A watch started again after a hidden spell keeps the location it had and
  // reads as Loading until it gives a fix of its own.
  store.state = ofSensorState(SENSOR_STATUS_LOADING);
  store.isTrackingLocation = true;
  geolocation.command = GEOLOCATION_COMMAND_START;
}

/** The location is kept: a watch stopped while the app is hidden is not the
 * location gone. It is dropped where the location ends (`geolocationClose`,
 * `geolocationError`). */
export function stopWatching(): void {
  const store = deviceLocationStore();
  if (store.isTrackingLocation) {
    store.isTrackingLocation = false;
    geolocation.command = GEOLOCATION_COMMAND_STOP;
  }
}

export function geolocationClose(): void {
  stopWatching();
  const store = deviceLocationStore();
  store.location = null;
  store.state = ofSensorState(SENSOR_STATUS_IDLE);
}

/** A location fix, its altitude over the ellipsoid as GPS gives it. */
export function geolocationFix(
  lon: f64,
  lat: f64,
  altEllipsoid: f64 | null,
  accuracy: f64,
  course: f64 | null,
): void {
  setLocation(
    deviceLocationStore(),
    [lon, lat],
    altEllipsoid,
    accuracy,
    course,
  );
}

/** Each error's words, by their locale key. */
const ERR_KEY: Readonly<Record<GeolocationError, string>> = {
  denied: "device.geolocation.permission_denied",
  unavailable: "device.geolocation.generic_error",
  timeout: "device.geolocation.timeout",
  other: "device.geolocation.generic_error",
};

/** A location error. Whether the watch runs on past it (`watching`) is the
 * platform's to say. One it rides out leaves the watch running, and the
 * location reads as failed until the next fix. One that ended it leaves
 * nothing watching and no location, and a grant, a return to the foreground
 * or a press starts the watch again. */
export function geolocationError(
  error: GeolocationError,
  watching: boolean,
): void {
  const store = deviceLocationStore();
  store.state = failedSensorState(ERR_KEY[error]);
  if (!watching) {
    store.isTrackingLocation = false;
    store.location = null;
  }
}

export function takeGeolocationCommand(): GeolocationCommand | null {
  const c = geolocation.command;
  geolocation.command = null;
  return c;
}
