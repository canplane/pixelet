// @pxl/native/platform/sensor.ts

/**
 * The device's two sensors as the OS grants them, through expo-location: the
 * position the kernel's geolocation watches and the heading its compass
 * does (crates/pxl/web/kernel/src/control/sensor). Each crosses as the page's
 * do (platform/web/sensor.ts), so the kernel hears a device and a browser the
 * same way — the heading in the place Safari's `webkitCompassHeading` takes.
 *
 * The OS asks for the one permission both need, on the first watch. A watch
 * that ends — refused, failed — is let go here and reported, as a page clears
 * a dead geolocation watch: the kernel's next start begins a new one rather
 * than finding the old one still held. The compass is where a device and a
 * browser part: Core Location speaks only when the heading turns and says
 * when it fails, so its silence is no loss (`orientation_silence_is_loss`).
 */

import { requireNativeModule } from "expo";
import * as Location from "expo-location";

import { Command, encode_sensor_error, encode_sensor_orientation, encode_sensor_permission, encode_sensor_update, GeolocationErrorCode, PermissionAnswer, Sensor } from "@/kernel/abi/cmd.spec.ts";
import { CORE_BUILD } from "@/kernel/ipc.ts";
import { request_frame, ring_now } from "@/kernel/sched.ts";

interface PxlSensor {
  headingAvailable(): boolean;
}

const SENSOR: PxlSensor = requireNativeModule("PxlSensor");

export function geolocation_supported(): boolean {
  return true;
}

export function orientation_supported(): boolean {
  return SENSOR.headingAvailable();
}

/** The heading needs no prompt of its own: the location permission covers it. */
export function orientation_needs_permission(): boolean {
  return false;
}

export function orientation_silence_is_loss(): boolean {
  return false;
}

export function orientation_permit(): Promise<boolean> {
  return Promise.resolve(true);
}

export function send_permission(sensor: Sensor, answer: PermissionAnswer): void {
  CORE_BUILD.send(0, Command.SENSOR_PERMISSION, encode_sensor_permission({ sensor, answer }));
  request_frame();
}

/** The permission as last seen: what a return to the app compares against. */
let granted: boolean | null = null;

/** Whether the OS lets the app read where it is, asking the first time. */
async function permitted(): Promise<boolean> {
  const { status } = await Location.requestForegroundPermissionsAsync();
  granted = status === Location.PermissionStatus.GRANTED;
  return granted;
}

/**
 * The app is in front again. A grant made in the device's settings meanwhile
 * reaches the kernel as a page's permission `change` does
 * (kernel/control/input.ts) — the one way back from a refusal, since nothing
 * else would ask again.
 */
export function permission_recheck(): void {
  if (granted !== false) return;
  void Location.getForegroundPermissionsAsync().then(({ status }) => {
    if (granted !== false || status !== Location.PermissionStatus.GRANTED) return;
    granted = true;
    send_permission(Sensor.Geolocation, PermissionAnswer.Granted);
    send_permission(Sensor.Orientation, PermissionAnswer.Granted);
  });
}

/** A watch, or one being started: a stop while the permission is still being
 * asked for ends the start where it lands rather than leaving a watch no one
 * holds. */
interface Watch {
  subscription: Location.LocationSubscription | null;
  stopped: boolean;
}

function watch_end(watch: Watch): void {
  watch.stopped = true;
  watch.subscription?.remove();
  watch.subscription = null;
}

let geolocation: Watch | null = null;

/** The watch ended on its own: let go of it, and the kernel hears why. A
 * watch already stopped or replaced says nothing. */
function geolocation_fail(watch: Watch, code: GeolocationErrorCode): void {
  if (geolocation !== watch) return;
  watch_end(watch);
  geolocation = null;
  CORE_BUILD.send(0, Command.SENSOR_ERROR, encode_sensor_error({ code }));
  request_frame();
}

export function geolocation_start(): void {
  if (geolocation !== null) return;
  const watch: Watch = { subscription: null, stopped: false };
  geolocation = watch;
  void (async () => {
    if (!(await permitted())) return geolocation_fail(watch, GeolocationErrorCode.PermissionDenied);
    const subscription = await Location.watchPositionAsync(
      { accuracy: Location.Accuracy.BestForNavigation, timeInterval: 1000, distanceInterval: 0 },
      (pos) => {
        const { latitude, longitude, altitude, accuracy, heading } = pos.coords;
        CORE_BUILD.send(0, Command.SENSOR_UPDATE, encode_sensor_update({ lon: longitude, lat: latitude, altitude, accuracy: accuracy ?? Infinity, heading }));
        request_frame();
      },
      () => geolocation_fail(watch, GeolocationErrorCode.PositionUnavailable),
    );
    if (watch.stopped) subscription.remove();
    else watch.subscription = subscription;
  })().catch(() => geolocation_fail(watch, GeolocationErrorCode.Other));
}

export function geolocation_stop(): void {
  if (geolocation === null) return;
  watch_end(geolocation);
  geolocation = null;
}

let orientation: Watch | null = null;

/** The compass ended on its own: a refusal is the permission's answer, and a
 * failure is the compass lost. */
function orientation_fail(watch: Watch, refused: boolean): void {
  if (orientation !== watch) return;
  watch_end(watch);
  orientation = null;
  if (refused) send_permission(Sensor.Orientation, PermissionAnswer.Denied);
  else ring_now(Command.SENSOR_ORIENTATION_LOST);
}

export function orientation_listen(): void {
  if (orientation !== null) return;
  const watch: Watch = { subscription: null, stopped: false };
  orientation = watch;
  void (async () => {
    if (!(await permitted())) return orientation_fail(watch, true);
    const subscription = await Location.watchHeadingAsync(
      (h) => {
        // True north where the OS has a fix to correct by, magnetic otherwise.
        const heading = h.trueHeading >= 0 ? h.trueHeading : h.magHeading;
        CORE_BUILD.send(0, Command.SENSOR_ORIENTATION, encode_sensor_orientation({ webkit_compass_heading: heading, alpha: null, beta: null, gamma: null }));
        request_frame();
      },
      (reason) => {
        console.error("[sensor] heading:", reason);
        orientation_fail(watch, false);
      },
    );
    if (watch.stopped) subscription.remove();
    else watch.subscription = subscription;
  })().catch((err) => {
    console.error("[sensor] heading:", err);
    orientation_fail(watch, false);
  });
}

export function orientation_remove(): void {
  if (orientation === null) return;
  watch_end(orientation);
  orientation = null;
}
