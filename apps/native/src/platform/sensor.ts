// @pxl/native/platform/sensor.ts

/**
 * The device's two sensors as the OS grants them, through
 * expo-horizon-location: the position the kernel's geolocation watches and
 * the heading its compass does (crates/pxl/web/kernel/src/control/sensor).
 * It is expo-location, forked so a Quest build reads the position without
 * Google Play Services, which Horizon OS has none of; a Quest has no GPS
 * either, and answers from the network, and no compass, which
 * `headingAvailable` says. Each crosses as the page's do
 * (platform/web/sensor.ts), so the kernel hears a device and a browser the
 * same way — the heading in the place Safari's `webkitCompassHeading` takes.
 *
 * The OS asks for the one permission both need, on the first watch. A watch
 * that ends — refused, failed — is let go here and reported, as a page clears
 * a dead geolocation watch: the kernel's next start begins a new one rather
 * than finding the old one still held. The compass is where a device and a
 * browser part: Core Location speaks only when the heading turns and says
 * when it fails, so its silence is no loss (`orientationSilenceIsLoss`).
 */

import { AppState } from "react-native";
import * as Location from "expo-horizon-location";

import { Command, encode_sensor_error, encode_sensor_orientation, encode_sensor_permission, encode_sensor_update, GeolocationErrorCode, PermissionAnswer, Sensor } from "@/kernel/abi/cmd.spec.ts";
import { CORE_DOORBELL } from "@/kernel/mem.ts";
import { request_frame, ring_now } from "@/kernel/sched.ts";
import { PLATFORM } from "./module.ts";

export function geolocationSupported(): boolean {
  return true;
}

export function orientationSupported(): boolean {
  return PLATFORM.headingAvailable();
}

/** The heading needs no prompt of its own: the location permission covers it. */
export function orientationNeedsPermission(): boolean {
  return false;
}

export function orientationSilenceIsLoss(): boolean {
  return false;
}

export function orientationPermit(): Promise<boolean> {
  return Promise.resolve(true);
}

function sendPermission(sensor: Sensor, answer: PermissionAnswer): void {
  CORE_DOORBELL.send(0, Command.SENSOR_PERMISSION, encode_sensor_permission({ sensor, answer }));
  request_frame();
}

/** The permission as last seen: what a return to the app compares against. */
let granted: boolean | null = null;

/** Whether the OS lets the app read where it is, asking only the first time.
 *
 * Asking is a system screen over the app even when the answer is already
 * given: Android opens its permission activity and closes it at once, and
 * the app pauses and resumes under it. A resume restarts the sensors, and a
 * sensor that asked on every start asked again — seven times a second, the
 * position never landing. So what is already answered is only read, as a
 * page's permission is; a refusal is not asked again, and a grant made in the
 * settings reaches the kernel on the way back (`recheck`). */
async function permitted(): Promise<boolean> {
  const now = await Location.getForegroundPermissionsAsync();
  const { status } = now.status === Location.PermissionStatus.UNDETERMINED ? await Location.requestForegroundPermissionsAsync() : now;
  granted = status === Location.PermissionStatus.GRANTED;
  return granted;
}

/** A grant made in the device's settings, which the app can only leave for
 * and come back from, reaches the kernel as it comes back — as a page's
 * permission `change` does (platform/web/sensor.ts), the one way back from a
 * refusal, since nothing else would ask again. What it returns stops
 * following. */
export function permissionFollow(): () => void {
  const back = AppState.addEventListener("change", (state) => {
    if (state === "active") recheck();
  });
  return () => back.remove();
}

function recheck(): void {
  if (granted !== false) return;
  void Location.getForegroundPermissionsAsync().then(({ status }) => {
    if (granted !== false || status !== Location.PermissionStatus.GRANTED) return;
    granted = true;
    sendPermission(Sensor.Geolocation, PermissionAnswer.Granted);
    sendPermission(Sensor.Orientation, PermissionAnswer.Granted);
  });
}

/** A watch, or one being started: a stop while the permission is still being
 * asked for ends the start where it lands rather than leaving a watch no one
 * holds. */
interface Watch {
  subscription: Location.LocationSubscription | null;
  stopped: boolean;
}

function watchEnd(watch: Watch): void {
  watch.stopped = true;
  watch.subscription?.remove();
  watch.subscription = null;
}

let geolocation: Watch | null = null;

/** The watch ended on its own: let go of it, and the kernel hears why. A
 * watch already stopped or replaced says nothing. */
function geolocationFail(watch: Watch, code: GeolocationErrorCode): void {
  if (geolocation !== watch) return;
  watchEnd(watch);
  geolocation = null;
  CORE_DOORBELL.send(0, Command.SENSOR_ERROR, encode_sensor_error({ code }));
  request_frame();
}

export function geolocationStart(): void {
  if (geolocation !== null) return;
  const watch: Watch = { subscription: null, stopped: false };
  geolocation = watch;
  void (async () => {
    if (!(await permitted())) return geolocationFail(watch, GeolocationErrorCode.PermissionDenied);
    const subscription = await Location.watchPositionAsync(
      { accuracy: Location.Accuracy.BestForNavigation, timeInterval: 1000, distanceInterval: 0 },
      (pos) => {
        const { latitude, longitude, altitude, accuracy, heading } = pos.coords;
        CORE_DOORBELL.send(0, Command.SENSOR_UPDATE, encode_sensor_update({ lon: longitude, lat: latitude, altitude, accuracy: accuracy ?? Infinity, heading }));
        request_frame();
      },
      () => geolocationFail(watch, GeolocationErrorCode.PositionUnavailable),
    );
    if (watch.stopped) subscription.remove();
    else watch.subscription = subscription;
  })().catch(() => geolocationFail(watch, GeolocationErrorCode.Other));
}

export function geolocationStop(): void {
  if (geolocation === null) return;
  watchEnd(geolocation);
  geolocation = null;
}

let orientation: Watch | null = null;

/** The compass ended on its own: a refusal is the permission's answer, and a
 * failure is the compass lost. */
function orientationFail(watch: Watch, refused: boolean): void {
  if (orientation !== watch) return;
  watchEnd(watch);
  orientation = null;
  if (refused) sendPermission(Sensor.Orientation, PermissionAnswer.Denied);
  else ring_now(Command.SENSOR_ORIENTATION_LOST);
}

export function orientationListen(): void {
  if (orientation !== null) return;
  const watch: Watch = { subscription: null, stopped: false };
  orientation = watch;
  void (async () => {
    if (!(await permitted())) return orientationFail(watch, true);
    const subscription = await Location.watchHeadingAsync(
      (h) => {
        // True north where the OS has a fix to correct by, magnetic otherwise.
        const heading = h.trueHeading >= 0 ? h.trueHeading : h.magHeading;
        CORE_DOORBELL.send(0, Command.SENSOR_ORIENTATION, encode_sensor_orientation({ webkit_compass_heading: heading, alpha: null, beta: null, gamma: null }));
        request_frame();
      },
      (reason) => {
        console.error("[sensor] heading:", reason);
        orientationFail(watch, false);
      },
    );
    if (watch.stopped) subscription.remove();
    else watch.subscription = subscription;
  })().catch((err) => {
    console.error("[sensor] heading:", err);
    orientationFail(watch, false);
  });
}

export function orientationRemove(): void {
  if (orientation === null) return;
  watchEnd(orientation);
  orientation = null;
}
