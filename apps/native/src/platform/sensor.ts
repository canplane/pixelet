// @pxl/native/platform/sensor.ts

/**
 * The device's two sensors as the OS grants them, through expo-location: the
 * position the kernel's geolocation watches and the heading its compass
 * does (crates/pxl/web/kernel/src/control/sensor). Each crosses as the page's
 * do (platform/web/sensor.ts), so the kernel hears a device and a browser the
 * same way — the heading in the place Safari's `webkitCompassHeading` takes.
 *
 * The OS asks for the one permission both need, on the first watch; a
 * refusal is the kernel's geolocation error, as a browser's is.
 */

import * as Location from "expo-location";

import {
  Command,
  encode_sensor_error,
  encode_sensor_orientation,
  encode_sensor_permission,
  encode_sensor_update,
  GeolocationErrorCode,
  type PermissionAnswer,
  type Sensor,
} from "@/kernel/abi/cmd.spec.ts";
import { CORE_BUILD } from "@/kernel/ipc.ts";
import { request_frame } from "@/kernel/sched.ts";

export function geolocation_supported(): boolean {
  return true;
}

export function orientation_supported(): boolean {
  return true;
}

/** The heading needs no prompt of its own: the location permission covers it. */
export function orientation_needs_permission(): boolean {
  return false;
}

export function orientation_permit(): Promise<boolean> {
  return Promise.resolve(true);
}

export function send_permission(sensor: Sensor, answer: PermissionAnswer): void {
  CORE_BUILD.send(0, Command.SENSOR_PERMISSION, encode_sensor_permission({ sensor, answer }));
  request_frame();
}

/** Whether the OS lets the app read where it is, asking the first time. */
async function permitted(): Promise<boolean> {
  const { status } = await Location.requestForegroundPermissionsAsync();
  return status === Location.PermissionStatus.GRANTED;
}

function send_error(code: GeolocationErrorCode): void {
  CORE_BUILD.send(0, Command.SENSOR_ERROR, encode_sensor_error({ code }));
  request_frame();
}

/** A watch, or one being started: a stop while the permission is still being
 * asked for ends the start where it lands rather than leaving a watch no one
 * holds. */
interface Watch {
  subscription: Location.LocationSubscription | null;
  stopped: boolean;
}

let geolocation: Watch | null = null;

export function geolocation_start(): void {
  if (geolocation !== null) return;
  const watch: Watch = { subscription: null, stopped: false };
  geolocation = watch;
  void (async () => {
    if (!(await permitted())) {
      if (!watch.stopped) send_error(GeolocationErrorCode.PermissionDenied);
      return;
    }
    const subscription = await Location.watchPositionAsync(
      { accuracy: Location.Accuracy.BestForNavigation, timeInterval: 1000, distanceInterval: 0 },
      (pos) => {
        const { latitude, longitude, altitude, accuracy, heading } = pos.coords;
        CORE_BUILD.send(0, Command.SENSOR_UPDATE, encode_sensor_update({ lon: longitude, lat: latitude, altitude, accuracy, heading }));
        request_frame();
      },
      () => send_error(GeolocationErrorCode.PositionUnavailable),
    );
    if (watch.stopped) subscription.remove();
    else watch.subscription = subscription;
  })().catch(() => send_error(GeolocationErrorCode.Other));
}

export function geolocation_stop(): void {
  if (geolocation === null) return;
  geolocation.stopped = true;
  geolocation.subscription?.remove();
  geolocation = null;
}

let orientation: Watch | null = null;

export function orientation_listen(): void {
  if (orientation !== null) return;
  const watch: Watch = { subscription: null, stopped: false };
  orientation = watch;
  void (async () => {
    if (!(await permitted())) return;
    const subscription = await Location.watchHeadingAsync((h) => {
      // True north where the OS has a fix to correct by, magnetic otherwise.
      const heading = h.trueHeading >= 0 ? h.trueHeading : h.magHeading;
      CORE_BUILD.send(0, Command.SENSOR_ORIENTATION, encode_sensor_orientation({ webkit_compass_heading: heading, alpha: null, beta: null, gamma: null }));
      request_frame();
    });
    if (watch.stopped) subscription.remove();
    else watch.subscription = subscription;
  })().catch((err) => console.error("[sensor] heading:", err));
}

export function orientation_remove(): void {
  if (orientation === null) return;
  orientation.stopped = true;
  orientation.subscription?.remove();
  orientation = null;
}
