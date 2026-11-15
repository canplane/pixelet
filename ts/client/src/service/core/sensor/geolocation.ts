// @pxl/client/service/core/sensor/geolocation.ts

/** The GPS sensor's decisions: when a watch runs and what an error means.
 * The platform owns the watch itself, and says with each failure whether the
 * watch runs on (`SensorListener.pos_err`'s `watching`): a W3C watch rides out
 * all but `PERMISSION_DENIED`
 * (https://www.w3.org/TR/geolocation/#watchposition-method), and a device's
 * ends on any. Nothing is retried here. A watch that ended starts again on a
 * grant, a return to the foreground or a press of the location card. */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { GeolocationError } from "@/service/platform/types.ts";
import {
  device_location_store,
  failed_SensorState,
  of_SensorState,
  set_location,
  SENSOR_STATUS_IDLE,
  SENSOR_STATUS_LOADING,
} from "./reading.ts";

export type GeolocationCommand =
  | typeof GEOLOCATION_COMMAND_START
  | typeof GEOLOCATION_COMMAND_STOP;
export const GEOLOCATION_COMMAND_START = 0;
export const GEOLOCATION_COMMAND_STOP = 1;

interface Geolocation {
  /** The last command. The platform's watch follows it and ignores a
   * repeated command. */
  cmd: GeolocationCommand | null;
}

const geolocation: Geolocation = { cmd: null };

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
 * (`DeviceLocationStore.is_tracking_location`). */
export function start_watching(): void {
  const store = device_location_store();
  if (store.is_tracking_location) return;
  if (!PLATFORM.supported) {
    store.state = failed_SensorState("device.geolocation.unsupported");
    return;
  }
  // A watch started again after a hidden spell keeps the location it had and
  // reads as Loading until it gives a fix of its own.
  store.state = of_SensorState(SENSOR_STATUS_LOADING);
  store.is_tracking_location = true;
  geolocation.cmd = GEOLOCATION_COMMAND_START;
}

/** The location is kept: a watch stopped while the app is hidden is not the
 * location gone. It is dropped where the location ends (`geolocation_close`,
 * `geolocation_err`). */
export function stop_watching(): void {
  const store = device_location_store();
  if (store.is_tracking_location) {
    store.is_tracking_location = false;
    geolocation.cmd = GEOLOCATION_COMMAND_STOP;
  }
}

export function geolocation_close(): void {
  stop_watching();
  const store = device_location_store();
  store.location = null;
  store.state = of_SensorState(SENSOR_STATUS_IDLE);
}

/** A grant from outside the app, such as the device's settings, starts the
 * watch again. Without it, a denial would stick until the page reloads. */
export function on_geolocation_permission_granted(): void {
  if (!device_location_store().is_tracking_location) start_watching();
}

/** A location fix, its altitude over the ellipsoid as GPS gives it. */
export function geolocation_fix(
  lon: f64,
  lat: f64,
  alt: f64 | null,
  accuracy: f64,
  course: f64 | null,
): void {
  set_location(device_location_store(), [lon, lat], alt, accuracy, course);
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
export function geolocation_err(err: GeolocationError, watching: boolean): void {
  const store = device_location_store();
  store.state = failed_SensorState(ERR_KEY[err]);
  if (!watching) {
    store.is_tracking_location = false;
    store.location = null;
  }
}

export function take_geolocation_cmd(): GeolocationCommand | null {
  const c = geolocation.cmd;
  geolocation.cmd = null;
  return c;
}
